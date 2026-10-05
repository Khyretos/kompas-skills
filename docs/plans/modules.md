**Goal:** a bug or a heavy job in one area (for example the GPU scheduler or the asset scan) cannot take down chat and tasks; each area can be switched off.

**Machine / role:** kireserver; server (Rust: `server/src/main.rs`, `server/src/api.rs`, the area folders), web app (menu items).

**Depends on:** nothing.

**Steps**
1. Write `docs/modules.md`: one row per area (chat, tasks and runs, machines and runner, assets, games, GPU scheduler and roles, voice, studio, notifications, search) with its routes, tables, background jobs and which other areas it may call.
2. In code, each area gets one `mod.rs` with a `routes()` function and a `spawn()` for its background jobs; `main.rs` only wires them. Calls between areas go through a small public function, not into another area's internals.
3. Background jobs run under a supervisor: a panic or error is logged, the job restarts with backoff, and chat keeps answering.
4. Feature switches in `kompanion.toml` `[features]` (default on): an area that is off has no routes, no jobs, and its menu item is hidden; the Capabilities page lists which areas are on.
5. A test per area: with every other area switched off, its own tests still pass.

**Done when:** with `[features] assets = false, gpus = false` the server starts, chat and tasks work, and the menu shows neither; a forced panic in the asset scan does not stop chat.

**How to test:** the per-area tests; the forced-panic test.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC).

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
