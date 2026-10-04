//! `kompanion-runner ask "..."`: asks Kompanion from this computer's terminal. The
//! answer may use this computer's tools; each step is approved in the Kompanion web app.
use std::{thread, time::Duration};

use serde_json::json;
use ureq::AgentBuilder;

pub fn ask(server: &str, machine_id: &str, token: &str, question: &str) -> Result<(), String> {
    let agent = AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();

    let base = server.trim_end_matches('/');

    // Step 2: Initial POST request
    let mut resp = match agent
        .post(format!("{base}/api/machines/{machine_id}/ask"))
        .header("Authorization", format!("Bearer {token}"))
        .header("X-Kompanion", "1")
        .send_json(&json!({"text": question}))
    {
        Ok(r) => r,
        Err(e) => return Err(format!("can't reach {base}: {e}")),
    };

    let body: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;

    if let Some(err_val) = body.get("error").and_then(|v| v.as_str()) {
        return Err(err_val.to_string());
    }

    let chat_id = body["chatId"]
        .as_str()
        .ok_or("Missing chatId in response")?
        .to_string();
    let after = body["after"]
        .as_str()
        .ok_or("Missing 'after' field in response")?
        .to_string();

    // Step 3: Polling loop
    let max_polls = 1800;
    let mut polls = 0;
    let mut pending_shown: Vec<String> = Vec::new();
    let mut last_error_count = 0;
    let mut running = true;

    while running && polls < max_polls {
        thread::sleep(Duration::from_secs(2));
        polls += 1;

        let encoded_after = after.replace('+', "%2B").replace(':', "%3A");
        let url = format!(
            "{base}/api/machines/{machine_id}/ask/{}?after={}",
            chat_id, encoded_after
        );

        let resp = match agent.get(&url).header("Authorization", format!("Bearer {token}")).call() {
            Ok(r) => r,
            Err(_) => {
                last_error_count += 1;
                if last_error_count >= 10 {
                    println!("(connection problem, retrying)");
                    last_error_count = 0;
                }
                continue;
            }
        };

        let body: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;

        if let Some(msgs) = body.get("messages").and_then(|v| v.as_array()) {
            for item in msgs {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    println!("\n{text}");
                }
                if let Some(at) = item.get("at").and_then(|v| v.as_str()) {
                    after = at.to_string();
                }
            }
        }

        if let Some(pending) = body.get("pending").and_then(|v| v.as_array()) {
            for item in pending {
                if let Some(summary) = item.get("summary").and_then(|v| v.as_str()) {
                    if !pending_shown.contains(summary) {
                        println!("⏳ Waiting for your approval in Kompanion: {summary}");
                        pending_shown.push(summary.to_string());
                    }
                }
            }
        }

        if let Some(is_running) = body.get("running").and_then(|v| v.as_bool()) {
            running = *is_running;
        }

        if !running && pending_shown.is_empty() {
            break;
        }
    }

    if running {
        Err("Gave up waiting after an hour.".to_string())
    } else {
        Ok(())
    }
}
