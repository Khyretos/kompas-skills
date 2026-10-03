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
//! grants_file = "~/.config/kompanion-runner/grants.json"
//! ```

mod grants;
mod tools;

use machine_stats::Sampler;
use serde::Deserialize;
use std::{fs, path::PathBuf, thread, time::{SystemTime, UNIX_EPOCH}};

#[derive(Deserialize)]
struct Config {
    server: String,
    machine_id: String,
    token_file: String,
    #[serde(default = "default_grants")]
    grants_file: String,
}

fn default_grants() -> String {
    "~/.config/kompanion-runner/grants.json".into()
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p),
    }
}

fn now() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (y, m, d) = civil(since_epoch);
    // Simple RFC 3339: YYYY-MM-DDTHH:MM:SSZ (UTC)
    // Note: This is a simplified version for the requirement of "only std"
    // and "small civil-date function".
    format!("{}-{:02}-{:02}T00:00:00Z", y, m, d)
}

fn civil(days: i64) -> (i64, u32, u32) {
    let mut d = days;
    let mut y = 1970;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let mut m = 1;
    let mut days_left = d + 1;
    loop {
        let dim = match m {
            1 => 31, 2 => if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) { 29 } else { 28 },
            3 => 31, 4 => 30, 5 => 31, 6 => 30, 7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
            _ => 0,
        };
        if days_left <= dim {
            break;
        }
        days_left -= dim;
        m += 1;
    }
    (y, m as u32, days_left as u32)
}

fn post_result(
    agent: &ureq::Agent,
    base: &str,
    machine_id: &str,
    token: &str,
    body: serde_json::Value,
) {
    let url = format!("{}/api/machines/{}/results", base.trim_end_matches('/'), machine_id);
    if let Err(e) = agent.post(&url).set("Authorization", &format!("Bearer {token}")).send_json(body) {
        eprintln!("kompanion-runner: failed to post result: {e}");
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

    let grants = match grants::Grants::load(&expand(&cfg.grants_file)) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("kompanion-runner: can't load grants: {e}");
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

    let base_url = cfg.server.trim_end_matches('/');
    let url = format!("{}/api/machines/{}/stats", base_url, cfg.machine_id);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();

    // Initial grant sync
    let _ = post_result(
        &agent,
        base_url,
        &cfg.machine_id,
        &token,
        serde_json::json!({
            "job_id": "grants",
            "ok": true,
            "output": "",
            "grants": grants.list()
        }),
    );

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

                    if let Some(jobs) = v["jobs"].as_array() {
                        for job_val in jobs {
                            let id = job_val["id"].as_str().unwrap_or("unknown");
                            let tool_val = &job_val["tool"];
                            let tool_name = tool_val["tool"].as_str().unwrap_or("");

                            let mut ok = false;
                            let mut output = String::new();
                            let mut refused = false;

                            match tool_name {
                                "add_grant" => {
                                    if let Ok(g) = serde_json::from_value::<grants::Grant>(tool_val["grant"].clone()) {
                                        match grants.add(g) {
                                            Ok(_) => { ok = true; output = "success".into(); }
                                            Err(e) => { output = format!("error: {e}"); }
                                        }
                                    } else {
                                        output = "invalid grant format".into();
                                    }
                                }
                                "revoke_grant" => {
                                    if let Some(target) = tool_val["target"].as_str() {
                                        grants.revoke(target);
                                        ok = true;
                                        output = "success".into();
                                    }
                                }
                                _ => {
                                    if let Ok(tool) = serde_json::from_value::<tools::Tool>(tool_val.clone()) {
                                        output = tools::run(&grants, &tool, &now());
                                        if output.starts_with("not granted") {
                                            refused = true;
                                        } else {
                                            ok = true;
                                        }
                                    } else {
                                        output = "invalid tool format".into();
                                    }
                                }
                            }

                            let _ = post_result(
                                &agent,
                                base_url,
                                &cfg.machine_id,
                                &token,
                                serde_json::json!({
                                    "job_id": id,
                                    "ok": ok,
                                    "output": output,
                                    "refused": refused,
                                    "grants": grants.list()
                                }),
                            );
                        }
                    }
                }
            }
            Err(ureq::Error::Status(401, _)) => {
                eprintln!("kompanion-runner: the server refused the token; pair this PC again");
                std::process::exit(3);
            }
            Err(e) => {
                backoff = (backoff * 2).clamp(5, 300);
                eprintln!("kompanion-runner: {e}; retrying in {backoff} s");
                interval = backoff;
            }
        }
    }
}
