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

- `<role>/SKILL.md` is the role's core: the rules every job in that role needs. New lessons on a
  topic that has a card go into that card, not the core (SK-01b split the web, rust and
  localization cores). A card in `worker/<area>/` is only given to that area's jobs.
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
- Task runs (SK-02) use the same rules in the server (`server/src/skills.rs`, a port of load.py).
  The planner names an area per step (one of the `worker/<x>` folders with a SKILL.md); files the
  step names override it when they match another area core's `paths` globs. Each step's cards are
  listed in the plan note and stored with the run; the worker gets exactly those cards for that
  step, and the reviewer gets `reviewer/SKILL` plus every card the steps used. The server reads the
  folder from `KOMPANION_SKILLS` (default `/app/skills`, which the image contains) and the budget
  from `[skills] budget_tokens`, a number or a table per model name with an optional `default`.
- After a review with findings, the reviewer drafts up to 3 lessons for the cards the steps used and
  proposes each in the project thread. Nothing becomes a rule until the user accepts it there (the
  text can be edited first) or dismisses it. Accepted lessons are appended to
  `<data>/skills/<card>.md` (next to the database, or `KOMPANION_SKILLS_DATA`), one line each with
  the date, and to `<data>/skills/lessons-learned.md`. The next run's worker and reviewer get them
  after that card's text. The skills folder in the image stays read-only.
- Every review finding: a lesson in the right card or core, and a row in
  `docs/lessons-learned.md` (in this repo).

## Layers (SK-03, 2026-10-06)

`tools/skills/load.py` merges three layers, later ones win:
1. general: `skills/general/` (the public `kompas-skills` library, no setup facts);
2. Kompanion: the rest of `skills/` (Kompanion's own coding lessons);
3. private: `$KOMPANION_SKILLS_LOCAL` (default `/skills-local`, read-only mount of the setup's own repo).

A card with the same path in a later layer replaces the earlier one. A card with `overrides: <name>`
in its header replaces that card's body; `extends: <name>` adds its body after it. Before anything
goes into `skills/general/`, `tools/skills/privacy_check.py skills/general` must pass (no IPs, home
paths, emails, or host names from the private layer's `deny-hosts.txt`).
