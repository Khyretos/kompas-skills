---
name: worker/rust/axum-api
description: Kompanion server handlers: axum responses and headers, query flags, access checks, model calls, events, mail links, URL allow-lists.
roles: [worker, reviewer]
tags: [axum, api, handler, route, header, headers, access, model, llm, event, broadcast, mail, url, spawn]
paths: ["server/src/api.rs", "server/src/admin.rs", "server/src/access/**", "server/src/taskrun/**"]
---
# Worker: Rust (Kompanion server, runner, machine-stats): axum api

2. (2026-10-03) Background tasks (`tokio::spawn`) must not `unwrap()` database or network results; log with `tracing::warn!` and carry on.
9. (2026-10-03) Model calls: bound the prompt (chat history budget), retry once on a failure before anything streamed, then fall back to another provider and say so in one line.
11. (2026-10-03) Access checks: a special target ("system") must never act as a wildcard for file paths; check every grant and skip expired ones instead of returning on the first expired match. Shell: drain stdout/stderr in threads (a full pipe blocks the child), kill on timeout, report the exit code.

27. axum responses: `([(header::CONTENT_TYPE, "text/plain")], body).into_response()`, where body is a
    `String` or `Vec<u8>`. `(StatusCode::NOT_FOUND, "Not found").into_response()` for errors.
33. Broadcast to every signed-in user with `s.bus.send_all(Event::…)` (user id `"*"`); per-user
    events keep using `s.bus.send(&user_id, …)`.
35. (2026-10-04) Resolve a model role with `api::user_role(&s, user_id, "worker")`: the user's own
    choice, else `[roles]` from kompanion.toml. Never query `user_roles` directly; a fresh account
    has no rows there, so W2 and `kompanion-runner ask` refused to start ("set the orchestrator
    model first") although Settings showed the defaults.
38. (2026-10-05) A query flag like `?download=1` doesn't deserialize into `bool` (serde wants
    "true"/"false"; axum answers 400). Use `Option<String>` and `.is_some()`.
39. Array-of-tuple headers in a response must have one value type: `[(CONTENT_TYPE, "a".to_string()),
    (CONTENT_DISPOSITION, format!(..))]`, not a `&str` next to a `String`.
43. (2026-10-05) Anything a mail links to (logo, images) must be a public route (add it to the
    auth guard's open list) and a PNG/JPEG: Gmail and Outlook show no SVG and send no cookie.
48. (2026-10-05) URL allow-lists: compare the whole prefix (`format!("{}/", server)`), never the host
    part against a full URL; split off `?query` before checking the path's characters. Test with
    look-alike hosts (`ntfy.example.com.evil.com`), `http://`, extra query keys and `/../`.
