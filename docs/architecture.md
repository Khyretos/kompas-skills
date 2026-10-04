# Kreative Kompanion: from-scratch architecture (draft, 2026-10-01)

Goal: a visual app you can open on any device, which plans your projects with a
strong model, hands the heavy lifting to your local model, reviews the result,
shows live progress, and can work on any of your PCs (files, shell,
screenshots, mouse and keyboard). FOSS only, and every OS is covered.

## The three pieces

```
 phone / tablet / any PC                kireserver                     each PC
┌────────────────────┐   HTTPS    ┌──────────────────────┐   WSS    ┌──────────────┐
│ Web app (PWA)      │◄──────────►│ Kompanion server     │◄────────►│ Runner       │
│ vanilla TypeScript │  Keycloak  │ orchestrator, tasks, │ outbound │ files, shell,│
│ chat, task board,  │            │ model routing,       │ from PC  │ screenshots, │
│ approvals, shots   │            │ history (SQLite)     │          │ input        │
└────────────────────┘            └──────────┬───────────┘          └──────────────┘
                                             │ OpenAI-compatible / Anthropic API
                                 OVMS (Qwen), DeepSeek, Claude, Ollama, ...
```

### 1. Server (Rust, one container on kireserver)

- **Why Rust:** one language with the runner, single static binary, strong async
  (tokio + axum), easy cross-compiling. C++ would work, but you'd hand-build what
  axum, serde and tokio give you for free, and none of this is CPU-bound.
- **Model providers:** two adapters cover everything. *OpenAI-compatible*
  (OVMS, DeepSeek, Ollama, llama.cpp, vLLM, OpenRouter) and *Anthropic Messages*.
  Each has a base URL, a key (from `.env`, never from the UI) and a model list.
- **Roles, routed per project:** `planner` (strong: Claude or DeepSeek),
  `worker` (local Qwen), `reviewer` (strong). This is what we have been doing by
  hand: Qwen drafts, the strong model checks. When a better local model arrives
  you change one setting, not code.
- **Task engine:** project → plan → tasks. Each task is a small state machine:
  `queued → running → needs_input | in_review → done | failed`, with an event log
  (model step, tool call, diff, screenshot, question). The UI's progress bar and
  timeline are just this log.
- **Guardrails for a 9B worker** (from your hardening test): tasks must be small
  and self-contained; every worker result goes through the reviewer before it is
  applied; a max-token limit per step; secrets are never put in a prompt or a
  command line; a task that fails review twice escalates to you as a question.
- **Storage:** SQLite (one file, easy to back up with your restic job). Move to
  Postgres only if it ever needs to.
- **Auth:** Keycloak OIDC (realm kreative-kompas) for people. Runners pair with a
  one-time code and get their own revocable device token.
- **Tool protocol:** tools are described as JSON schema, MCP-style, so your
  existing MCP servers (Stack Overflow MCP etc.) plug in, and Kompanion's runner
  tools could be reused by other agents.

### 2. Web app (vanilla TypeScript PWA)

- Vite only as the build tool; no framework. A small `Component` base class, a
  hash router, one store with subscribe, and an `esc()` helper for every piece of
  model or user text (models output HTML-looking text; never `innerHTML` it raw).
- Views: projects, chat with the planner, task board with live progress
  (server-sent events), approval cards, diff viewer, screenshot viewer, settings
  (providers, roles, runners).
- Installable as an app on Android, iOS (home screen; web push works there since
  iOS 16.4), macOS, Windows and Linux. No app store, no Apple developer fee.
- Optional later: a Tauri or Qt shell around the same page for a tray icon. Not
  needed to be useful.

### 3. Runner (Rust, small binary per PC)

- Connects **out** to the server over WebSocket, so no ports open on your PCs and
  it works from anywhere through your existing domain.
- Tools, each limited to folders you allow per device: `list`, `read`, `write`
  (returns a diff, keeps a backup), `search`, `shell` (with a timeout and the whole
  process tree killed on stop), `screenshot`, `click/type/keys`.
- Approval policy per device: read-only free; writes inside allowed folders free
  or asked; shell and input always asked unless you allow a pattern.
- Builds for Linux, Windows and macOS from one codebase (`cargo` cross targets).
  Phones get no runner; they're the remote control.
