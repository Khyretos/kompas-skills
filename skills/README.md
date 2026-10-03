# Skills (lessons per role)

Lessons belong to roles, not to models: whichever model fills a role reads that
role's skills. Every review finding becomes a lesson here, with a test case
where possible (`tests/` next to the lesson, or a unit test in the code it is
about). Layout:

- `worker/rust/`, `worker/web/`: how to write code that passes review here.
- `reviewer/`: what to check, in order.
- `_model-notes/<model>/`: known failure patterns of one model, so prompts can
  guard against them.

Each folder has a `SKILL.md`; lessons are numbered and dated, newest last.
