//! kompanion-gpu-role: the only piece that may change what runs on kireserver's A770.
//! Kompanion's server sends ONE line with a fixed command over a unix socket; the helper
//! runs a fixed program with fixed arguments (never a shell, no free text) and answers
//! with one JSON line. One command at a time: a second client waits for the first.
//!
//! Usage: kompanion-gpu-role [socket] (default ~/.local/run/kompanion-gpu-role/role.sock)

use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::{UnixListener, UnixStream}},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const GPU_MODE: &str = "/home/khyretos/Docker/Services/ai/gpu-share/gpu-mode.sh";
const DOCKER: &str = "/usr/bin/docker";
const DEFAULT_SOCKET: &str = "/home/khyretos/.local/run/kompanion-gpu-role/role.sock";
const POLL: Duration = Duration::from_millis(200);

/// The whole command list: (program, args, timeout in seconds). Nothing else runs.
fn command(line: &str) -> Option<(&'static str, Vec<&'static str>, u64)> {
    Some(match line {
        "status" => (GPU_MODE, vec!["status"], 60),
        // gpu-mode.sh waits up to an hour for OVMS to be idle before it unloads Coder.
        "artist" => (GPU_MODE, vec!["artist"], 3900),
        "coder" => (GPU_MODE, vec!["coder"], 900),
        "studio comfyui" => (GPU_MODE, vec!["studio", "comfyui"], 3900),
        "studio heartmula" => (GPU_MODE, vec!["studio", "heartmula"], 3900),
        "studio moss-sfx" => (GPU_MODE, vec!["studio", "moss-sfx"], 3900),
        "restart-ovms" => (DOCKER, vec!["restart", "ovms"], 300),
        "ovms-log" => (DOCKER, vec!["logs", "--since", "180s", "ovms"], 60),
        _ => return None,
    })
}

/// A JSON string literal, quotes included.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn answer(ok: bool, code: i32, secs: f64, output: &str) -> String {
    format!("{{\"ok\":{ok},\"code\":{code},\"seconds\":{secs:.1},\"output\":{}}}\n", json_str(output))
}

fn run(program: &str, args: &[&str], timeout_s: u64) -> (String, i32, f64) {
    let start = Instant::now();
    let mut child = match Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => return (answer(false, -3, 0.0, &e.to_string()), -3, 0.0),
    };
    let reader = |mut r: Box<dyn Read + Send>| thread::spawn(move || { let mut s = String::new(); let _ = r.read_to_string(&mut s); s });
    let out = child.stdout.take().map(|o| reader(Box::new(o)));
    let err = child.stderr.take().map(|e| reader(Box::new(e)));
    let deadline = start + Duration::from_secs(timeout_s);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let mut text = out.and_then(|t| t.join().ok()).unwrap_or_default();
    text += &err.and_then(|t| t.join().ok()).unwrap_or_default();
    let n = text.chars().count();
    let mut text: String = text.chars().skip(n.saturating_sub(4000)).collect();
    let secs = start.elapsed().as_secs_f64();
    let (ok, code) = match status {
        Some(s) => (s.success(), s.code().unwrap_or(-4)),
        None => {
            text += &format!("\ntimed out after {timeout_s} s");
            (false, -2)
        }
    };
    (answer(ok, code, secs, &text), code, secs)
}

fn handle(stream: &mut UnixStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut bytes = Vec::new();
    let mut b = [0u8; 1];
    while bytes.len() < 64 {
        match stream.read(&mut b) {
            Ok(1) if b[0] == b'\n' => break,
            Ok(1) if b[0] == b'\r' => {}
            Ok(1) => bytes.push(b[0]),
            _ => break,
        }
    }
    let line = String::from_utf8_lossy(&bytes).trim().to_string();
    let Some((program, args, timeout)) = command(&line) else {
        eprintln!("kompanion-gpu-role: refused {line:?}");
        return answer(false, -1, 0.0, "unknown command");
    };
    let (text, code, secs) = run(program, &args, timeout);
    eprintln!("kompanion-gpu-role: {line} -> {code} in {secs:.1} s");
    text
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_SOCKET.to_string());
    if let Some(dir) = Path::new(&path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kompanion-gpu-role: can't bind {path}: {e}");
            std::process::exit(1);
        }
    };
    // Owner and group (khyretos) only: the Kompanion container runs with gid 1000.
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o660));
    eprintln!("kompanion-gpu-role: serving {path}");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let reply = handle(&mut stream);
        let _ = stream.write_all(reply.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_commands_run() {
        assert_eq!(command("artist").map(|c| c.2), Some(3900));
        assert_eq!(command("studio comfyui").map(|c| c.1), Some(vec!["studio", "comfyui"]));
        assert!(command("artist --force").is_none());
        assert!(command("rm -rf /").is_none());
        assert!(command("").is_none());
        assert!(command("studio comfyui; reboot").is_none());
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_str("a\"b\n"), "\"a\\\"b\\n\"");
        assert_eq!(json_str("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn a_command_runs_and_times_out() {
        let (text, code, _) = run("/bin/echo", &["hi"], 5);
        assert_eq!(code, 0);
        assert!(text.contains("\"ok\":true") && text.contains("hi\\n"));
        let (text, code, _) = run("/bin/sleep", &["5"], 1);
        assert_eq!(code, -2);
        assert!(text.contains("timed out"));
    }
}