- **Honest caveat, Wayland:** on your Hyprland PC an app can't grab the screen or
  send input freely. Screenshots go through `grim` or the xdg-desktop-portal
  ScreenCast API, input through the RemoteDesktop portal (or `ydotool`). X11,
  Windows and macOS are simpler (macOS asks once for Screen Recording and
  Accessibility permission). The runner needs a small backend per OS for these
  two tools. If you want C++ anywhere, this is the natural spot; otherwise the
  Rust crates `xcap` and `enigo` cover Windows, macOS and X11.

## Product requirements (Kees, 2026-10-01)

- **One-step deploy.** The server ships as one `docker-compose.yml` (server +
  Postgres with pgvector/BM25 for the RAG index) or as a native installer
  (single binary + service on Linux, Windows, macOS). Keys and settings go in on
  first run, not in config files.
- **The app is separate.** A Kompanion app per device that asks for a link,
  or finds servers on the local network by itself (mDNS/DNS-SD, `_kompanion._tcp`),
  pairing by QR code or code. Built with Tauri 2 around the same vanilla-TS UI, so
  one codebase for Linux, Windows, macOS, Android and iOS. The server also serves
  that UI as a web app, as a fallback and for an iPhone without paying Apple:
  distributing an iOS app outside TestFlight needs the paid Apple developer
  account, and free sideloading expires every 7 days.
- **Works from anywhere, no VPN needed.** The app talks HTTPS/WSS to a public
  link (behind your reverse proxy); runners connect outward. Built-in accounts
  and passkeys by default, OIDC (Keycloak) optional, so other people can run it
  without a Keycloak.
- **Sets itself up.** Point it at an API URL or a local model and it: detects the
  API type (OpenAI-compatible or Anthropic), lists models, measures context size
  and speed, tests tool calling and JSON-schema output, then proposes which model
  takes which role. It also scans the local network and well-known ports for
  Ollama, LM Studio, llama.cpp and OVMS.

### Root on a PC: pkexec from the runner works (tested 2026-10-03)

The runner runs as a systemd user service. On soucouyant (CachyOS, Hyprland, mate-polkit),
a `package paru install cowsay` job ran `paru --sudo pkexec`, and the polkit prompt appeared
on the desktop. Kees typed his password there and the job ended with exit 0. polkit finds
the user's graphical session even though the service is not part of it. The password
never passes through Kompanion. The test grant (system: packages + root) was revoked
afterwards.
Known gap: the runner handles jobs on its polling thread, so it stops sending stats while
a job waits for the prompt. Jobs should get their own thread (W2).

### UI libraries and motion (decision, 2026-10-04)

Kees's order: performance, then usability, then looks. The web app stays vanilla TypeScript.
- No React-based kits (Kokonut UI; bklit ui if it is React/shadcn). manus.im is an AI product,
  not a library: ideas only.
- Motion uses plain CSS transitions (expand/collapse, card enter) and respects
  prefers-reduced-motion.
- A JS animation library is allowed only if CSS can't do the job, it is MIT and tiny (Motion's
  vanilla `animate` mini, or anime.js v4), and the bundle size is measured. Anything that adds
  noticeable weight is rejected.

### PC agent guardrails (after the Hyprland test, 2026-10-04)

- Never create a config or system file the user didn't ask to create. If the target is
  missing, stop, report what is there, and ask. In code: a write_file to a path that a read in
  the same answer found missing is blocked.
- Never route around a refused or declined step with another tool; report it.
- Check the live config format first: system_info reports the home folder, the Hyprland version
  and which config exists (hyprland.lua or hyprland.conf).
- Verify an effect before claiming success (`hyprctl getoption ...`), else say "not verified".
- The runner reports a missing path as "no such file or folder", distinct from "not granted".

### Snappy and live, never a refresh (standing UI rule, Kees 2026-10-03)

The app must feel quick and reactive. Nothing may need a page reload.
1. Every action that changes data (grant, revoke, rename, move chat, settings, task
   edits, pairing, ...) updates the screen at once. It is an optimistic update: on
   failure it rolls back and shows an error toast. Its button is disabled and shows
   a spinner while the request runs.
