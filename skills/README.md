# Skills (lessons per role)

Lessons belong to roles, not to models: whichever model fills a role reads that
role's skills. Every review finding becomes a lesson here, with a test case
where possible (`tests/` next to the lesson, or a unit test in the code it is
about). Layout:

- `orchestrator/`: planning tasks into steps with a "done when".
- `worker/rust/`, `worker/web/`: how to write code that passes review here.
- `worker/cpp-games/`: kk-engine games in C++ (cameras, controllers, assets, testing on soucouyant).
- `worker/localization/`: translating the website (kk-localize): what to protect, what needs context, what to hold.
- `worker/docs/`: READMEs, guides and task descriptions: only given facts, keep what you are told to keep.
- `runner/`: running tools on PCs within the access grants.
- `shared/`: facts every role needs (house rules, brand, FOSS only).
- `reviewer/`: what to check, in order.
- `work-habits.md`: how the reviewer works; the pipeline gives it to every role.
- `_model-notes/<model>/`: known failure patterns of one model (qwen3, gemma4, gpt-oss), so prompts can
  guard against them.

Each folder has a `SKILL.md`; lessons are numbered and dated, newest last.

## Cards: small topic files any model can load (2026-10-05)

These files grow with every review, so models get only what a job needs, and every model
(today's 9B, a 27B later) reads the same lessons.

- `<role>/SKILL.md` is the role's core: the rules every job in that role needs.
- `<role>/<topic>.md` is a card: one topic, with a header for the loader:
  ```
  ---
  name: shared/colour-themes
  description: One line: when this card helps.
  roles: [worker, reviewer]
  tags: [theme, css, contrast]
  paths: ["**/*.css"]        # files that make this card relevant (optional)
  models: [qwen3]            # only for _model-notes; leave out for general cards
  ---
  ```
- `_model-notes/<family>/SKILL.md` holds only quirks of that model family (settings, speed, typical
  slips). A general rule never lives only there.
- `work-habits.md` goes to every job.
- Loading (SK-01): `tools/skills/load.py` gives every job `work-habits.md`, `shared/SKILL.md`, the role
  core, the cards the job names in `"skills"` and the model's notes (`MODEL_NOTES`, default `qwen3`),
  then the cards whose `paths` match the job's files or whose `tags` appear in the task, best match
  first, within `[skills] budget_tokens` in kompanion.toml (default 1500). Each drafting-log line
  lists the files a job got. `tools/skills/index.py` writes `skills/index.json`; with `--check`
  (in CI) it fails on a file without a header.
- Every review finding: a lesson in the right card or core, and a row in
  `docs/lessons-learned.md` (in this repo).
