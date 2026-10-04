//! `kompanion-runner ask "..."`: asks Kompanion from this computer's terminal. The
//! answer may use this computer's tools; each step is approved in the Kompanion web
//! app. (Claude wrote this after two failed model drafts.)

use std::{thread, time::Duration};

use serde_json::Value;

fn error_text(e: ureq::Error, base: &str) -> String {
    match e {
        ureq::Error::Status(code, resp) => resp
            .into_json::<Value>()
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_string))
            .unwrap_or_else(|| format!("Kompanion answered HTTP {code}")),
        e => format!("can't reach {base}: {e}"),
    }
}

/// `after` is an RFC 3339 time; `+` and `:` must be escaped in the query string.
fn encode(after: &str) -> String {
    after.replace('+', "%2B").replace(':', "%3A")
}

pub fn ask(server: &str, machine_id: &str, token: &str, question: &str) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let base = server.trim_end_matches('/');
    let auth = format!("Bearer {token}");
    let started: Value = agent
        .post(&format!("{base}/api/machines/{machine_id}/ask"))
        .set("Authorization", &auth)
        .set("X-Kompanion", "1")
        .send_json(serde_json::json!({ "text": question }))
        .map_err(|e| error_text(e, base))?
        .into_json()
        .map_err(|e| format!("unexpected answer: {e}"))?;
    let (Some(chat_id), Some(mut after)) = (
        started["chatId"].as_str().map(str::to_string),
        started["after"].as_str().map(str::to_string),
    ) else {
        return Err("unexpected answer from Kompanion".into());
    };

    let mut shown: Vec<String> = Vec::new();
    let mut failures = 0u32;
    for _ in 0..1800 {
        thread::sleep(Duration::from_secs(2));
        let url = format!("{base}/api/machines/{machine_id}/ask/{chat_id}?after={}", encode(&after));
        let v: Value = match agent.get(&url).set("Authorization", &auth).call().map_err(|e| error_text(e, base)).and_then(|r| r.into_json().map_err(|e| e.to_string())) {
            Ok(v) => v,
            Err(_) => {
                if failures % 10 == 0 {
                    eprintln!("(connection problem, retrying)");
                }
                failures += 1;
                continue;
            }
        };
        for m in v["messages"].as_array().into_iter().flatten() {
            println!("\n{}", m["text"].as_str().unwrap_or(""));
            if let Some(at) = m["at"].as_str() {
                after = at.to_string();
            }
        }
        let pending: Vec<String> = v["pending"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_string)).collect();
        for p in &pending {
            if !shown.contains(p) {
                println!("⏳ Waiting for your approval in Kompanion: {p}");
                shown.push(p.clone());
            }
        }
        if v["running"] == Value::Bool(false) && pending.is_empty() {
            return Ok(());
        }
    }
    Err("Gave up waiting after an hour.".into())
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encodes_the_time() {
        assert_eq!(encode("2026-10-04T02:00:00+00:00"), "2026-10-04T02%3A00%3A00%2B00%3A00");
    }
}
