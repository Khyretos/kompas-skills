use std::env;
use std::fs;
use std::path::PathBuf;

use crate::tools::Outcome;

pub fn system_info() -> Outcome {
    let os = os_name();
    let kernel = read_trim("/proc/sys/kernel/osrelease");
    let hostname = read_trim("/proc/sys/kernel/hostname");
    let package_managers = find_package_managers();
    let desktop = env::var("XDG_CURRENT_DESKTOP").ok().and_then(|v| if v.is_empty() { None } else { Some(v) });
    let home = env::var("HOME").ok();
    let user = env::var("USER").ok();
    let hyprland_version = hyprland_version();
    let hyprland_config = hyprland_config();
    let hyprland = hypr_runtime_exists();
    let polkit_agent = polkit_agent();

    let json = serde_json::json!({
        "os": os,
        "kernel": kernel,
        "hostname": hostname,
        "package_managers": package_managers,
        "desktop": desktop,
        "home": home,
        "user": user,
        "hyprland_version": hyprland_version,
        "hyprland_config": hyprland_config,
        "hyprland": hyprland,
        "polkit_agent": polkit_agent
    });

    let output = serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string());
    Outcome { ok: true, output }
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn on_path(name: &str) -> bool {
    env::var("PATH").ok().is_some_and(|p| p.split(':').any(|dir| PathBuf::from(dir).join(name).exists()))
}

fn pretty_name(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| {
            let val = l.trim_start_matches("PRETTY_NAME=");
            // Remove surrounding quotes if present
            if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
                val[1..val.len()-1].to_string()
            } else {
                val.to_string()
            }
        })
}

fn os_name() -> Option<String> {
    read_trim("/etc/os-release")
        .as_deref()
        .and_then(pretty_name)
}

fn find_package_managers() -> Vec<String> {
    let names = ["pacman", "paru", "apt-get", "flatpak"];
    names
        .iter()
        .filter(|n| on_path(n))
        .map(|n| n.to_string())
        .collect()
}

fn hyprland_version() -> Option<String> {
    let out = std::process::Command::new("hyprctl")
        .args(["version", "-j"])
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v["tag"].as_str().map(str::to_string)
}

fn hyprland_config() -> Vec<String> {
    let Ok(home) = env::var("HOME") else { return Vec::new() };
    ["hyprland.lua", "hyprland.conf"]
        .iter()
        .map(|f| format!("{home}/.config/hypr/{f}"))
        .filter(|p| std::path::Path::new(p).is_file())
        .collect()
}

fn hypr_runtime_exists() -> bool {
    env::var("XDG_RUNTIME_DIR").ok().is_some_and(|dir| PathBuf::from(dir).join("hypr").exists())
}

fn polkit_agent() -> bool {
    let agents = [
        "hyprpolkitagent",
        "polkit-gnome-au",
        "polkit-kde-auth",
        "lxpolkit",
        "polkit-mate-aut",
    ];

    for pid in fs::read_dir("/proc").into_iter().flatten() {
        let Ok(entry) = pid else { continue };
        let Ok(pid_str) = entry.file_name().into_string() else { continue };
        
        // comm is cut to 15 chars
        let path = entry.path().join("comm");
        let Ok(content) = fs::read_to_string(&path) else { continue };
        
        let trimmed = content.trim();
        let name = if trimmed.len() > 15 {
            &trimmed[..15]
        } else {
            trimmed
        };
        
        let lower = name.to_lowercase();
        if agents.iter().any(|a| *a == lower) {
            return true;
        }
    }
    
    false
}
