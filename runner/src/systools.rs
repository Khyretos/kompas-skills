use std::{fs, path::Path};
use crate::grants::{Grants, Right};
use crate::proc::run_cmd;
use crate::tools::Outcome;

fn no(s: impl Into<String>) -> Outcome {
    Outcome { ok: false, output: s.into() }
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

pub fn valid_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        if first == '-' {
            return false;
        }
        for c in chars {
            if !c.is_ascii_alphanumeric() && !matches!(c, '@' | '.' | '_' | '+' | '-' | ':') {
                return false;
            }
        }
    } else {
        return false;
    }
    true
}

pub fn service(grants: &Grants, action: &str, unit: Option<&str>, now: &str) -> Outcome {
    if !grants.allows_system(&Right::Services, now) {
        return no("not granted: services");
    }

    let cmd = match action {
        "list" => argv(&["systemctl", "--user", "list-units", "--type=service", "--no-pager", "--plain"]),
        _ => {
            let unit_name = match unit {
                Some(u) => u,
                None => return no("unit name missing or invalid"),
            };

            if !valid_name(unit_name) {
                return no("unit name missing or invalid");
            }

            match action {
                "status" => argv(&["systemctl", "--user", "status", "--no-pager", unit_name]),
                "start" => argv(&["systemctl", "--user", "start", unit_name]),
                "stop" => argv(&["systemctl", "--user", "stop", unit_name]),
                "restart" => argv(&["systemctl", "--user", "restart", unit_name]),
                "enable" => argv(&["systemctl", "--user", "enable", unit_name]),
                "disable" => argv(&["systemctl", "--user", "disable", unit_name]),
                "is-active" => argv(&["systemctl", "--user", "is-active", unit_name]),
                _ => return no("unknown service action"),
            }
        }
    };

    run_cmd(&cmd[0], &cmd[1..], None, &[], 60)
}

pub fn package_cmd(manager: &str, action: &str, names: &[String]) -> Option<Vec<String>> {
    let base = match (manager, action) {
        ("pacman", "install") => argv(&["pkexec", "pacman", "-S", "--noconfirm", "--needed"]),
        ("pacman", "remove") => argv(&["pkexec", "pacman", "-R", "--noconfirm"]),
        ("pacman", "search") => argv(&["pacman", "-Ss"]),
        ("pacman", "info") => argv(&["pacman", "-Si"]),
        ("paru", "install") => argv(&["paru", "-S", "--noconfirm", "--needed", "--sudo", "pkexec"]),
        ("paru", "remove") => argv(&["paru", "-R", "--noconfirm", "--sudo", "pkexec"]),
        ("paru", "search") => argv(&["paru", "-Ss"]),
        ("paru", "info") => argv(&["paru", "-Si"]),
        ("apt", "install") => argv(&["pkexec", "apt-get", "install", "-y"]),
        ("apt", "remove") => argv(&["pkexec", "apt-get", "remove", "-y"]),
        ("apt", "search") => argv(&["apt-cache", "search"]),
        ("apt", "info") => argv(&["apt-cache", "show"]),
        ("flatpak", "install") => argv(&["flatpak", "install", "--user", "-y", "--noninteractive", "flathub"]),
        ("flatpak", "remove") => argv(&["flatpak", "uninstall", "--user", "-y", "--noninteractive"]),
        ("flatpak", "search") => argv(&["flatpak", "search"]),
        ("flatpak", "info") => argv(&["flatpak", "remote-info", "flathub"]),
        _ => return None,
    };

    let mut result = base;
    result.extend(names.iter().cloned());
    Some(result)
}

