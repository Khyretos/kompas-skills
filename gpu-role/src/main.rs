use std::env;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const GPU_MODE: &str = "/home/khyretos/Docker/Services/ai/gpu-share/gpu-mode.sh";
const DOCKER: &str = "/usr/bin/docker";
const DEFAULT_SOCKET: &str = "/home/khyretos/.local/run/kompanion-gpu-role/role.sock";
const MAX_BYTES: usize = 64;
const POLL_INTERVAL: Duration = Duration::from_millis(200);

fn main() {
    let args: Vec<String> = env::args().collect();
    let socket_path = if args.len() > 1 {
        &args[1]
    } else {
        DEFAULT_SOCKET
    };

    let parent_dir = fs::parent_path(socket_path).expect("Failed to get parent directory");
    fs::create_dir_all(&parent_dir).expect("Failed to create parent directory");

    let _ = fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path).expect("Failed to bind socket");
    let perms = fs::Permissions::from_mode(0o660);
    fs::set_permissions(socket_path, perms).expect("Failed to set permissions");

    eprintln!("kompanion-gpu-role: serving {}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_connection(stream),
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: io::BufReader<impl io::Read>) {
    let mut buffer = vec![0u8; MAX_BYTES];
    let mut bytes_read = 0usize;
    let mut line = String::new();

    loop {
        match stream.read(&mut buffer[bytes_read..]) {
            Ok(0) => break,
            Ok(n) => {
                bytes_read += n;
                if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    line.push_str(&String::from_utf8_lossy(&buffer[..pos]));
                    break;
                } else if bytes_read >= MAX_BYTES {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let trimmed = line.trim();
    if let Some((program, args, timeout)) = command(trimmed) {
        run_command(program, args, timeout);
    } else {
        let response = format!(
            r#"{{"ok":false,"code":-1,"output":"unknown command"}}"#
        );
        println!("{}", response);
    }
}

fn command(line: &str) -> Option<(&'static str, Vec<&'static str>, u64)> {
    match line {
        "status" => Some((GPU_MODE, vec!["status"], 60)),
        "artist" => Some((GPU_MODE, vec!["artist"], 3900)),
        "coder" => Some((GPU_MODE, vec!["coder"], 900)),
        "studio comfyui" => Some((GPU_MODE, vec!["studio", "comfyui"], 3900)),
        "studio heartmula" => Some((GPU_MODE, vec!["studio", "heartmula"], 3900)),
        "studio moss-sfx" => Some((GPU_MODE, vec!["studio", "moss-sfx"], 3900)),
        "restart-ovms" => Some((DOCKER, vec!["restart", "ovms"], 300)),
        "ovms-log" => Some((DOCKER, vec!["logs", "--since", "180s", "ovms"], 60)),
        _ => None,
    }
}

fn run_command(program: &str, args: Vec<&str>, timeout: u64) {
    let start = Instant::now();
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn command");

    let stdout = output.stdout.take().unwrap();
    let stderr = output.stderr.take().unwrap();

    let (stdout_reader, stderr_reader) = (stdout, stderr);

    let stdout_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        stdout_reader.read_to_end(&mut buf).ok();
        String::from_utf8_lossy(&buf).to_string()
    });

    let stderr_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        stderr_reader.read_to_end(&mut buf).ok();
        String::from_utf8_lossy(&buf).to_string()
    });

    let deadline = start + Duration::from_secs(timeout);
    let mut elapsed = 0u64;

    while start.elapsed() < deadline {
        if let Some(code) = output.try_wait().unwrap() {
            let stdout = stdout_thread.join().unwrap_or_default();
            let stderr = stderr_thread.join().unwrap_or_default();
            let combined = stdout + stderr;
            let truncated = if combined.len() > 4000 {
                combined.chars().take(4000).collect::<String>()
            } else {
                combined
            };
            let response = format!(
                r#"{{"ok":true,"code":{},"seconds":{:.1},"output":"{}"}}"#,
                code,
                start.elapsed().as_secs_f64(),
                json_str(&truncated)
            );
            println!("{}", response);
            return;
        }
        thread::sleep(POLL_INTERVAL);
        elapsed = start.elapsed().as_secs();
    }

    let _ = output.kill();
    let _ = output.wait();

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();
    let combined = stdout + stderr;
    let truncated = if combined.len() > 4000 {
        combined.chars().take(4000).collect::<String>()
    } else {
        combined
    };

    let response = format!(
        r#"{{"ok":false,"code":-2,"seconds":{:.1},"output":"{}"}}"#,
        start.elapsed().as_secs_f64(),
        json_str(&truncated)
    );
    println!("{}", response);

    eprintln!("command: {}, code: -2, seconds: {:.1}", program, start.elapsed().as_secs_f64());
}

fn json_str(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_artist() {
        assert_eq!(
            command("artist"),
            Some((GPU_MODE, vec!["artist"], 3900))
        );
    }

    #[test]
    fn test_command_studio_comfyui() {
        assert_eq!(
            command("studio comfyui"),
            Some((GPU_MODE, vec!["studio", "comfyui"], 3900))
        );
    }

    #[test]
    fn test_command_invalid() {
        assert!(command("artist --force").is_none());
        assert!(command("rm -rf /").is_none());
        assert!(command("").is_none());
    }

    #[test]
    fn test_json_str() {
        assert_eq!(json_str("a\"b\n"), r#""a\"b\n""#);
    }
}
