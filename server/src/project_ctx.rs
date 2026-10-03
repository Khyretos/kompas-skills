//! What a project chat knows: the project and its tasks (drafted by qwen3:14b).

/// Context for a chat that belongs to a project: the project and its tasks, within `budget` characters.
pub async fn project_context(db: &sqlx::SqlitePool, project_id: &str, user_id: &str, budget: usize) -> sqlx::Result<String> {
    let project: Option<(String, String)> = sqlx::query_as("SELECT name, description FROM projects WHERE id = ? AND user_id = ?")
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;

    let Some((name, description)) = project else {
        return Ok(String::new());
    };

    let unfinished_tasks: Vec<(String, String, String)> = sqlx::query_as("SELECT title, description, state FROM tasks WHERE project_id = ? AND user_id = ? AND state != 'done' ORDER BY position")
        .bind(project_id)
        .bind(user_id)
        .fetch_all(db)
        .await?;

    let finished_tasks: Vec<(String, String, String)> = sqlx::query_as("SELECT title, description, state FROM tasks WHERE project_id = ? AND user_id = ? AND state = 'done' ORDER BY position")
        .bind(project_id)
        .bind(user_id)
        .fetch_all(db)
        .await?;

    let mut tasks = Vec::new();
    tasks.extend(unfinished_tasks);
    tasks.extend(finished_tasks);

    let mut result = format!("Project: {}\n{}\n\nTasks ({}, by order):\n", name, description, tasks.len());

    let mut total_chars = result.len();
    let mut task_count = 0;

    for (title, task_description, state) in &tasks {
        let truncated_description = task_description.chars().take(300).collect::<String>().replace('\n', " ");
        let line = format!("- [{state}] {title}\n  {truncated_description}\n");
        let line_len = line.len();

        if total_chars + line_len > budget {
            break;
        }

        result.push_str(&line);
        total_chars += line_len;
        task_count += 1;
    }

    if task_count < tasks.len() {
        result.push_str(&format!("... and {} more tasks", tasks.len() - task_count));
    }

    Ok(result)
}
