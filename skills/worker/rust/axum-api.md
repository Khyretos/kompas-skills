---
extends: worker/rust/axum-api
---
35. (2026-10-04) Resolve a model role with `api::user_role(&s, user_id, "worker")`: the user's own
    choice, else `[roles]` from kompanion.toml. Never query `user_roles` directly; a fresh account
    has no rows there, so W2 and `kompanion-runner ask` refused to start ("set the orchestrator
    model first") although Settings showed the defaults.
38. (2026-10-05) A query flag like `?download=1` doesn't deserialize into `bool` (serde wants
    "true"/"false"; axum answers 400). Use `Option<String>` and `.is_some()`.
