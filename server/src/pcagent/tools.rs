use serde_json::{Value, json};

/// The schema of PC agent tools available to the runner.
pub fn schema() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a text file (up to 256 KiB).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List the contents of a directory.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create or replace a file with new content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Replace one exact, unique piece of text in a file; returns a diff.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "old": { "type": "string" },
                        "new": { "type": "string" }
                    },
                    "required": ["path", "old", "new"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "shell",
                "description": "Run a shell command in a folder (60 s limit).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "cwd": { "type": "string" },
                        "command": { "type": "string" }
                    },
                    "required": ["cwd", "command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "service",
                "description": "Manage the user's systemd services.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["list", "status", "start", "stop", "restart", "enable", "disable", "is-active"] },
                        "unit": { "type": "string" }
                    },
                    "required": ["action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "package",
                "description": "Install/remove packages using a package manager (asks for password on the PC).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "manager": { "type": "string", "enum": ["pacman", "paru", "apt", "flatpak"] },
                        "action": { "type": "string", "enum": ["install", "remove", "search", "info"] },
                        "names": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["manager", "action", "names"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "reload",
                "description": "Reload a desktop compositor or panel.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "what": { "type": "string", "enum": ["hyprland", "waybar", "mako", "swaync"] }
                    },
                    "required": ["what"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "system_info",
                "description": "Show system information (OS, kernel, package managers, desktop).",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "capabilities",
                "description": "What Kompanion can use right now: models and their status, the user's computers with their grants, the tools and the skills. Answered by Kompanion itself, not a computer.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }
    ])
}

/// Convert a tool call JSON into a runner job JSON.
/// Returns None if the name is not recognized or arguments are invalid.
pub fn to_job(name: &str, args: &Value) -> Option<Value> {
    let known = match name {
        "read_file" => vec!["path"],
        "list_dir" => vec!["path"],
        "write_file" => vec!["path", "content"],
        "edit_file" => vec!["path", "old", "new"],
        "shell" => vec!["cwd", "command"],
        "service" => vec!["action", "unit"],
        "package" => vec!["manager", "action", "names"],
        "reload" => vec!["what"],
        "system_info" => vec![],
        _ => return None,
    };

    // Ensure all string values in args are actually Strings (not numbers or booleans masquerading)
    // and that arrays contain strings.
    let clean_args = |v: &Value| -> Option<Value> {
        match v {
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map {
                    if !known.contains(&k.as_str()) {
                        continue;
                    }
                    match k.as_str() {
                        "names" => {
                            // Must be an array of strings
                            if let Value::Array(arr) = val {
                                if arr.iter().all(|item| item.is_string()) {
                                    out.insert(k.clone(), val.clone());
                                } else {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                        _ => {
                            // All other params must be strings
                            if val.is_string() {
                                out.insert(k.clone(), val.clone());
                            } else {
                                return None;
                            }
                        }
                    }
                }
                Some(Value::Object(out))
            }
            _ => None,
        }
    };

    let clean = clean_args(args)?;
    let Value::Object(mut map) = clean else { return None };
    map.insert("tool".into(), json!(name));
    Some(Value::Object(map))
}

/// One readable line for an approval card. The job is `{"tool": name, ...args}`.
pub fn summary(job: &Value) -> String {
    let s = |k: &str| job[k].as_str().unwrap_or("").to_string();
    let line = match job["tool"].as_str().unwrap_or("") {
        "read_file" => format!("Read {}", s("path")),
        "list_dir" => format!("List {}", s("path")),
        "write_file" => format!("Write {}", s("path")),
        "edit_file" => format!("Edit {}", s("path")),
        "shell" => format!("Run `{}` in {}", s("command"), s("cwd")),
        "service" => match job["unit"].as_str() {
            Some(unit) => format!("{} the user service {unit}", s("action")),
            None => format!("{} user services", s("action")),
        },
        "package" => {
            let names: Vec<&str> = job["names"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            format!("{} {} with {}", s("action"), names.join(", "), s("manager"))
        }
        "reload" => format!("reload {}", s("what")),
        "system_info" => "Show system info".to_string(),
        other => format!("Run {other}"),
    };
    line.chars().take(200).collect()
}

/// The grant "Always allow" creates for this job: (target, rights).
pub fn grant_for(job: &Value) -> Option<(String, Vec<&'static str>)> {
    let path = || job["path"].as_str().filter(|p| safe(p)).map(str::to_string);
    match job["tool"].as_str()? {
        "read_file" => Some((parent(&path()?), vec!["read"])),
        "list_dir" => Some((path()?, vec!["read"])),
        "write_file" | "edit_file" => Some((parent(&path()?), vec!["write"])),
        "shell" => Some((job["cwd"].as_str().filter(|p| safe(p))?.to_string(), vec!["shell"])),
        "service" => Some(("system".into(), vec!["services"])),
        "package" => {
            let changes = matches!(job["action"].as_str(), Some("install" | "remove"));
            let root = changes && job["manager"].as_str() != Some("flatpak");
            Some(("system".into(), if root { vec!["packages", "root"] } else { vec!["packages"] }))
        }
        "reload" => Some(("system".into(), vec!["desktop"])),
        _ => None,
    }
}

/// The folder a path is in ("/" for top-level files).
fn parent(path: &str) -> String {
    std::path::Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.is_empty()).unwrap_or_else(|| "/".into())
}

/// Absolute and without `..` components.
fn safe(path: &str) -> bool {
    path.starts_with('/') && !path.split('/').any(|c| c == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_has_10_tools() {
        let schema = schema();
        let tools = schema.as_array().unwrap();
        assert_eq!(tools.len(), 10);
    }

    #[test]
    fn test_to_job_edit_file_keeps_only_keys() {
        let input = json!({
            "path": "/home/k/x.conf",
            "old": "foo",
            "new": "bar",
            "extra": "ignored"
        });
        let job = to_job("edit_file", &input).unwrap();
        let obj = job.as_object().unwrap();
        assert_eq!(obj.len(), 4); // tool + path + old + new
        assert!(obj.contains_key("tool"));
        assert!(obj.contains_key("path"));
        assert!(obj.contains_key("old"));
        assert!(obj.contains_key("new"));
        assert!(!obj.contains_key("extra"));
    }

    #[test]
    fn test_to_job_unknown_tool() {
        assert!(to_job("rm", &json!({})) .is_none());
    }

    #[test]
    fn test_grant_for_edit_file() {
        let job = json!({
            "tool": "edit_file",
            "path": "/home/k/x.conf",
                "old": "foo",
                "new": "bar"
        });
        let grant = grant_for(&job).unwrap();
        assert_eq!(grant.0, "/home/k");
        assert_eq!(grant.1, vec!["write"]);
    }

    #[test]
    fn test_grant_for_package_install() {
        let job = json!({
            "tool": "package",
            "manager": "paru",
                "action": "install",
                "names": ["htop"]
        });
        let grant = grant_for(&job).unwrap();
        assert_eq!(grant.0, "system");
        assert_eq!(grant.1, vec!["packages", "root"]);
    }

    #[test]
    fn test_grant_for_read_file_relative() {
        let job = json!({
            "tool": "read_file",
            "path": "relative/path.txt"
        });
        assert!(grant_for(&job).is_none());
    }
}