pub fn package(grants: &Grants, manager: &str, action: &str, names: &[String], now: &str) -> Outcome {
    if !grants.allows_system(&Right::Packages, now) {
        return no("not granted: packages");
    }

    if names.is_empty() || names.len() > 20 {
        return no("bad package name");
    }

    for name in names {
        if !valid_name(name) {
            return no("bad package name");
        }
    }

    let needs_root = matches!(manager, "pacman" | "paru" | "apt") && matches!(action, "install" | "remove");
    if needs_root && !grants.allows_system(&Right::Root, now) {
        return no("not granted: root");
    }

    let cmd = match package_cmd(manager, action, names) {
        Some(c) => c,
        None => return no("unknown package manager or action"),
    };

    let timeout = if matches!(action, "install" | "remove") { 1800 } else { 120 };
    run_cmd(&cmd[0], &cmd[1..], None, &[], timeout)
}

fn hypr_signature() -> Option<String> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let path = Path::new(&runtime_dir).join("hypr");
    
    if !path.exists() {
        return None;
    }

    let mut entries = fs::read_dir(&path).ok()?.filter_map(|e| e.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    
    if let Some(last) = entries.last() {
        return Some(last.file_name().to_string_lossy().into_owned());
    }
    None
}

pub fn reload(grants: &Grants, what: &str, now: &str) -> Outcome {
    if !grants.allows_system(&Right::Desktop, now) {
        return no("not granted: desktop");
    }

    let envs = if what == "hyprland" {
        let sig = hypr_signature();
        if sig.is_some() && std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err() {
            vec![("HYPRLAND_INSTANCE_SIGNATURE".to_string(), sig.unwrap())]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let cmd = match what {
        "hyprland" => argv(&["hyprctl", "reload"]),
        "waybar" => argv(&["pkill", "-SIGUSR2", "-x", "waybar"]),
        "mako" => argv(&["makoctl", "reload"]),
        "swaync" => argv(&["swaync-client", "-R"]),
        _ => return no("can reload: hyprland, waybar, mako, swaync"),
    };

    run_cmd(&cmd[0], &cmd[1..], None, &envs, 30)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process;

    /// An empty (deny-all) Grants: loading a path that does not exist.
    fn temp_grants() -> Grants {
        let dir = env::temp_dir().join(format!("kk-systools-{}", process::id()));
        let grants = Grants::load(&dir.join("grants.json")).unwrap();
        let _ = fs::remove_dir_all(&dir);
        grants
    }

    #[test]
    fn test_valid_name() {
        assert!(valid_name("firefox"));
        assert!(valid_name("org.mozilla.firefox"));
        assert!(valid_name("foo@1.service"));
        assert!(!valid_name(""));
        assert!(!valid_name("-rf"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("a;b"));
        assert!(!valid_name("$(x)"));
    }

    #[test]
    fn test_package_cmd_pacman_install() {
        let result = package_cmd("pacman", "install", &["htop".to_string()]);
        assert!(result.is_some());
        let cmd = result.unwrap();
        assert_eq!(cmd, argv(&["pkexec", "pacman", "-S", "--noconfirm", "--needed", "htop"]));
    }

    #[test]
    fn test_package_cmd_flatpak_search() {
        let result = package_cmd("flatpak", "search", &[]);
        assert!(result.is_some());
        let cmd = result.unwrap();
        assert_eq!(cmd, argv(&["flatpak", "search"]));
    }

    #[test]
    fn test_unknown_manager() {
        let result = package_cmd("unknown", "install", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_service_refused() {
        let grants = temp_grants();
        let outcome = service(&grants, "status", Some("test.service"), "2026-06-01T00:00:00Z");
        assert!(!outcome.ok);
        assert!(outcome.output.contains("not granted: services"));
    }

    #[test]
    fn test_package_refused() {
        let grants = temp_grants();
        let outcome = package(&grants, "pacman", "install", &["htop".to_string()], "2026-06-01T00:00:00Z");
        assert!(!outcome.ok);
        assert!(outcome.output.contains("not granted: packages"));
    }

    #[test]
    fn test_reload_refused() {
        let grants = temp_grants();
        let outcome = reload(&grants, "hyprland", "2026-06-01T00:00:00Z");
        assert!(!outcome.ok);
        assert!(outcome.output.contains("not granted: desktop"));
    }
}
