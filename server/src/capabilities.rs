use axum::{Extension, Json, extract::{Query, State}};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::time::{timeout, Duration};
use futures::future::join_all;

use crate::{AppState, auth::User, error::{ApiError, ApiResult}, llm, pcagent, util};

/// Returns the directory containing skill files.
fn skills_dir() -> PathBuf {
    std::env::var("KOMPANION_SKILLS")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/app/skills"))
}

/// Maps a tool name to its capability description.
fn needs(tool: &str) -> &'static str {
    match tool {
        "read_file" | "list_dir" => "read on the folder",
        "write_file" | "edit_file" => "write on the folder",
        "shell" => "shell in the folder",
        "service" => "system: services",
        "package" => "system: packages (+ root to install)",
        "reload" => "system: desktop",
        "system_info" | "capabilities" => "nothing",
        _ => "",
    }
}

/// Every configured provider, probed now (5 s at most), with its models, the roles that use
/// it and the newest failed call.
pub async fn models(s: &AppState, user_id: &str) -> ApiResult<Vec<Value>> {
    let roles = crate::api::user_roles(s, user_id).await?;
    let probes = s.config.providers.iter().map(|p| async move {
        match timeout(Duration::from_secs(5), llm::list_models(&s.http, p)).await {
            Ok(Ok(models)) => ("ok", None, models.into_iter().map(|m| m.id).collect::<Vec<_>>()),
            Ok(Err(e)) => ("down", Some(format!("{e:#}")), Vec::new()),
            Err(_) => ("down", Some("no answer in 5 s".to_string()), Vec::new()),
        }
    });
    let results = join_all(probes).await;
    let mut output = Vec::new();
    for (p, (status, error, model_ids)) in s.config.providers.iter().zip(results) {
        let last: Option<(String, String)> = sqlx::query_as(
            "SELECT error, at FROM calls WHERE user_id = ? AND provider_id = ? AND error IS NOT NULL ORDER BY at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(&p.id)
        .fetch_optional(&s.db)
        .await?;
        let used_by: Vec<String> =
            roles.iter().filter(|r| r.provider_id == p.id).map(|r| format!("{}: {}", r.role, r.model_id)).collect();
        output.push(json!({
            "id": p.id,
            "name": p.name,
            "local": p.local,
            "status": status,
            "error": error,
            "models": model_ids,
            "roles": used_by,
            "lastError": last.map(|(text, at)| json!({ "text": text, "at": at })),
        }));
    }
    Ok(output)
}

/// Lists computers accessible by the user.
pub async fn computers(s: &AppState, user_id: &str) -> ApiResult<Vec<Value>> {
    let recent = util::minutes_ago(3);
    
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, last_seen FROM machines WHERE user_id = ? ORDER BY name"
    )
    .bind(user_id)
    .fetch_all(&s.db)
    .await?;

    let mut output = Vec::new();
    for (id, name, last_seen) in rows {
        let online = last_seen.as_deref().is_some_and(|t| t > recent.as_str());
        
        let grants_rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT target, rights, expires FROM machine_grants WHERE machine_id = ? ORDER BY target"
        )
        .bind(&id)
        .fetch_all(&s.db)
        .await?;

        let grants = grants_rows.into_iter().map(|(target, rights_str, expires)| {
            let rights: Vec<String> = serde_json::from_str(&rights_str).unwrap_or_default();
            json!({
                "target": target,
                "rights": rights,
                "expires": expires.unwrap_or_default()
            })
        }).collect::<Vec<_>>();

        output.push(json!({
            "id": id,
            "name": name,
            "online": online,
            "lastSeen": last_seen.unwrap_or_default(),
            "runnerVersion": s.host.runner_version(&id),
            "grants": grants
        }));
    }

    Ok(output)
}

