use std::fs::{self, canonicalize, File, metadata};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Child};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use crate::grants::{Grants, Right};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum Tool {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    ListDir { path: String },
    Shell { cwd: String, command: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome { pub ok: bool, pub output: String }

/// Runs one tool call if the grants allow it. Never panics.
pub fn run(grants: &Grants, tool: &Tool, now: &str) -> Outcome {
    match tool {
        Tool::ReadFile { path } => {
            let path = Path::new(path);
            let canonical_path = match canonicalize(path) {
                Ok(p) => p,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, path.display()) },
            };

            if !grants.allows(&canonical_path, &Right::Read, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, canonical_path.display()) };
            }

            if canonical_path.is_file() {
                let metadata = match metadata(&canonical_path) {
                    Ok(m) => m,
                    Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, canonical_path.display()) },
                };

                if metadata.len() > 256 * 1024 {
                    return Outcome { ok: false, output: format!("file too big: {}", canonical_path.display()) };
                }

                let content = match fs::read_to_string(&canonical_path) {
                    Ok(c) => c,
                    Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, canonical_path.display()) },
                };

                Outcome { ok: true, output: content }
            } else {
                Outcome { ok: false, output: format!("not a file: {}", canonical_path.display()) }
            }
        },
        Tool::WriteFile { path, content } => {
            let path = Path::new(path);
            let parent_path = path.parent().unwrap_or(Path::new("."));
            let canonical_parent = match canonicalize(parent_path) {
                Ok(p) => p,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, path.display()) },
            };

            if !grants.allows(&canonical_parent, &Right::Write, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, canonical_parent.display()) };
            }

            if content.len() > 1024 * 1024 {
                return Outcome { ok: false, output: format!("content too big: {}", path.display()) };
            }

            let canonical_path = canonical_parent.join(path.file_name().unwrap_or(Path::new("")));
            let mut file = match File::create(&canonical_path) {
                Ok(f) => f,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, canonical_path.display()) },
            };

            if let Err(e) = file.write_all(content.as_bytes()) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, canonical_path.display()) };
            }

            Outcome { ok: true, output: format!("wrote {}", canonical_path.display()) }
        },
        Tool::ListDir { path } => {
            let path = Path::new(path);
            let canonical_path = match canonicalize(path) {
                Ok(p) => p,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, path.display()) },
            };

            if !grants.allows(&canonical_path, &Right::Read, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, canonical_path.display()) };
            }

            if !canonical_path.is_dir() {
                return Outcome { ok: false, output: format!("not a directory: {}", canonical_path.display()) };
            }

            let mut entries = Vec::new();
            if let Ok(dir) = fs::read_dir(&canonical_path) {
                for entry in dir {
                    if let Ok(entry) = entry {
                        let entry_path = entry.path();
                        let entry_name = entry.file_name();
                        let entry_type = if entry_path.is_dir() { "dir" } else { "file" };
                        let entry_name = entry_name.to_string_lossy();
                        entries.push((entry_name.as_ref(), entry_type));
                    }
                }
            }

            entries.sort();

            let mut output = String::new();
            for (i, (name, entry_type)) in entries.iter().enumerate() {
                if i >= 500 {
                    break;
                }

                let name = if *entry_type == "dir" {
                    format!("{}{}", name, '/')
                } else {
                    name.to_string()
                };

                output.push_str(&name);
                output.push('\n');
            }

            Outcome { ok: true, output }
        },
        Tool::Shell { cwd, command } => {
            let cwd = Path::new(cwd);
            let canonical_cwd = match canonicalize(cwd) {
                Ok(p) => p,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, cwd.display()) },
            };

            if !grants.allows(&canonical_cwd, &Right::Shell, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, canonical_cwd.display()) };
            }

            let mut child = match Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&canonical_cwd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn() {
                Ok(c) => c,
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, canonical_cwd.display()) },
            };

            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let start = Instant::now();

            while start.elapsed() < Duration::from_secs(60) {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let mut output = String::new();
                        if let Ok(s) = std::str::from_utf8(&stdout) {
                            output.push_str(s);
                        }
                        if let Ok(s) = std::str::from_utf8(&stderr) {
                            output.push_str(s);
                        }
                        output.push_str(&format!("exit: {}", status.code().unwrap_or(-1)));
                        return Outcome { ok: true, output };
                    },
                    Ok(None) => {
                        if let Ok(Some(buf)) = child.stdout.as_mut().and_then(BufRead::read_until) {
                            stdout.extend_from_slice(&buf);
                        }
                        if let Ok(Some(buf)) = child.stderr.as_mut().and_then(BufRead::read_until) {
                            stderr.extend_from_slice(&buf);
                        }
                    },
                    Err(_) => {
                        return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, canonical_cwd.display()) };
                    }
                }

                std::thread::sleep(Duration::from_millis(50));
            }

            let mut output = String::new();
            if let Ok(s) = std::str::from_utf8(&stdout) {
                output.push_str(s);
            }
            if let Ok(s) = std::str::from_utf8(&stderr) {
                output.push_str(s);
            }
            output.push_str("exit: -1");

            Outcome { ok: true, output }
        },
    }
}
