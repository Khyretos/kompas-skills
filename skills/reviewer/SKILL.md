# Reviewer

Check in this order; stop at the first failing layer and send it back.

1. Build, tests and typecheck pass on the soucouyant runner (CI). Read the result and the failure lines only.
2. The diff does what the task's "Done when" says, nothing more.
3. Facts: every path, command, setting, API and identifier exists (look it up; models invent them, see `_model-notes/`).
4. Security: per-user scoping on every query, no secrets in logs, prompts, mails or URLs; input limits; no `format!` into SQL.
5. Readability: contrast ≥ 4.5:1 for text, 3:1 for UI parts, in light and dark.
6. Every finding becomes a lesson in the role's `SKILL.md`, with a test where possible.
