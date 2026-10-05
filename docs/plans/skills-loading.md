**Goal:** Skill files grow with every review, so each job gets only the skills it needs within a size budget, and every model (today Qwen3.5-9B, later for example a 27B) reads the same files and benefits from the same lessons.

**Machine / role:** kireserver; server code (Rust) plus `tools/qwen/pipeline.py`.

**Depends on:** the card format in `skills/README.md` (2026-10-05).

**Steps**
1. Give every `<role>/SKILL.md` the same front matter the cards have (name, description, roles, tags, paths).
2. Create a small Python module `tools/skills/load.py` with one function `select(role, task_text, paths, model, budget_tokens)`: always include `skills/work-habits.md` and the role core; then cards whose `paths` globs match the files the job touches; then cards whose `tags` or `description` share words with the task text, best match first; then `_model-notes/<family>/SKILL.md` for the model's family only; stop before the budget is used up (count about 4 characters per token). It returns the chosen file list and the text with front matter removed.
3. Add budgets per model in `kompanion.toml` under `[skills]` (for example `budget_tokens = { "Coder" = 2500 }`, default 4000), so a bigger model gets more.
4. Update `pipeline.py` to call `select()` instead of loading whole files; a job's own `"skills"` list is always included.
5. Create `tools/skills/index.py` to write `skills/index.json` (name, description, roles, tags, paths, size per file); CI fails when a card has no front matter or a general rule sits only in `_model-notes`.
6. Update Kompanion server: implement the same selection in Rust for task runs (orchestrator, worker, reviewer), and update the Capabilities page to list cards too (today it only counts `SKILL.md` files, in `server/src/capabilities.rs`).
7. Ensure every model call records which skill files it got (in the call log), so a review can see whether a missing lesson was the cause.

**Done when:** a web CSS job gets `shared/colour-themes` and not `worker/rust`; a 9B job stays under its budget; the call log shows the chosen files.

**How to test:** unit tests for `select()` with fixed cards; run one real pipeline job and read its call log.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude. FOSS only._
