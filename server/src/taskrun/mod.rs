//! W2: a task runs by itself on a computer, in a folder: the orchestrator plans,
//! the worker does each step with the computer's tools (steps under a standing
//! grant run without asking, others wait for an approval card), the check
//! command runs, and a reviewer reads the result. Up to 3 fix rounds, then the
//! task needs the user. Everything shows in the task's own chat.
//! (Claude rewrote the loop after two failed model drafts; parse.rs is the model's.)
pub mod parse;

use axum::{Extension, Json, extract::{Path, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, api::{self, RoleAssignment}, auth::User, error::{ApiError, ApiResult}, events::Event, llm, pcagent, util};

const ROUNDS: usize = 3;

#[derive(Deserialize)]
pub struct StartBody {
    pub machine_id: String,
    pub folder: String,
    #[serde(default)]
    pub check: String,
}

struct Run {
    user_id: String,
    task_id: String,
    title: String,
    description: String,
    chat_id: String,
    machine_id: String,
    machine_name: String,
    folder: String,
    check: Option<String>,
    orchestrator: RoleAssignment,
    worker: RoleAssignment,
    reviewer: RoleAssignment,
}

async fn role(s: &AppState, user_id: &str, name: &str) -> ApiResult<Option<RoleAssignment>> {
    api::user_role(s, user_id, name).await
}

/// Starts a task on a computer, in a folder.
pub async fn start(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<StartBody>,
) -> ApiResult<StatusCode> {
    let task: Option<(String, String, String, String)> =
        sqlx::query_as("SELECT title, description, project_id, state FROM tasks WHERE id = ? AND user_id = ?")
            .bind(&id)
            .bind(&u.id)
            .fetch_optional(&s.db)
            .await?;
    let Some((title, description, project_id, state)) = task else { return Err(ApiError::NotFound) };
    if state == "running" {
        return Err(ApiError::BadRequest("This task is already running.".into()));
    }
    if description.trim().is_empty() {
        return Err(ApiError::BadRequest("Write what the task should do first (goal, steps, done when).".into()));
    }
    let machine: Option<(String,)> = sqlx::query_as("SELECT name FROM machines WHERE id = ? AND user_id = ?")
        .bind(&b.machine_id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    let Some((machine_name,)) = machine else { return Err(ApiError::NotFound) };
    let folder = b.folder.trim().trim_end_matches('/').to_string();
    if !folder.starts_with('/') || folder.split('/').any(|c| c == "..") {
        return Err(ApiError::BadRequest("Use an absolute folder.".into()));
    }
    let Some(orchestrator) = role(&s, &u.id, "orchestrator").await? else {
        return Err(ApiError::BadRequest("Set the orchestrator model first.".into()));
    };
    let worker = role(&s, &u.id, "worker").await?.unwrap_or_else(|| orchestrator.clone());
    let reviewer = role(&s, &u.id, "reviewer").await?.unwrap_or_else(|| orchestrator.clone());

    // The task's own chat, in its project.
    let chat_title = format!("Task: {title}");
    let chat: Option<(String,)> = sqlx::query_as("SELECT id FROM chats WHERE user_id = ? AND project_id = ? AND title = ? LIMIT 1")
        .bind(&u.id)
        .bind(&project_id)
        .bind(&chat_title)
        .fetch_optional(&s.db)
        .await?;
    let chat_id = match chat {
        Some((c,)) => c,
        None => {
            let c = util::new_id();
            sqlx::query("INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES (?, ?, ?, ?, ?)")
                .bind(&c)
                .bind(&project_id)
                .bind(&chat_title)
                .bind(util::now())
                .bind(&u.id)
                .execute(&s.db)
                .await?;
            c
        }
    };
    let check = Some(b.check.trim().to_string()).filter(|c| !c.is_empty());
    sqlx::query(
        "UPDATE tasks SET machine_id = ?, folder = ?, check_cmd = ?, chat_id = ?, state = 'running', progress = 0,
         step = 'planning', updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&b.machine_id)
    .bind(&folder)
    .bind(&check)
    .bind(&chat_id)
    .bind(util::now())
    .bind(&id)
    .bind(&u.id)
    .execute(&s.db)
    .await?;
    s.bus.send(&u.id, Event::Changed { what: "tasks", machine_id: None });
    s.bus.send(&u.id, Event::Changed { what: "chats", machine_id: None });

    let r = Run {
        user_id: u.id.clone(), task_id: id, title, description, chat_id, machine_id: b.machine_id,
        machine_name, folder, check, orchestrator, worker, reviewer,
    };
    tokio::spawn(run(s.clone(), r));
    Ok(StatusCode::ACCEPTED)
}

async fn note(s: &AppState, r: &Run, text: &str) {
    match api::insert_message(s, &r.chat_id, "orchestrator", text).await {
        Ok(message) => s.bus.send(&r.user_id, Event::Message { message }),
        Err(e) => tracing::warn!("task note: {e}"),
    }
}

async fn progress(s: &AppState, r: &Run, progress: f64, step: &str) {
    let _ = sqlx::query("UPDATE tasks SET progress = ?, step = ?, updated_at = ? WHERE id = ?")
        .bind(progress)
        .bind(step)
        .bind(util::now())
        .bind(&r.task_id)
        .execute(&s.db)
        .await;
    s.bus.send(&r.user_id, Event::Changed { what: "tasks", machine_id: None });
}

async fn finish(s: &AppState, r: &Run, state: &str, step: &str) {
    let _ = sqlx::query(
        "UPDATE tasks SET state = ?, step = ?, progress = CASE WHEN ? = 'done' THEN 1.0 ELSE progress END,
         updated_at = ? WHERE id = ?",
    )
    .bind(state)
    .bind(step)
    .bind(state)
    .bind(util::now())
    .bind(&r.task_id)
    .execute(&s.db)
    .await;
    s.bus.send(&r.user_id, Event::Changed { what: "tasks", machine_id: None });
    crate::notify::task_changed(s.db.clone(), r.user_id.clone(), r.title.clone(), "running".into(), state.into());
}

/// One answer from a model, without tools (plans and reviews).
async fn ask_model(s: &AppState, role: &RoleAssignment, system: &str, user: &str) -> Result<String, String> {
    let Some(p) = s.config.provider(&role.provider_id) else {
        return Err(format!("The provider {} is not configured.", role.provider_id));
    };
    let messages = [json!({ "role": "system", "content": system }), json!({ "role": "user", "content": user })];
    let msg = llm::chat_with_tools(&s.http, p, &role.model_id, &messages, &json!([]))
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(msg["content"].as_str().unwrap_or("").to_string())
}

fn worker_prompt(r: &Run) -> String {
    format!(
        "You are Kreative Kompanion's worker on {}. Work only inside {}. Use the tools; steps outside your \
         grants wait for the user's approval. Make the smallest change that does the step, look before you \
         change, and never create files the task doesn't need. When the step is done, answer with one short \
         line saying what you did.",
        r.machine_name, r.folder
    )
}

fn agent(s: &AppState, r: &Run, max_steps: usize) -> pcagent::Agent {
    pcagent::Agent {
        s: s.clone(),
        user_id: r.user_id.clone(),
        chat_id: r.chat_id.clone(),
        machine_id: r.machine_id.clone(),
        role: r.worker.clone(),
        auto: true,
        max_steps,
    }
}

/// Runs the worker on one instruction: Ok(its last line) or Err(why it stopped).
async fn work(s: &AppState, r: &Run, max_steps: usize, instruction: String) -> Result<String, String> {
    agent(s, r, max_steps)
        .run(vec![json!({ "role": "system", "content": worker_prompt(r) }), json!({ "role": "user", "content": instruction })])
        .await
}

/// The worker's instruction for one step: only that step, stopping once its "done when" holds.
fn step_instruction(task: &str, i: usize, n: usize, step: &parse::PlanStep, plan_list: &str) -> String {
    let task = cut(task, 3000);
    let done = if step.done_when.is_empty() { String::new() } else { format!("\nDone when: {}", step.done_when) };
    format!(
        "The task, for context:\n{}\n\nStep {} of {n}: {}{done}\n\nThe whole plan, for context only:\n{plan_list}\n\nDo only step {}; the other steps are done \
         separately. Stop as soon as it is done: don't run the tests or re-check it again. If it is already done, say so \
         and change nothing.",
        task,
        i + 1,
        step.what,
        i + 1
    )
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

async fn run(s: AppState, r: Run) {
    // 1. Plan.
    let plan_user = format!(
        "Task: {}\n\n{}\n\nWork in the folder {} on {}.\n\nWhat Kompanion has (for planning only):\n{}",
        r.title,
        r.description,
        r.folder,
        r.machine_name,
        crate::capabilities::summary(&s, &r.user_id).await
    );
    let plan_text = match ask_model(
        &s,
        &r.orchestrator,
        "You plan coding and admin tasks on the user's computer. Answer only with a JSON array of 1 to 6 steps, each \
         {\"step\": \"...\", \"done_when\": \"...\"}. A step is one change the user would notice (\"add char_count \
         to textutil.py\"), never only opening, reading or finding something, and never running the tests or the \
         check: that runs by itself afterwards. done_when is one fact the worker can see, such as \"textutil.py \
         defines char_count\". A small task is one or two steps.",
        &plan_user,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            note(&s, &r, &format!("Planning failed: {e}")).await;
            return finish(&s, &r, "needs_input", "planning failed").await;
        }
    };
    let steps = parse::plan(&plan_text);
    if steps.is_empty() {
        note(&s, &r, "I couldn't make a plan; the task description may need more detail.").await;
        return finish(&s, &r, "needs_input", "no plan").await;
    }
    let plan_list = steps
        .iter()
        .enumerate()
        .map(|(i, x)| if x.done_when.is_empty() { format!("{}. {}", i + 1, x.what) } else { format!("{}. {} (done when: {})", i + 1, x.what, x.done_when) })
        .collect::<Vec<_>>()
        .join("\n");
    note(&s, &r, &format!("Plan:\n{plan_list}")).await;

    // 2. The steps.
    let n = steps.len();
    for (i, step) in steps.iter().enumerate() {
        progress(&s, &r, i as f64 / n as f64 * 0.8, &format!("Step {}/{n}: {}", i + 1, step.what)).await;
        match work(&s, &r, 12, step_instruction(&r.description, i, n, step, &plan_list)).await {
            Ok(line) => note(&s, &r, &format!("Step {}: {line}", i + 1)).await,
            Err(why) => {
                if pcagent::hit_step_limit(&why) {
                    note(&s, &r, &format!("Step {} used all its tool calls; the check decides.", i + 1)).await;
                    continue;
                }
                note(&s, &r, &format!("Step {} stopped: {why}", i + 1)).await;
                return finish(&s, &r, "needs_input", &format!("step {} needs you", i + 1)).await;
            }
        }
    }

    // 3. Check and review, with fix rounds.
    for round in 1..=ROUNDS {
        progress(&s, &r, 0.85, &format!("review, round {round}")).await;
        let check_text = match &r.check {
            Some(cmd) => work(&s, &r, 3, format!("Run exactly this check in {} with the shell tool and report the full result: `{cmd}`", r.folder))
                .await
                .unwrap_or_else(|e| format!("The check could not run: {e}")),
            None => "(no check command)".to_string(),
        };
        let diff_text = work(
            &s,
            &r,
            3,
            format!("Show the changes: run `git -C {0} diff --stat` and `git -C {0} diff` with the shell tool and report them.", r.folder),
        )
        .await
        .unwrap_or_else(|_| "(no git diff)".to_string());
        let review_user = format!(
            "Task: {}\n\n{}\n\nCheck result:\n{}\n\nChanges:\n{}",
            r.title,
            cut(&r.description, 6000),
            cut(&check_text, 6000),
            cut(&diff_text, 6000)
        );
        let review = match ask_model(
            &s,
            &r.reviewer,
            "You review a finished task. Answer only with JSON: {\"ok\": true|false, \"findings\": [\"...\"]}. \
             ok only when the check passed and the changes do what the task asks, nothing more.",
            &review_user,
        )
        .await
        {
            Ok(t) => parse::review(&t),
            Err(e) => {
                note(&s, &r, &format!("The review failed: {e}")).await;
                return finish(&s, &r, "needs_input", "review failed").await;
            }
        };
        if review.ok {
            note(&s, &r, "Review: looks good.").await;
            return finish(&s, &r, "done", &format!("done after {round} round(s)")).await;
        }
        let findings = review.findings.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n");
        note(&s, &r, &format!("Review, round {round}:\n{findings}")).await;
        if round == ROUNDS {
            note(&s, &r, "Still not right after 3 rounds; it needs you.").await;
            return finish(&s, &r, "needs_input", "review failed 3 times").await;
        }
        match work(&s, &r, 12, format!("The task:\n{}\n\nFix these review findings, then answer with one short line:\n{findings}", cut(&r.description, 3000))).await {
            Ok(line) => note(&s, &r, &format!("Fix {round}: {line}")).await,
            Err(why) => {
                if pcagent::hit_step_limit(&why) {
                    note(&s, &r, &format!("Fix {round} used all its tool calls; checking again.")).await;
                    continue;
                }
                note(&s, &r, &format!("The fix stopped: {why}")).await;
                return finish(&s, &r, "needs_input", "fix needs you").await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role() -> RoleAssignment {
        RoleAssignment { role: "worker".into(), provider_id: "p".into(), model_id: "m".into() }
    }

    #[test]
    fn worker_prompt_names_the_folder_and_machine() {
        let r = Run {
            user_id: "u".into(), task_id: "t".into(), title: "T".into(), description: "D".into(), chat_id: "c".into(),
            machine_id: "m".into(), machine_name: "soucouyant".into(), folder: "/home/k/app".into(), check: None,
            orchestrator: role(), worker: role(), reviewer: role(),
        };
        let p = worker_prompt(&r);
        assert!(p.contains("/home/k/app") && p.contains("soucouyant"));
    }

    #[test]
    fn a_step_instruction_names_only_its_step_and_its_done_when() {
        let step = parse::PlanStep { what: "add char_count".into(), done_when: "textutil.py defines char_count".into() };
        let t = step_instruction("T", 0, 2, &step, "1. add char_count\n2. add a test");
        assert!(t.starts_with("The task, for context:\nT\n\n"));
        assert!(t.contains("Step 1 of 2: add char_count\nDone when: textutil.py defines char_count"));
        assert!(t.contains("Do only step 1"));
        let bare = parse::PlanStep { what: "x".into(), done_when: String::new() };
        assert!(!step_instruction("X", 1, 2, &bare, "").contains("Done when"));
    }
}