2. The server pushes changes to each user over one Server-Sent Events stream
   (`/api/events`): grants, machines online/offline, jobs, tasks, chats and settings.
   Other tabs and server-side changes appear live. The stream reconnects with backoff
   and reloads the affected list after a reconnect.
3. Every page gets a Playwright test: do an action, then check the screen changed
   without a reload.

### Layout of the app

Left sidebar: chats and projects. Middle: the conversation with the project's
orchestrator. Right sidebar: running and queued tasks with progress, waiting
questions and approvals, and the latest screenshots.

## The teacher loop (strong model trains the local ones, without new weights)

"Smarter" here means better instructions, skills, examples and tools, not
retraining; that part is realistic and is what actually moves a 9B model.

1. A task fails a check or review → the teacher (strong model) writes a
   **lesson**: what went wrong, the root cause, and a rule or worked example.
2. Lessons are merged into **skills**: versioned Markdown files per domain
   (Docker compose, Keycloak, Rust, kk-engine...) that are retrieved into the
   worker's context when relevant, plus new recipes or tools where a rule is
   better done by code.
3. Every failure also becomes a **test case**. Each night the local model reruns
   the test set; a skill change that doesn't raise the pass rate is reverted, and
   skills nobody uses are pruned, so the skill set doesn't bloat.
4. A dashboard shows per-model pass rates over time, per domain, which also tells
   you when a new local model is worth switching to.
5. Later and optional: the saved good examples are a dataset for a LoRA
   fine-tune when hardware allows (training on the A770 is possible but rough).

Who is the teacher: any strong provider in settings. Claude via the API is paid
per token and separate from a Claude subscription, so the cheap default is
DeepSeek for routine reviews and Claude only for hard ones. Kompanion also
exposes its own MCP server, so a Claude Code session (like this one) can connect,
audit tasks and write lessons, which is how we work today.

### Skills belong to roles, never to models (the child becomes the father)

Long-term goal: step away from proprietary models. So nothing Kompanion learns
may live inside, or depend on, one model or provider.

- **Roles are slots, models are plug-ins.** Roles: `orchestrator` (talks to you,
  plans), `worker`/executor (does the steps), `reviewer`/teacher (checks and
  writes lessons), plus tool skills used by the `runner`. Any model, paid or
  local, can be put in any slot in settings; swapping is a setting, not a
  migration.
- **One skill library, split by role and domain**, in plain Markdown with
  frontmatter in the open Agent Skills (`SKILL.md`) format, kept in its own git
  repo on Forgejo so it is readable, diffable and portable to other agents:

  ```
  skills/
    orchestrator/   planning, splitting work, when to ask you
    worker/         docker-compose/, keycloak/, rust/, kk-engine/ ...
    reviewer/       review rubrics, how to write a lesson, how to grade
    runner/         tool use: shell, files, workspaces, playwright
    shared/         project facts, your preferences (FOSS only, budget...)
    _model-notes/   quirks per model family, kept apart so a swap only drops these
  ```
- **The teacher teaches its own job too.** The reviewer's rubric and the way it
  writes lessons are skills under `reviewer/`, so when a local model takes over
  the teacher slot it inherits how the job is done, not just the results.
- **Promotion exam.** Every role has its own test set built from past tasks and
  failures. Before a model gets a slot (for example a local model replacing
  Claude or DeepSeek as reviewer), it runs that role's tests; it gets the slot
  when it reaches the pass rate you set. The dashboard shows how close each local
  model is to taking over each role.
- **No lock-in in the data either:** lessons, examples and test sets are stored
  as files and Postgres rows with no provider-specific fields, so the history
  stays usable whichever model reads it.

## Seeing everything: calls, reasons, machines, power

Goal: at the end of the day you can see what each model did, with which request,
why, on which machine, and what it cost in tokens, money and watts.

- **Every model call is recorded.** Full request (system prompt, retrieved skills
  and context, messages, tool schema) and full response, tokens in/out, latency,
  model, provider and cost. Secrets are redacted before storage. Shown per task as
  an "Inspect" view, and in a global activity log you can filter by model,
  project, machine and time.
- **Every call has a reason.** Calls are nested as a trace: project → task → step
  → model call → tool call, following the OpenTelemetry GenAI conventions. Each
  step carries the orchestrator's one-line reason ("tests failed, asking the
  worker to fix the include"), so "why" is always one click up from "what".