/// Returns tool schemas with capability descriptions.
pub fn tools() -> Vec<Value> {
    let schema = pcagent::tools::schema();
    let array = schema.as_array().expect("schema must be an array");
    
    array.iter().map(|item| {
        let obj = item.as_object().expect("item must be an object");
        let name = obj.get("function").and_then(|f| f.as_object().and_then(|fn_obj| fn_obj.get("name"))).and_then(|n| n.as_str()).unwrap_or("").to_string();
        let description = obj.get("function").and_then(|f| f.as_object().and_then(|fn_obj| fn_obj.get("description"))).and_then(|d| d.as_str()).unwrap_or("").to_string();
        
        json!({
            "name": name,
            "description": description,
            "needs": needs(&name)
        })
    }).collect()
}

/// Index status for asset search.
pub async fn indexes(s: &AppState) -> Vec<Value> {
    let (total,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM asset WHERE is_meta = 0 AND dup_of IS NULL")
        .fetch_one(&s.db)
        .await
        .unwrap_or((0,));
    
    let (done,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM asset WHERE is_meta = 0 AND dup_of IS NULL AND ai_state = 'ok'")
        .fetch_one(&s.db)
        .await
        .unwrap_or((0,));
    
    let (failed,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM asset WHERE is_meta = 0 AND dup_of IS NULL AND ai_state = 'error'")
        .fetch_one(&s.db)
        .await
        .unwrap_or((0,));

    let status = if total == 0 { "empty" } else if failed > 0 { "partly" } else { "ok" };

    vec![json!({
        "id": "assets",
        "name": "Asset search by meaning",
        "items": done,
        "of": total,
        "failed": failed,
        "model": "Embedder (ovms-cpu)",
        "status": status
    })]
}

/// Every SKILL.md under the skills folder: id (its folder, e.g. "worker/rust"), title,
/// number of lessons and when it last changed.
pub fn skills() -> Vec<Value> {
    let root = skills_dir();
    let mut files = Vec::new();
    let mut todo = vec![root.clone()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else if path.file_name().is_some_and(|n| n == "SKILL.md") {
                files.push(path);
            }
        }
    }
    let mut output: Vec<Value> = files
        .into_iter()
        .filter_map(|path| {
            let folder = path.parent()?.strip_prefix(&root).ok()?;
            let id = folder.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            if id.is_empty() {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            let title = content.lines().find_map(|l| l.strip_prefix("# ")).map(str::trim).unwrap_or(&id).to_string();
            let lessons = lessons(&content);
            let updated = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| OffsetDateTime::from(t).format(&Rfc3339).ok())
                .unwrap_or_default();
            Some(json!({ "id": id, "title": title, "lessons": lessons, "updated": updated }))
        })
        .collect();
    output.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    output
}

/// The skill files use three styles: "12. ...", "## 1. Title" and plain "- " bullets.
/// Numbered lessons count when there are any, else the top-level bullets.
fn lessons(content: &str) -> usize {
    let numbered = content.lines().filter(|l| is_numbered(l.trim_start_matches('#').trim_start())).count();
    if numbered > 0 { numbered } else { content.lines().filter(|l| l.starts_with("- ")).count() }
}

fn is_numbered(line: &str) -> bool {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && line[digits..].starts_with(". ")
}

#[derive(Deserialize)]
pub struct SkillQuery {
    pub id: String,
}

pub async fn skill(Query(q): Query<SkillQuery>) -> ApiResult<Json<Value>> {
    if q.id.is_empty() || q.id.starts_with('/') || q.id.contains("..") || q.id.contains('\\') {
        return Err(ApiError::BadRequest("Unknown skill.".into()));
    }

    let path = skills_dir().join(&q.id).join("SKILL.md");
    let text = std::fs::read_to_string(&path).map_err(|_| ApiError::NotFound)?;

    Ok(Json(json!({
        "id": q.id,
        "text": text
    })))
}

pub async fn list(State(s): State<AppState>, Extension(u): Extension<User>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "models": models(&s, &u.id).await?,
        "computers": computers(&s, &u.id).await?,
        "tools": tools(),
        "mcp": [],
        "gpus": crate::gpus::current(&s).await,
        "indexes": indexes(&s).await,
        "skills": skills()
    })))
}

