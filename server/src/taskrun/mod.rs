pub mod parse;

use axum::{Extension, Json, extract::{Path, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, api::{self, RoleAssignment}, auth::User, error::{ApiError, ApiResult}, events::Event, llm, pcagent, util};

#[derive(Deserialize)]
pub struct StartBody {
    pub machine_id: String,
    pub folder: String,
    #[serde(default)]
    pub check: String,
}

pub async fn start(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<StartBody>,
) -> ApiResult<StatusCode> {
    // Fetch task details
    let task_result = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT title, description, project_id, state FROM tasks WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await;

    let task_result = match task_result {
        Ok(Some(row)) => row,
        Ok(None) => return Err(ApiError::new(StatusCode::NOT_FOUND, "Task not found").into()),
        Err(e) => return Err(ApiError::from_sqlx(e).into()),
    };

    let (title, description, project_id, state) = task_result;

    if state == "running" {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "This task is already running.").into());
    }

    if description.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Write what the task should do first (goal, steps, done when).").into());
    }

    // Fetch machine details
    let machine_result = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM machines WHERE id = ? AND user_id = ?"
    )
    .bind(&b.machine_id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await;

    let machine_result = match machine_result {
        Ok(Some(name)) => name,
        Ok(None) => return Err(ApiError::new(StatusCode::NOT_FOUND, "Machine not found or not yours.").into()),
        Err(e) => return Err(ApiError::from_sqlx(e).into()),
    };

    let machine_name = machine_result;

    // Validate folder
    if !b.folder.starts_with('/') || b.folder.contains("..") {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Use an absolute folder.").into());
    }

    // Fetch roles
    let orchestrator = s.config.get_role_assignment(&u.id, "orchestrator", &s.http).await.ok();
    let worker = s.config.get_role_assignment(&u.id, "worker", &s.http).await.ok();
    let reviewer = s.config.get_role_assignment(&u.id, "reviewer", &s.http).await.ok();

    if orchestrator.is_none() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Set the orchestrator model first.").into());
    }

    let orchestrator = orchestrator.unwrap_or_else(|| worker.clone());
    let worker = worker.unwrap_or_else(|| orchestrator.clone());
    let reviewer = reviewer.unwrap_or_else(|| orchestrator.clone());

    // Ensure chat exists
    let chat_result = sqlx::query_as::<_, (i64,)> (
        "SELECT id FROM chats WHERE project_id = ? AND user_id = ? AND title = ?"
    )
    .bind(project_id)
    .bind(&u.id)
    .bind(&format!("Task: {}", title))
    .fetch_optional(&s.db)
    .await;

    let chat_id = match chat_result {
        Ok(Some(row)) => row.0,
        Ok(None) => {
            let now = util::now();
            let insert = sqlx::query(
                "INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(util::uuid())
            .bind(project_id)
            .bind(&format!("Task: {}", title))
            .bind(now)
            .bind(&u.id)
            .execute(&s.db)
            .await
            .expect("Failed to create chat");
            insert.get_ref::<i64, _>(0)
        },
        Err(e) => return Err(ApiError::from_sqlx(e).into()),
    };

    // Update task
    let now = util::now();
    let update = sqlx::query(
        "UPDATE tasks SET machine_id = ?, folder = ?, check_cmd = ?, chat_id = ?, state = ?, progress = ?, step = ?, updated_at = ? WHERE id = ?"
    )
    .bind(&b.machine_id)
    .bind(&b.folder)
    .bind(if b.check.trim().is_empty() { None } else { Some(b.check.trim().to_string()) })
    .bind(chat_id)
    .bind("running")
    .bind(0.0)
    .bind("planning")
    .bind(now)
    .bind(id)
    .execute(&s.db)
    .await
    .expect("Failed to update task");

    // Send events
    s.bus.send(&u.id, Event::Changed { what: "tasks", machine_id: None });
    s.bus.send(&u.id, Event::Changed { what: "chats", machine_id: None });

    // Spawn runner
    tokio::spawn(run(s.clone(), Run {
        user_id: u.id.clone(),
        task_id: id,
        title,
        description,
        chat_id,
        machine_id: b.machine_id.clone(),
        machine_name,
        folder: b.folder.clone(),
        check: if b.check.trim().is_empty() { None } else { Some(b.check.trim().to_string()) },
        orchestrator: orchestrator.unwrap(),
        worker: worker.unwrap(),
        reviewer: reviewer.unwrap(),
    }));

    Ok(StatusCode::ACCEPTED)
}