- **Every tool call is recorded** with the exact command or file diff, who
  approved it (you, a standing rule, or auto), exit code and output.
- **Machines.** Each runner reports CPU, RAM, disk, network, GPU load, VRAM,
  temperature and power draw every few seconds, plus what Kompanion itself is
  using (per workspace container, via cgroups). GPU power comes from the driver:
  Intel Arc through the i915/xe hwmon energy counters (or `xpu-smi`), NVIDIA
  through NVML, AMD through amdgpu hwmon. The server does the same for kireserver
  and reads OVMS's own Prometheus metrics (queue, tokens/s).
- **Energy and cost.** Watts × time per task gives Wh per task and per day; with
  your kWh price that becomes euros, next to the API spend per provider.
- **Views:** a live Machines panel (sparklines per PC and GPU), a per-task cost
  line (tokens, € API, Wh), and a daily report ("today: 41 tasks, 2.1 M local
  tokens, 180k DeepSeek tokens €0.09, A770 1.4 kWh").
- **Open standards out.** The server exposes `/metrics` (Prometheus) and can send
  traces over OTLP, so Grafana, Prometheus or any other FOSS tool can use the same
  data. Retention is configurable (e.g. full prompts 30 days, totals forever).

## Workspaces: containers and VMs instead of your real desktop (added after Kees's idea)

Most work never touches your real Hyprland session. The runner gives the agent a
**workspace**, and only falls back to your desktop when a task truly needs it.

- **Container workspace (default):** rootless Podman or Docker, driven from the
  runner (Rust `bollard` crate for the Docker/Podman API). Builds, tests,
  benchmarks and git work happen here, against a copy or a mounted folder you allow.
- **GUI inside the container:** a headless compositor (Sway or Weston headless,
  or Xvfb for X11 apps) runs inside the workspace. Screenshots and input are then
  plain commands (`grim`, `wtype`/`xdotool`) with no portal and no permission
  prompts, because it is the agent's own screen, not yours.
- **GPU work** (kk-engine, games, 3dco-plus): pass `/dev/dri` into the container
  for Vulkan/OpenGL on Intel/AMD; still isolated from your session.
- **VM workspace (QEMU/KVM via libvirt):** for a full OS: other distros, kernel
  or driver testing. Windows and macOS testing go to the real Windows and Mac
  runners instead (a Windows VM needs a licence; a macOS VM on non-Apple
  hardware breaks Apple's licence).
- **Your real desktop:** only for things that must happen there; the runner
  asks first and you handle anything Wayland blocks.

### Resource consent and scheduling

- Each runner reports what it sees: CPU/GPU/RAM load, and whether you look busy
  (a fullscreen window, Steam/gamemode, a running game, recent input).
- A task that needs a workspace states what it wants ("8 cores, 16 GB, GPU, about
  40 min"). You answer on a card: **Now**, **Tonight** (your night window, e.g.
  01:00–07:00), **At…**, or **With less** (you lower the limits).
- The runner enforces what you granted with cgroup limits (`--cpus`, `--memory`,
  GPU on/off) and pauses or stops the workspace if you start gaming, or when the
  window ends.
- Standing rules you can set per device, e.g. "always OK under 4 cores",
  "never while Steam is running", "night jobs may use everything".

## Tools: MCP and Playwright, connect or build

- **MCP client built in.** Kompanion speaks MCP both ways a server can be
  reached: remote HTTP servers connect to the Kompanion server (your Stack
  Overflow MCP, anything on kireserver), and local stdio servers run through a
  runner, inside a workspace container by default. Tools show up per project
  with an on/off switch and an approval level.
- **Playwright** comes as one of those: the Playwright MCP server (Apache-2.0)
  running in a container workspace with its own headless browser, so web
  testing and screenshots never touch your own browser.
- **Kompanion can make its own tools.** A "tool builder" task: the planner
  specifies a tool, the worker writes a small MCP server, it is built and tested
  in a container, and it is registered only after review and your OK. Over time
  your weak spots become deterministic tools instead of model guesswork.

## Making a small model act smarter (resourceful instead of expensive)

Rule of thumb: let code decide whatever code can decide; the model only fills the
gaps, and every output is checked by something cheaper than the strong model.

1. **Retrieval (RAG) on what you already run.** Postgres with BM25 + pgvector
   (the ParadeDB setup from the Stack Overflow MCP) for hybrid search over each
   project's code, docs and notes, ranked by your CPU Reranker. One index per
   project, refreshed by the runner on file changes.
2. **A code map, not raw files.** tree-sitter builds a symbol map (files,
   functions, call sites) so the worker gets the 2–3 relevant functions instead
   of whole files. Small context is where a 9B model does best.
3. **Structured output.** Tool calls and plans are forced into a JSON schema
   (OVMS/llama.cpp grammar-constrained output; check which your OVMS version
   supports). No free-text commands, so no invented flags.
4. **Recipes over improvisation.** Proven procedures (add a compose service,
   add an OIDC client, bump a dependency) are stored as recipes with parameters.
   The model picks a recipe and fills the blanks; code runs the steps.
5. **Cheap checks first.** Compile, tests, linters, `docker compose config`,
   schema validation and a dry run run before any reviewer model is asked. The
   strong model only sees work that already passes those, which saves tokens.
6. **Learn from success.** Every task that passes review is saved as an example
   (task, context, result). Similar future tasks get it as a few-shot example
   through the same retrieval.
7. **Spend free night compute.** For hard steps the local model makes several
   attempts overnight and the checks pick the one that passes (best-of-n).
8. **Local docs.** Mirror the docs your projects use (DevDocs, man pages, Qt,
   Vulkan, Rust) into the index, so the worker looks up real setting names
   instead of guessing them, which was its main failure in the hardening test.

## Repository layout (one repo, Forgejo main, mirrored to GitHub like your others)

```
kreative-kompanion/
  server/      Rust: axum API, task engine, providers, SQLite migrations
  runner/      Rust: WebSocket client, tools, per-OS capture/input
  protocol/    Rust crate shared by both + generated TypeScript types
  web/         vanilla TypeScript PWA
  deploy/      Dockerfile + compose snippet for ~/Docker
```

The current Kreative-Cowork fork can stay as a reference (Apache-2.0: borrow
prompts and tool ideas, with attribution) or be archived.

## Milestones (each one usable on its own)

1. **Talk to any model** — server + web app: chat with streaming, provider
   settings for OVMS, DeepSeek and Claude, history in SQLite, Keycloak login.
   Already useful: one chat app for all your models on every device.
1b. **Make it yours** (asked 2026-10-01, after first real use):
   - *Account*: sign out (menu under your name, top left), the Kreative Kompas
     logo top left and on the sign-in and SSO buttons.
   - *Chats and projects*: ⋮ menu per chat (pin, rename, archive, delete),
     projects that open and expand.
   - *Admin settings* (admins only, stored in the database, not the config
     file): instance name and logo, who may sign in (`allow_new`, password
     sign-in on/off), users list (make admin, disable), model providers and
     default roles, mail server, default theme. Secrets stay in env.
   - *Mail*: SMTP through your own mailserver (house rule: service mail as
     info@kreative-kompas.com, or a descriptive kompanion@ alias with replies
     to info@). Per-user notification settings: off, digest, or immediately.
     The first events are "a task needs you" and "a task finished or failed";
     they fire for real once tasks run (milestone 3).
   - *Theming*: Kompas Night/Day/system per user; admins set the default theme
     and can override the colour tokens (palette variables) and logo. No
     custom CSS or scripts from users, so the strict CSP stays.
   - *Machines*: kireserver itself shows live CPU, RAM, disk and GPU.
2. **Hands on one PC** — runner on your Linux PC: pairing, allowed folders,
   read/write/shell with approval cards and diffs in the web app.
3. **Plan, delegate, review** (with MCP client, RAG index and cheap checks) — planner splits a project into tasks, Qwen works
   them, the reviewer checks, the task board shows progress, questions come to
   you as cards and notifications.
   Live status everywhere: a spinner and step text on running tasks (task list,
   project list badges, chat task chips, sidebar counts), a progress bar, and
   needs-you/running badges. Driven by task events over SSE, the same way chat
   replies stream today.
4. **Workspaces** — container workspaces with headless GUI (screenshots and
   input there), resource-consent cards and night scheduling; real-desktop
   control per OS after that.
5. **Every machine** — runner builds for Windows and macOS, auto-update,
   scheduled tasks, project memory.

Already in use alongside the milestones: soucouyant's Ollama (qwen3:14b)
reachable by kireserver only, and a Forgejo Actions runner on soucouyant, so
image builds run on the faster PC instead of kireserver.

## Added milestones (owner requests, 2026-10-01)

Already in milestone 1: sign out (sidebar, bottom), the logo top left, chat
menu, expandable projects, the server's own machine in the Machines tab.

### Model choice result (2026-10-03 benchmarks)

| Model | Where | Speed | Tool use | Screen grounding | Text |
|---|---|---|---|---|---|
| Qwen3.5-9B int8 ("Coder") | OVMS, A770 on kireserver | 33 tok/s | 5/6 | 18/20 | good |
| qwen3.5:9b-q8_0 | Ollama, RX 9070 XT on soucouyant | — | 5/6 | 15/20 | 15/15 code quality |
| gemma4:12b-it-qat | Ollama, soucouyant | 68 tok/s | 4/6, loops | 5/20 | equal on text |

Decision (Kees, 2026-10-03): Qwen3.5 9B on both GPUs for code, tools, PC
control and vision; gemma4 only as kk-localize's translation judge (judge runs
move to nights later).
- orchestrator, reviewer, PC-control agent (F6), vision and test-driver → Coder on OVMS
- worker and drafting → qwen3.5:9b-q8_0 on soucouyant (thinking off with `reasoning_effort: "none"`)
- The drafting pipeline uses soucouyant when Ollama has qwen3.5 loaded or
  nothing loaded, and OVMS Coder otherwise, so it never forces a model swap.

### Milestone 1.5 status (2026-10-03)

Done:
- [x] Admins (first account), admin settings in the database, app name.
- [x] Mail: SMTP settings, password only from `SMTP_PASSWORD`, test mail, sender kompanion@ with replies to info@.
- [x] Theming: brand colours with the WCAG AA check, served as `/api/theme.css`; logo upload (PNG/SVG, sandboxed); light/dark/system per user.
- [x] Tasks: descriptions (goal, steps, done when; required), add/edit/reorder/close/delete, history.
- [x] Project sources: internal or windshift; two-way Windshift sync every 5 minutes (env-configured, conflicts kept in history); "stop syncing".
- [x] Notifications by mail: needs you, failed, done, daily summary; address defaults to the account email. (Planned for milestone 3, done early.)
- [x] Machines: this server's stats, refresh slider with Live over SSE, paired PCs through `kompanion-runner` (first slice of milestone 2), GPU telemetry for amdgpu and Intel i915/xe.
- [x] Sign out, chat menu, expandable projects.

Still open in 1.5:
- [x] Admin rights from a Keycloak role (`[oidc] admin_role`, realm or client role), checked at every sign-in.
- [x] Sign out of Keycloak too (OIDC end-session with the session's ID token).
- [x] Tasks made in Kompanion inside a Windshift project are created in Windshift as well.
- [x] Drag to reorder (and move up/down).

### 1.5 Admin and polish (doesn't need tasks, so it comes before them)

- **Admins.** `users.is_admin`; the first account is admin, and an optional
  `[oidc] admin_group` gives admin to members of a Keycloak group (checked on
  every sign-in). Admin-only routes under `/api/admin/*`, refused (403) for
  everyone else; a test per route checks that.
- **General settings** (admin menu): app name, default model roles for new
  users, sign-in options (password on/off, who gets a new account), the
  provider list stays in `kompanion.toml` (no SSRF through the UI). Stored in a
  `settings` key-value table; the config file gives the defaults.
- **Mail (SMTP).** Host, port, TLS mode, user and sender in `settings`; the
  password only from an environment variable (`SMTP_PASSWORD`), never in the
  database or the UI. Sent with `lettre` (MIT/Apache-2.0) to the existing
  docker-mailserver. A "Send test mail" button. Mails carry titles and links,
  never prompts, model output, keys or file contents.
- **Theming.** Admin sets brand colours and uploads a logo (SVG or PNG, size
  limit, SVGs sanitised and served as images, never inline); default is the
  Kreative Kompas palette. Saving checks every text/background pair the app
  uses and refuses anything under 4.5:1 (3:1 for large text), computed in the
  server with the WCAG formula (no extra crate). Users choose light, dark or
  system (`users.theme`).
- **Tests:** contrast function against known pairs; admin guard; settings
  round-trip; test mail against a local SMTP sink in CI.

### Tasks and project sources (milestone 1.5)

- **Every task says what to do.** A task has a title and a description in
  markdown: a one-line goal, numbered steps, and a "Done when" line. A title
  alone is never enough; the editor shows that template for new tasks, and the
  orchestrator (milestone 3) must fill it before a task can start.
- **Editing in Kompanion.** Add a task, change title, description and state,
  reorder, close, delete (asks first). Every endpoint is scoped to the
  signed-in user. Changes are kept in a `task_events` history.
- **Project sources.** Each project has `source = internal | windshift`.
  - *internal*: only in Kompanion.
  - *windshift*: a live link to a Windshift workspace, configured per user:
    the Windshift URL in the user's settings, the API token in the server's
    secret store (`.env`), never in the database. Edits in Kompanion are
    written to Windshift through its token API (`/rest/api/v2/items`);
    Windshift changes are pulled every few minutes. If both sides changed since
    the last sync, the newest write wins and the losing version is kept in the
    task's history.
  - Users without Windshift only see internal projects.
  - A windshift project can be turned into an internal one; it keeps a copy
    and stops syncing.
- **Tests:** per-user scoping (another user's task is a 404), stable
  reordering, the sync adapter against a fake Windshift (including the
  both-sides-changed case), and the description template check.

### Notifications (milestone 3, when tasks run)

- Per-user switches: task needs input, task failed, task done, daily summary
  (`notification_prefs` table). Mail for now; web push later.
- Sent from the task state machine on state changes, batched (at most one mail
  per task per 10 minutes, daily summary at a set hour).
- Tests: a simulated task run produces exactly the mails the user switched on,
  with no secrets in the body.

### Agents that don't need babysitting (owner request, 2026-10-01)

Kees: "if explicitly given permission it should be able to handle most by
itself". These land in **milestone 3** (they need running tasks and agents),
with the permission model started in milestone 2 (the runner is the first
thing that asks).

- **Standing permissions.** A user grants a scope once, explicitly: a
  machine, a folder or repo, a kind of action (read, write, run commands,
  push without force, install packages, change a service), optionally with a
  time limit. Agents then act inside that scope without asking again. Grants
  are listed on one page, can be revoked at any time, and every action taken
  under a grant is logged with the grant that allowed it. Outside the scope,
  or for anything no one can undo (deleting data, force-push, production
  changes, spending money), the agent still asks. It only escalates to the
  user for things that need their hands: typing a password, a web login, a
  physical action. Before handing the user a command, the agent tests it.
- **Agents talk to each other.** The orchestrator and its task threads can
  message, steer and hand work to each other directly (a message bus on the
  server, stored like everything else), so the user is never the relay
  between two agents. A task thread can ask the orchestrator for help or
  another machine; the orchestrator can redirect or stop a task.
- **Orchestrator notes.** Like the coordinator in Kees's Claude project: the
  orchestrator posts short notes to the user ("started", "blocked on X",
  "done, here is the result") and can inject notes into a running task
  thread, which show there as a distinct, visible "note from the
  orchestrator" bubble. The user sees who said what.
- **Calendar view.** A month/week view of planned and scheduled tasks
  (scheduled-for time, deadlines, night-scheduled work, consent windows),
  next to the task list. Dragging a task to another day reschedules it.
  Imported tasks show up once they have a date.

## Milestone 1 in detail (the first step)

- `server`: axum app with `/api/chat` (SSE stream), `/api/providers`,
  `/api/conversations`; adapters for OpenAI-compatible and Anthropic; SQLite via
  `sqlx`; OIDC login via Keycloak; config and keys from env.
- `web`: login, conversation list, chat view with streaming markdown, model
  picker, settings page.
- `deploy`: Dockerfile and a compose service for kireserver, behind your reverse
  proxy with Keycloak (e.g. kompanion.kreative-kompas.com).
- Done when you can open it on your phone, pick Qwen or DeepSeek, and chat.
