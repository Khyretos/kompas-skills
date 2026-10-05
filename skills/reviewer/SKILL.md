# Reviewer

Check in this order; stop at the first failing layer and send it back.

1. Build, tests and typecheck pass on the soucouyant runner (CI). Read the result and the failure lines only.
2. The diff does what the task's "Done when" says, nothing more.
3. Facts: every path, command, setting, API and identifier exists (look it up; models invent them, see `_model-notes/`).
4. Security: per-user scoping on every query, no secrets in logs, prompts, mails or URLs; input limits; no `format!` into SQL.
5. Readability: contrast ≥ 4.5:1 for text, 3:1 for UI parts, in light and dark.
6. Every finding becomes a lesson in the role's `SKILL.md`, with a test where possible.

## Lessons

7. (2026-10-03) Never let a model grade its own output. In kk-localize, Qwen3.5-9B repaired "Our fork" to "Onze vork" (a kitchen fork) on a wrong critique and then scored its own repair 5. Score with a different model (qwen3:14b judged the 9B's translations), keep a repair only when that judge scores it higher, and still read a small random sample yourself: the judge misses wrong senses of one-word labels.
7. (2026-10-03) Check that authentication uses the request's own identifiers: gemma4 wrote a results handler that authenticated against a hard-coded "dummy" machine and used the machine id from the path as the job id. Read every handler's first five lines for this.
8. (2026-10-03) Tuple `Path((a, b))` extractors bind in the order of the route's placeholders; check them against the route string (gemma4 swapped machine and job ids).
9. (2026-10-05) Read every changed condition in a patch, letter by letter: a Coder patch to mail.rs
   dropped one `!` (`smtp_from.is_empty()`), so every task mail failed with "mail is not set up"
   while mail was set up, and all tests passed. Diff the old and new line of each `if`/`ensure!`.