struct Run {
    user_id: String,
    task_id: i64,
    title: String,
    description: String,
    chat_id: i64,
    machine_id: String,
    machine_name: String,
    folder: String,
    check: Option<String>,
    orchestrator: RoleAssignment,
    worker: RoleAssignment,
    reviewer: RoleAssignment,
}

async fn note(s: &AppState, r: &Run, text: &str) -> ApiResult<()> {
    let msg = api::insert_message(s, &r.chat_id.to_string(), "orchestrator", text).await?;
    s.bus.send(&r.user_id, Event::Message { message: msg });
    Ok(())
}

async fn progress(s: &AppState, r: &Run, progress: f64, step: &str) -> ApiResult<()> {
    let now = util::now();
    sqlx::query(
        "UPDATE tasks SET progress = ?, step = ?, updated_at = ? WHERE id = ?"
    )
    .bind(progress)
    .bind(step)
    .bind(now)
    .bind(r.task_id)
    .execute(&s.db)
    .await?;

    s.bus.send(&r.user_id, Event::Changed { what: "tasks", machine_id: None });
    Ok(())
}

async fn finish(s: &AppState, r: &Run, state: &str, step: &str) -> ApiResult<()> {
    let now = util::now();
    sqlx::query(
        "UPDATE tasks SET state = ?, progress = 1.0, step = ?, updated_at = ? WHERE id = ?"
    )
    .bind(state)
    .bind(step)
    .bind(now)
    .bind(r.task_id)
    .execute(&s.db)
    .await?;

    s.bus.send(&r.user_id, Event::Changed { what: "tasks", machine_id: None });
    crate::notify::task_changed(s.db.clone(), r.user_id.clone(), r.title.clone(), "running".into(), state.into());
    Ok(())
}

fn worker_prompt(r: &Run) -> String {
    format!("You are Kreative Kompanion's worker on {}. Work only inside {}. Use the tools; steps outside your grants wait for the user's approval. Make the smallest change that does the step, look before you change, and never create files the task doesn't need. When the step is done, answer with one short line saying what you did.", r.machine_name, r.folder)
}

fn agent(s: &AppState, r: &Run, max_steps: usize) -> pcagent::Agent {
    pcagent::Agent {
        s: s.clone(),
        user_id: r.user_id.clone(),
        chat_id: r.chat_id.to_string(),
        machine_id: r.machine_id.clone(),
        role: r.worker.clone(),
        auto: true,
        max_steps,
    }
}