pub async fn summary(s: &AppState, user_id: &str) -> String {
    let models = models(s, user_id).await.ok();
    let computers = computers(s, user_id).await.ok();
    let tools_list = tools();

    let mut lines = Vec::new();

    // Models
    lines.push("Models:".to_string());
    if let Some(m) = models {
        for item in m {
            let name = item["name"].as_str().unwrap_or("unknown");
            let status = item["status"].as_str().unwrap_or("unknown");
            let roles = item["roles"].as_array().map(|r| r.iter().map(|v| v.as_str().unwrap_or("")).collect::<Vec<_>>().join("; ")).unwrap_or_default();
            lines.push(format!("- {}: {}, {}", name, status, roles));
        }
    }

    // Computers
    lines.push("Computers:".to_string());
    if let Some(c) = computers {
        for item in c {
            let name = item["name"].as_str().unwrap_or("unknown");
            let online = item["online"].as_bool().unwrap_or(false);
            let status = if online { "online" } else { "offline" };
            let grants = item["grants"].as_array().map(|g| {
                g.iter().filter_map(|item| {
                    item["target"].as_str().map(|t| {
                        let rights = item["rights"].as_array().map(|r| r.iter().map(|v| v.as_str().unwrap_or("")).collect::<Vec<_>>().join(",")).unwrap_or_default();
                        format!("{} ({})", t, rights)
                    })
                }).collect::<Vec<_>>().join("; ")
            }).filter(|g| !g.is_empty()).unwrap_or_else(|| "none".to_string());
            lines.push(format!("- {}: {}, grants: {}", name, status, grants));
        }
    }

    // Tools
    lines.push(format!("Tools: {}", tools_list.iter().map(|t| t["name"].as_str().unwrap_or("")).collect::<Vec<_>>().join(", ")));

    // Skills
    let skills = skills();
    lines.push(format!("Skills: {}", skills.iter().map(|s| s["id"].as_str().unwrap_or("")).collect::<Vec<_>>().join(", ")));

    lines.join("\n").chars().take(2000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_needs_edit_file() {
        assert_eq!(needs("edit_file"), "write on the folder");
    }

    #[test]
    fn test_needs_system_info() {
        assert_eq!(needs("system_info"), "nothing");
    }

    #[test]
    fn test_tools_has_descriptions() {
        let tools = tools();
        assert_eq!(tools.len(), pcagent::tools::schema().as_array().unwrap().len());
        for tool in &tools {
            let desc = tool["description"].as_str().unwrap_or("");
            assert!(!desc.is_empty(), "Tool {} has empty description", tool["name"]);
        }
    }

    #[test]
    fn test_skills() {
        let temp_dir = env::temp_dir().join(format!("kk-skill-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        unsafe {
            env::set_var("KOMPANION_SKILLS", &temp_dir);
        }

        assert_eq!(lessons("# T\n1. a\n2. b\n- detail\n"), 2);
        assert_eq!(lessons("# T\n## 1. a (2026)\n- detail\n## 2. b\n"), 2);
        assert_eq!(lessons("# T\n- a\n  - nested\n- b\n2026 was\n"), 2);
        let skill_path = temp_dir.join("worker/rust/SKILL.md");
        std::fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        std::fs::write(&skill_path, "# Worker: Rust\n\n1. one\n2. two\n- detail\n3. three\n").unwrap();

        let skills = skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["id"], "worker/rust");
        assert_eq!(skills[0]["title"], "Worker: Rust");
        assert_eq!(skills[0]["lessons"], 3);

        unsafe {
            env::remove_var("KOMPANION_SKILLS");
        }
        
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
