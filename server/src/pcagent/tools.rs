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
                    if !known.contains(k) {
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
    Some(json!({ "tool": name, ..clean }))
}

/// Generate a short, readable summary line for a job.
pub fn summary(job: &Value) -> String {
    let tool = job.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    let args = job.get("args").and_then(|v| v.as_object()).unwrap_or(&serde_json::Map::new());

    match tool {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            format!("Read {}", path)
        }
        "list_dir" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            format!("List {}", path)
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            format!("Write {}", path)
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            format!("Edit {}", path)
        }
        "shell" => {
            let cwd = args.get("cwd").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
            format!("Run `{}` in {}", cmd, cwd)
        }
        "service" => {
            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let unit = args.get("unit").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(u) = unit {
                format!("{} the user service {}", action, u)
            } else {
                format!("{}", action)
            }
        }
        "package" => {
            let manager = args.get("manager").and_then(|v| v.as_str()).unwrap_or("");
            let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let names = args.get("names").and_then(|v| v.as_array()).unwrap_or(&Vec::<Value>::new());
            let names_str = names.iter().filter_map(|n| n.as_str()).collect::<Vec<_>>().join(", ");
            format!("{} {} with {}", action, names_str, manager)
        }
        "reload" => {
            let what = args.get("what").and_then(|v| v.as_str()).unwrap_or("");
            format!("reload {}", what)
        }
        "system_info" => {
            "Show system info".to_string()
        }
        _ => "Unknown job".to_string(),
    }
}

/// Determine the grant required for a job.
/// Returns None if paths are invalid (relative or containing '..').
pub fn grant_for(job: &Value) -> Option<(String, Vec<&'static str>)> {
    let tool = job.get("tool").and_then(|v| v.as_str())?;
    let args = job.get("args").and_then(|v| v.as_object())?;

    // Helper to validate absolute paths without '..'
    let valid_path = |p: &str| -> bool {
        p.starts_with('/') && !p.contains("..")
    };

    match tool {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str())?;
            if !valid_path(path) {
                return None;
            }
            // Parent folder of path
            let parent = if path == "/" {
                "/".to_string()
            } else {
                path[..path.rfind('/').unwrap_or(0)..].trim_start_matches('/').to_string()
            };
            Some((parent, vec!["read"]))
        }
        "list_dir" => {
            let path = args.get("path").and_then(|v| v.as_str())?;
            if !valid_path(path) {
                return None;
            }
            // Path itself
            Some((path.to_string(), vec!["read"]))
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str())?;
            if !valid_path(path) {
                return None;
            }
            let parent = if path == "/" {
                "/".to_string()
            } else {
                path[..path.rfind('/').unwrap_or(0)..].trim_start_matches('/').to_string()
            };
            Some((parent, vec!["write"]))
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str())?;
            if !valid_path(path) {
                return None;
            }
            let parent = if path == "/" {
                "/".to_string()
            } else {
                path[..path.rfind('/').unwrap_or(0)..].trim_start_matches('/').to_string()
            };
            Some((parent, vec!["write"]))
        }
        "shell" => {
            let cwd = args.get("cwd").and_then(|v| v.as_str())?;
            if !valid_path(cwd) {
                return None;
            }
            Some((cwd.to_string(), vec!["shell"]))
        }
        "service" => {
            Some(("system".to_string(), vec!["services"]))
        }
        "package" => {
            let manager = args.get("manager").and_then(|v| v.as_str())?;
            let action = args.get("action").and_then(|v| v.as_str())?;
            // If action is install/remove and manager is not flatpak -> packages, root
            // else -> packages
            let perms = if (action == "install" || action == "remove") && manager != "flatpak" {
                vec!["packages", "root"]
            } else {
                vec!["packages"]
            };
            Some(("system".to_string(), perms))
        }
        "reload" => {
            Some(("system".to_string(), vec!["desktop"]))
        }
        "system_info" => {
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_has_9_tools() {
        let schema = schema();
        let tools = schema.as_array().unwrap();
        assert_eq!(tools.len(), 9);
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
            "args": {
                "path": "/home/k/x.conf",
                "old": "foo",
                "new": "bar"
            }
        });
        let grant = grant_for(&job).unwrap();
        assert_eq!(grant.0, "/home/k");
        assert_eq!(grant.1, vec!["write"]);
    }

    #[test]
    fn test_grant_for_package_install() {
        let job = json!({
            "tool": "package",
            "args": {
                "manager": "paru",
                "action": "install",
                "names": ["htop"]
            }
        });
        let grant = grant_for(&job).unwrap();
        assert_eq!(grant.0, "system");
        assert_eq!(grant.1, vec!["packages", "root"]);
    }

    #[test]
    fn test_grant_for_read_file_relative() {
        let job = json!({
            "tool": "read_file",
            "args": {
                "path": "relative/path.txt"
            }
        });
        assert!(grant_for(&job).is_none());
    }
}