async fn run(s: AppState, mut r: Run) {
    // Plan
    let plan_system = "You plan coding and admin tasks on the user's computer. Answer only with a JSON array of 3 to 8 short, concrete steps.";
    let plan_user = format!("Task: {}\n\n{}\n\nWork in the folder {} on {}.", r.title, r.description, r.folder, r.machine_name);
    
    let steps = match parse::plan(&plan_user) {
        Ok(steps) => steps,
        Err(_) => {
            note(&s, &r, "I couldn't make a plan; the task description may need more detail.").await.ok();
            finish(&s, &r, "needs_input", "planning failed").await.ok();
            return;
        }
    };

    note(&s, &r, &format!("Plan created:\n{}", steps.iter().enumerate().map(|(i, x)| format!("{}. {x}", i + 1)).collect::<Vec<_>>().join("\n"))).await.ok();

    let n_steps = steps.len();
    for (i, step) in steps.iter().enumerate() {
        progress(&s, &r, (i as f64) / (n_steps as f64), &format!("Step {}/{}: {}", i+1, n_steps, step)).await.ok();

        let worker_prompt_text = worker_prompt(&r);
        
        let agent_messages = vec![
            json!({ "role": "system", "content": worker_prompt_text }),
            json!({ "role": "user", "content": format!("Step {} of {}: {}\n\nThe whole plan:\n{}", i+1, n_steps, step, steps.iter().enumerate().map(|(j, x)| format!("{}. {x}", j + 1)).collect::<Vec<_>>().join("\n")) }),
        ];

        let agent = agent(&s, &r, 12);

        match agent.run(agent_messages).await {
            Ok(line) => {
                note(&s, &r, &format!("Step {}: {}", i+1, line)).await.ok();
            },
            Err(text) => {
                note(&s, &r, &format!("Step {} failed: {}", i+1, text)).await.ok();
                finish(&s, &r, "needs_input", &format!("Step {} failed", i+1)).await.ok();
                return;
            }
        }
    }

    // Check and review
    let mut round = 1;
    loop {
        let check_text = if let Some(ref cmd) = r.check {
            let check_agent = agent(&s, &r, 3);
            
            let check_messages = vec![
                json!({ "role": "system", "content": "You are a checker. Run exactly this check in the folder with the shell tool and report the full result." }),
                json!({ "role": "user", "content": format!("Run exactly this check in {} with the shell tool and report the full result: {}", r.folder, cmd) }),
            ];
            
            match check_agent.run(check_messages).await {
                Ok(text) => text,
                Err(e) => {
                    note(&s, &r, &format!("Check command failed: {}", e)).await.ok();
                    finish(&s, &r, "needs_input", "review failed").await.ok();
                    return;
                }
            }
        } else {
            "(no check command)".to_string()
        };

        let diff_text = {
            let diff_agent = agent(&s, &r, 3);
            
            let diff_messages = vec![
                json!({ "role": "system", "content": "You are a diff viewer. Show the changes by running git commands and reporting them." }),
                json!({ "role": "user", "content": format!("Show the changes: run `git -C {} diff --stat` and `git -C {} diff` with the shell tool and report them.", r.folder, r.folder) }),
            ];
            
            match diff_agent.run(diff_messages).await {
                Ok(text) => text,
                Err(_) => "(no git diff)".to_string(),
            }
        };

        let safe_task = r.description.chars().take(6000).collect::<String>();
        let safe_check = check_text.chars().take(6000).collect::<String>();
        let safe_diff = diff_text.chars().take(6000).collect::<String>();

        let review_system = "You review a finished task. Answer only with JSON: {\"ok\": true|false, \"findings\": [\"...\"]}. ok only when the check passed and the changes do what the task asks, nothing more.";
        let review_user = format!("Task: {}\n\nCheck result:\n{}\n\nDiff:\n{}", safe_task, safe_check, safe_diff);

        match ask_model(&s, &r.reviewer, review_system, &review_user).await {
            Ok(review_text) => {
                let review = parse::review(&review_text);

                if review.ok {
                    note(&s, &r, "Review: looks good.").await.ok();
                    finish(&s, &r, "done", &format!("done after {} round(s)", round)).await.ok();
                    return;
                } else {
                    let findings = review.findings.join("\n- ");
                    note(&s, &r, &format!("Review findings:\n- {}", findings)).await.ok();
                    
                    if round >= 3 {
                        note(&s, &r, "Still not right after 3 rounds; it needs you.").await.ok();
                        finish(&s, &r, "needs_input", "review failed 3 times").await.ok();
                        return;
                    } else {
                        let fix_system = "You are a fixer. Fix these review findings, then answer with one short line.";
                        let fix_user = format!("Fix these review findings, then answer with one short line:\n- {}", findings);
                        
                        let fix_agent = agent(&s, &r, 12);
                        
                        match fix_agent.run(vec![
                            json!({ "role": "system", "content": fix_system }),
                            json!({ "role": "user", "content": fix_user }),
                        ]).await {
                            Ok(fix_line) => {
                                note(&s, &r, &format!("Fix attempt {}: {}", round, fix_line)).await.ok();
                            },
                            Err(e) => {
                                note(&s, &r, &format!("Fix attempt {} failed: {}", round, e)).await.ok();
                            }
                        }
                        round += 1;
                        continue;
                    }
                }
            },
            Err(e) => {
                note(&s, &r, &format!("Review failed: {}", e)).await.ok();
                finish(&s, &r, "needs_input", "review failed").await.ok();
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn dummy_run() -> Run {
        Run {
            user_id: "test-user".to_string(),
            task_id: 1,
            title: "Test Task".to_string(),
            description: "Test Description".to_string(),
            chat_id: 1,
            machine_id: "test-machine".to_string(),
            machine_name: "test-machine".to_string(),
            folder: "/tmp/test".to_string(),
            check: None,
            orchestrator: RoleAssignment { provider_id: "ollama".to_string(), model_id: "qwen3:14b".to_string() },
            worker: RoleAssignment { provider_id: "ollama".to_string(), model_id: "qwen3.5:9b-q8_0".to_string() },
            reviewer: RoleAssignment { provider_id: "ollama".to_string(), model_id: "qwen3.5:9b-q8_0".to_string() },
        }
    }

    #[test]
    fn test_worker_prompt_contains_folder_and_machine() {
        let r = dummy_run();
        let prompt = worker_prompt(&r);
        assert!(prompt.contains("test-machine"));
        assert!(prompt.contains("/tmp/test"));
    }
}
