use std::{env, fs, os::unix::fs::{OpenOptionsExt, PermissionsExt}, path::PathBuf, process::Command};

/// The systemd user unit the installer writes.
const UNIT: &str = r#"[Unit]
Description=Kreative Kompanion runner (stats and granted tools, outbound only)
After=network-online.target

[Service]
ExecStart=%h/.local/bin/kompanion-runner
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
"#;

/// Returns the path to the Kompanion runner config directory.
fn config_dir() -> Result<PathBuf, String> {
    env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config").join("kompanion-runner"))
        .map_err(|_| "$HOME is not set".to_string())
}

/// Pairs the runner with a Kompanion server.
pub fn pair(server: &str, code: &str) -> Result<(), String> {
    // Validate server URL
    let server = if server.starts_with("http://localhost") || server.starts_with("http://127.0.0.1") {
        server.to_string()
    } else if !server.starts_with("https://") {
        return Err(format!("Server URL must start with https:// or http://localhost/http://127.0.0.1, got: {}", server));
    } else {
        server.to_string()
    };

    // Remove trailing slash
    let server = server.trim_end_matches('/');

    // Get hostname from /proc/sys/kernel/hostname
    let hostname = fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Prepare request payload
    let payload = serde_json::json!({
        "code": code,
        "hostname": hostname
    });

    // Create ureq agent with timeout
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(20))
        .build();

    // Make POST request
    let response = match agent
        .post(&format!("{}/api/pair", server))
        .set("X-Kompanion", "1")
        .send_json(&payload)
    {
        Ok(r) => r,
        Err(ureq::Error::Status(code, resp)) => {
            let msg = resp
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or_else(|| format!("pairing failed (HTTP {code})"));
            return Err(msg);
        }
        Err(e) => return Err(format!("can't reach {server}: {e}")),
    };

    // Parse success response
    let result: serde_json::Value = response.into_json()
        .map_err(|_| "unexpected answer from the server")?;

    let machine_id = result.get("machineId")
        .and_then(|v| v.as_str())
        .ok_or("unexpected answer from the server")?;

    let token = result.get("token")
        .and_then(|v| v.as_str())
        .ok_or("unexpected answer from the server")?;

    let name = result.get("name").and_then(|v| v.as_str()).unwrap_or("this computer");

    // Create config directory with mode 0700
    let config_path = config_dir()?;
    fs::create_dir_all(&config_path)
        .map_err(|e| format!("failed to create config directory: {}", e))?;
    
    let perms = fs::Permissions::from_mode(0o700);
    fs::set_permissions(&config_path, perms)
        .map_err(|e| format!("failed to set config directory permissions: {}", e))?;

    // Write token file with mode 0600
    let token_path = config_path.join("token");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&token_path)
        .map_err(|e| format!("failed to open token file: {}", e))?;
    f.write_all(token.as_bytes())
        .map_err(|e| format!("failed to write token file: {}", e))?;
    
    let token_perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(&token_path, token_perms)
        .map_err(|e| format!("failed to set token file permissions: {}", e))?;

    // Write config.toml
    let config_content = format!(
        r#"server = "{}"
machine_id = "{}"
token_file = "~/.config/kompanion-runner/token"
grants_file = "~/.config/kompanion-runner/grants.json""#,
        server, machine_id
    );
    
    fs::write(config_path.join("config.toml"), config_content)
        .map_err(|e| format!("failed to write config.toml: {}", e))?;

    // Create grants.json with [] if it doesn't exist
    let grants_path = config_path.join("grants.json");
    if !grants_path.exists() {
        fs::write(grants_path, "[]")
            .map_err(|e| format!("failed to create grants.json: {}", e))?;
    }

    println!("Paired as \"{}\"", name);
    Ok(())
}

/// Installs the systemd user service for the Kompanion runner.
pub fn install_service() -> Result<(), String> {
    let service_dir = env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config").join("systemd").join("user"))
        .map_err(|_| "$HOME is not set".to_string())?;
    
    fs::create_dir_all(&service_dir)
        .map_err(|e| format!("failed to write service file: {}", e))?;

    let service_path = service_dir.join("kompanion-runner.service");

    fs::write(&service_path, UNIT)
        .map_err(|e| format!("failed to write service file: {}", e))?;

    // Run systemctl commands
    let daemon_reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()
        .map_err(|e| format!("failed to run systemctl daemon-reload: {}", e))?;
    
    if !daemon_reload.status.success() {
        return Err(String::from_utf8_lossy(&daemon_reload.stderr).to_string());
    }

    let enable_cmd = Command::new("systemctl")
        .args(["--user", "enable", "--now", "kompanion-runner.service"])
        .output()
        .map_err(|e| format!("failed to run systemctl enable --now: {}", e))?;
    
    if !enable_cmd.status.success() {
        return Err(String::from_utf8_lossy(&enable_cmd.stderr).to_string());
    }

    let restart_cmd = Command::new("systemctl")
        .args(["--user", "restart", "kompanion-runner.service"])
        .output()
        .map_err(|e| format!("failed to run systemctl restart: {}", e))?;
    
    if !restart_cmd.status.success() {
        return Err(String::from_utf8_lossy(&restart_cmd.stderr).to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_text_contains_execstart() {
        assert!(UNIT.contains("ExecStart=%h/.local/bin/kompanion-runner"));
    }
}
