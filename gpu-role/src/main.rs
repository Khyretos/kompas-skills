//! kompanion-gpu-role: the only piece that may change what runs on a GPU shared by a model server and studio apps.
//! Kompanion's server sends ONE line with a fixed command over a unix socket; the helper
//! runs a fixed program with fixed arguments (never a shell, no free text) and answers
//! with one JSON line. One command at a time: a second client waits for the first.
//!
//! Usage: kompanion-gpu-role [socket] (default ~/.local/run/kompanion-gpu-role/role.sock).
//! Paths come from the environment, read once at start: KOMPANION_GPU_MODE (the gpu-mode.sh
//! script; without it the switch commands are refused) and KOMPANION_OVMS_CONTAINER (default "ovms").

use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::{UnixListener, UnixStream}},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DOCKER: &str = "/usr/bin/docker";
const POLL: Duration = Duration::from_millis(200);

/// Which program a command runs; the paths are resolved at start (see `Paths`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Prog { GpuMode, Docker }

/// Read once at start from the environment.
struct Paths { gpu_mode: Option<String>, ovms: String }

impl Paths {
    fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let ovms = get("KOMPANION_OVMS_CONTAINER").filter(|c| container_name_ok(c)).unwrap_or_else(|| "ovms".to_string());
        Paths { gpu_mode: get("KOMPANION_GPU_MODE"), ovms }
    }
}

/// A docker container name: letters, digits, `_`, `.`, `-`, not starting with `-`.
fn container_name_ok(c: &str) -> bool {
    !c.is_empty() && c.len() <= 64 && !c.starts_with('-') && c.chars().all(|ch| ch.is_ascii_alphanumeric() || "_.-".contains(ch))
}

/// The whole command list: (program, args, timeout in seconds). Nothing else runs.
fn command(line: &str, ovms: &str) -> Option<(Prog, Vec<String>, u64)> {
    Some(match line {
        "status" => (Prog::GpuMode, vec!["status".to_string()], 60),
        // gpu-mode.sh waits up to an hour for OVMS to be idle before it unloads Coder.
        "artist" => (Prog::GpuMode, vec!["artist".to_string()], 3900),
        "coder" => (Prog::GpuMode, vec!["coder".to_string()], 900),
        "studio comfyui" => (Prog::GpuMode, vec!["studio".to_string(), "comfyui".to_string()], 3900),
        "studio heartmula" => (Prog::GpuMode, vec!["studio".to_string(), "heartmula".to_string()], 3900),
        "studio moss-sfx" => (Prog::GpuMode, vec!["studio".to_string(), "moss-sfx".to_string()], 3900),
        "restart-ovms" => (Prog::Docker, vec!["restart".to_string(), ovms.to_string()], 300),
        "ovms-log" => (Prog::Docker, vec!["logs".to_string(), "--since".to_string(), "180s".to_string(), ovms.to_string()], 60),
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

fn run(program: &str, args: &[String], timeout_s: u64) -> (String, i32, f64) {
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

fn handle(stream: &mut UnixStream, paths: &Paths) -> String {
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
    let Some((prog, args, timeout)) = command(&line, &paths.ovms) else {
        eprintln!("kompanion-gpu-role: refused {line:?}");
        return answer(false, -1, 0.0, "unknown command");
    };
    let program = match prog {
        Prog::Docker => DOCKER.to_string(),
        Prog::GpuMode => match &paths.gpu_mode {
            Some(p) => p.clone(),
            None => return answer(false, -5, 0.0, "KOMPANION_GPU_MODE is not set: run install-user.sh /path/to/gpu-mode.sh"),
        },
    };
    let (text, code, secs) = run(&program, &args, timeout);
    eprintln!("kompanion-gpu-role: {line} -> {code} in {secs:.1} s");
    text
}

fn main() {
    let paths = Paths::from_env();
    let path = std::env::args().nth(1).unwrap_or_else(|| format!("{}/.local/run/kompanion-gpu-role/role.sock", std::env::var("HOME").unwrap_or_default()));
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
    eprintln!("kompanion-gpu-role: gpu-mode {}", paths.gpu_mode.as_deref().unwrap_or("(not set)"));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let reply = handle(&mut stream, &paths);
        let _ = stream.write_all(reply.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_names_cannot_be_options() {
        assert!(container_name_ok("ovms"));
        assert!(container_name_ok("my_llm-1.0"));
        assert!(!container_name_ok("--privileged"));
        assert!(!container_name_ok("a b"));
        assert!(!container_name_ok(""));
    }

    #[test]
    fn only_listed_commands_run() {
        assert_eq!(command("artist", "ovms").map(|c| c.2), Some(3900));
        assert_eq!(command("studio comfyui", "ovms").map(|c| c.1), Some(vec!["studio".to_string(), "comfyui".to_string()]));
        assert!(command("artist --force", "ovms").is_none());
        assert!(command("rm -rf /", "ovms").is_none());
        assert!(command("", "ovms").is_none());
        assert!(command("studio comfyui; reboot", "ovms").is_none());
        assert_eq!(command("restart-ovms", "my-llm").map(|c| (c.0, c.1)), Some((Prog::Docker, vec!["restart".to_string(), "my-llm".to_string()])));
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_str("a\"b\n"), "\"a\\\"b\\n\"");
        assert_eq!(json_str("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn a_command_runs_and_times_out() {
        let (text, code, _) = run("/bin/echo", &["hi".to_string()], 5);
        assert_eq!(code, 0);
        assert!(text.contains("\"ok\":true") && text.contains("hi\\n"));
        let (text, code, _) = run("/bin/sleep", &["5".to_string()], 1);
        assert_eq!(code, -2);
        assert!(text.contains("timed out"));
    }
}
