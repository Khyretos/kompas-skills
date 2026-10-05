# Areas of the Kompanion server

The server is one binary, but it is built from areas. Each area keeps its own routes, tables and background jobs. An area may call another area only through the functions listed in its row, never into its internals. Background jobs run under `util::supervise`, which restarts a job that crashed (after 5 s, doubling to 5 minutes). Optional areas can be switched off in `[features]` in kompanion.toml: they then have no routes, no background jobs and no menu item.

## The areas

| Area | Code | Routes (under /api) | Background jobs | Switch | Uses |
|---|---|---|---|---|---|
| Sign-in and accounts | `auth.rs`, `oidc.rs`, `admin.rs` | `/status`, `/setup`, `/login`, `/logout`, `/auth/oidc/*`, `/admin/*`, `/me/*`, `/theme.css`, `/logo` | none | always on | - |
| Chat and roles | `api.rs`, `llm.rs`, `live.rs`, `events.rs` | `/chats/*`, `/providers`, `/roles`, `/calls`, `/events` | none | always on | Sign-in |
| Projects and tasks | `tasks.rs`, `projects.rs`, `import.rs`, `windshift.rs` | `/projects/*`, `/tasks/*` | import watcher (`import-watch`), Windshift sync (`windshift-sync`) | Windshift: `windshift` | Chat (orchestrator messages) |
| Task runs on computers | `taskrun/`, `runs.rs`, `pcagent/`, `access/`, `folders.rs` | `/tasks/{id}/start`, `/tasks/{id}/stop`, `/runs/*`, `/actions/*`, `/machines/{id}/grants`, `/machines/{id}/jobs`, `/access`, `/activity` | none (runs are started by requests) | always on | Chat, Machines |
| Machines and runners | `hoststats.rs`, `pairing.rs`, `cli.rs` | `/machines/*`, `/pair`, `/install.sh`, `/download/*` (outside /api) | machine stats (`machine-stats`) | always on | - |
| Notifications | `notify.rs`, `mail.rs`, `mailhtml.rs`, `push.rs` | `/me/notifications`, `/push/register` | daily summary (`daily-summary`) | push only with `[push] servers` | Tasks (state changes) |
| Search and capabilities | `search.rs`, `capabilities.rs` | `/search`, `/capabilities*` | none | always on | all areas (read only) |
| Asset library | `assets/` | `/assets/*`, `/projects/{id}/assets`, `/asset-file/*`, `/asset-preview/*` | scan, previews, AI tags, game discovery (`asset-games`) | `assets` (also needs ASSET_LIBRARY) | Notifications (none), GPUs (asset jobs) |
| GPU ledger and scheduler | `gpus/` | `/gpus`, `/gpus/jobs`, `/gpus/role`, `/gpus/timeline` | GPU sampler and scheduler (`gpus`) | `gpus` (also needs `[[gpu]]`) | Assets (AI tagging jobs) |
| Voice | `voice.rs` | `/voice`, `/voice/transcribe`, `/voice/speak` | none | `voice` (also needs `[voice]`) | - |

## Rules for new code

- A new area gets its own module with a `routes()` function and, if it has background jobs, starts them with `util::supervise("name", ...)`.
- Optional areas get a switch in `[features]` (`config::FeaturesConfig`), checked in `main.rs` for routes and jobs, and reported in `/api/status` so the web app can hide them.
- Unknown `/api` paths answer 404; never let them fall through to the web app.
- Add the area to the table above in the same pull request.
