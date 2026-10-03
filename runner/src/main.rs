//! kompanion-runner: reports this PC's stats to a Kreative Kompanion server.
//! Outbound HTTPS only; nothing listens on this machine.
//!
//! Config (default `~/.config/kompanion-runner/config.toml`, or the path given
//! as the first argument):
//!
//! ```toml
//! server = "https://kompanion.kreative-kompas.com"
//! machine_id = "…"                 # from pairing
//! token_file = "~/.config/kompanion-runner/token"   # file with the token, mode 600
//! ```

#[allow(dead_code)]
mod grants;
#[allow(dead_code)] // wired into the job loop in the next step
mod tools;

use std::{fs, path::PathBuf, thread, time::Duration};

use machine_stats::Sampler;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    server: String,
    machine_id: String,
    token_file: String,
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p),
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| expand("~/.config/kompanion-runner/config.toml"));
    let cfg: Config = match fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| toml::from_str(&t).map_err(|e| e.to_string())) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("kompanion-runner: can't read {}: {e}", path.display());
            std::process::exit(2);
        }
    };
    let token = match fs::read_to_string(expand(&cfg.token_file)) {
        Ok(t) => t.trim().to_string(),
        Err(e) => {
            eprintln!("kompanion-runner: can't read the token file {}: {e}", cfg.token_file);
            std::process::exit(2);
        }
    };
    if !cfg.server.starts_with("https://") && !cfg.server.starts_with("http://localhost") && !cfg.server.starts_with("http://127.0.0.1") {
        eprintln!("kompanion-runner: the server must use https://");
        std::process::exit(2);
    }
    let url = format!("{}/api/machines/{}/stats", cfg.server.trim_end_matches('/'), cfg.machine_id);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let mut sampler = Sampler::new("/", "/");
    sampler.sample(); // first CPU/GPU counters
    let mut interval = 5u64;
    let mut backoff = 0u64;
    eprintln!("kompanion-runner: reporting to {}", cfg.server);
    loop {
        thread::sleep(Duration::from_secs(interval.max(1)));
        let snap = sampler.sample();
        match agent.post(&url).set("Authorization", &format!("Bearer {token}")).send_json(&snap) {
            Ok(resp) => {
                backoff = 0;
                if let Ok(v) = resp.into_json::<serde_json::Value>() {
                    interval = v["interval"].as_u64().unwrap_or(60).clamp(1, 300);
                }
            }
            Err(ureq::Error::Status(401, _)) => {
                eprintln!("kompanion-runner: the server refused the token; pair this PC again");
                std::process::exit(3);
            }
            Err(e) => {
                // Server away or restarting: back off up to five minutes.
                backoff = (backoff * 2).clamp(5, 300);
                eprintln!("kompanion-runner: {e}; retrying in {backoff} s");
                interval = backoff;
            }
        }
    }
}
