//! Tells a user's other tabs that something changed: after every successful
//! write request, one `Event::Changed` goes to that user's live event stream.
use axum::{extract::{Request, State}, http::Method, middleware::Next, response::Response};
use crate::{AppState, auth::User, events::Event};

pub fn what_changed(method: &Method, path: &str) -> Option<(&'static str, Option<String>)> {
    // Ignore GET, HEAD, OPTIONS
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return None;
    }

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    
    match segments.as_slice() {
        ["tasks", ..] => Some(("tasks", None)),
        ["projects", ..] => Some(("projects", None)),
        ["chats", ..] => Some(("chats", None)),
        ["lessons", ..] => Some(("lessons", None)),
        ["machines", id, "grants", ..] => Some(("access", Some(id.to_string()))),
        ["machines", _, "stats" | "results" | "jobs", ..] => None,
        ["machines", "live"] => None,
        ["machines"] | ["machines", _] => Some(("machines", None)),
        ["admin", ..] | ["me", ..] | ["roles"] => Some(("settings", None)),
        _ => None,
    }
}

pub async fn notify_changes(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let change = what_changed(req.method(), req.uri().path());
    let user_id = req.extensions().get::<User>().map(|u| u.id.clone());

    let res = next.run(req).await;

    if res.status().is_success()
        && let (Some((what, machine_id)), Some(user_id)) = (change, user_id)
    {
        s.bus.send(&user_id, Event::Changed { what, machine_id });
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;

    #[test]
    fn test_what_changed_patch_tasks() {
        let result = what_changed(&Method::PATCH, "/tasks/1");
        assert_eq!(result, Some(("tasks", None)));
    }

    #[test]
    fn test_what_changed_post_machines_grants() {
        let result = what_changed(&Method::POST, "/machines/m1/grants/revoke");
        assert_eq!(result, Some(("access", Some("m1".to_string()))));
    }

    #[test]
    fn test_what_changed_post_machines_stats() {
        let result = what_changed(&Method::POST, "/machines/m1/stats");
        assert_eq!(result, None);
    }

    #[test]
    fn test_what_changed_get_tasks() {
        let result = what_changed(&Method::GET, "/tasks");
        assert_eq!(result, None);
    }

    #[test]
    fn test_what_changed_put_me_prefs() {
        let result = what_changed(&Method::PUT, "/me/prefs");
        assert_eq!(result, Some(("settings", None)));
    }

    #[test]
    fn test_what_changed_delete_machines() {
        let result = what_changed(&Method::DELETE, "/machines/m1");
        assert_eq!(result, Some(("machines", None)));
    }

    #[test]
    fn test_what_changed_post_logout() {
        let result = what_changed(&Method::POST, "/logout");
        assert_eq!(result, None);
    }

    #[test]
    fn test_what_changed_post_lessons() {
        assert_eq!(what_changed(&Method::POST, "/lessons/l1"), Some(("lessons", None)));
    }
}
