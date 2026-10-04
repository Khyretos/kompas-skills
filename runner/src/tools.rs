use std::fs::{self, canonicalize};
use std::io::Write;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::grants::{Grants, Right};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum Tool {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    ListDir { path: String },
    Shell { cwd: String, command: String },
    EditFile { path: String, old: String, new: String },
    Service { action: String, #[serde(default)] unit: Option<String> },
    Package { manager: String, action: String, names: Vec<String> },
    Reload { what: String },
    SystemInfo,
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
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Outcome { ok: false, output: format!("no such file or folder: {}", path.display()) },
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, path.display()) },
            };

            if !grants.allows(&canonical_path, &Right::Read, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Read, canonical_path.display()) };
            }

            if canonical_path.is_file() {
                let metadata = match fs::metadata(&canonical_path) {
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
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Outcome { ok: false, output: format!("no such folder: {}", parent_path.display()) },
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, path.display()) },
            };

            if !grants.allows(&canonical_parent, &Right::Write, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Write, canonical_parent.display()) };
            }

            if content.len() > 1024 * 1024 {
                return Outcome { ok: false, output: format!("content too big: {}", path.display()) };
            }

            let canonical_path = canonical_parent.join(path.file_name().unwrap_or_default());
            let mut file = match fs::File::create(&canonical_path) {
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
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Outcome { ok: false, output: format!("no such file or folder: {}", path.display()) },
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
                        entries.push((entry_name.to_string_lossy().into_owned(), entry_type));
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
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Outcome { ok: false, output: format!("no such folder: {}", cwd.display()) },
                Err(_) => return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, cwd.display()) },
            };

            if !grants.allows(&canonical_cwd, &Right::Shell, now) {
                return Outcome { ok: false, output: format!("not granted: {:?} on {}", Right::Shell, canonical_cwd.display()) };
            }

            crate::proc::run_cmd("sh", &["-c".to_string(), command.clone()], Some(&canonical_cwd), &[], 60)
        },
        Tool::EditFile { path, old, new } => crate::edit::edit_file(grants, path, old, new, now),
        Tool::Service { action, unit } => crate::systools::service(grants, action, unit.as_deref(), now),
        Tool::Package { manager, action, names } => crate::systools::package(grants, manager, action, names, now),
        Tool::Reload { what } => crate::systools::reload(grants, what, now),
        Tool::SystemInfo => crate::sysinfo::system_info(),
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
