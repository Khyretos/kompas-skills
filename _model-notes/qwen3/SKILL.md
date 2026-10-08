---
name: _model-notes/qwen3
description: Quirks of the Qwen3 and Qwen3.5 models only (settings, speed, memory, typical slips). General rules are in the role skills.
models: [qwen3, qwen3.5]
---
# Qwen3 family: quirks only

General lessons learned from these models now live where every model reads them:
`work-habits.md`, `orchestrator/prompting-workers.md`, `shared/colour-themes.md`, `worker/*`.
Full history with evidence: `docs/model-notes/qwen3-history-2026-10.md`.

## Settings

## Speed and memory

- qwen3:14b: 60 tok/s, 14.0 GB at 16k; at 32k it no longer fits. No vision.

## Typical slips (check these in review)

- Grades its own work too kindly and follows a wrong reviewer comment.
- Dutch: literal sense ("Our fork" → "Onze vork", "kat uit de boom kijken"); Irish: makes good drafts
  worse; Japanese: leaves one-word labels in English. Use a judge or gemma4 for Dutch.
- Writes TypeScript when asked for plain JavaScript; copies numbered prompt steps as numbered comments.
- Answers Markdown without a wrapping fence (`pipeline.py` keeps unwrapped `.md` answers whole).
- Drops a must-keep sentence asked for in passing; keeps a 6-part structure and 15 table rows exactly.
- Plans "open/locate" and "run the tests" steps when told not to; re-checks finished work until the
  tool-call cap.
- Stateful code (object identity, mutexes, event listeners, process pipes) is wrong more often than
  not, and the same bug comes back after a review names it once. Pure logic, markup and CSS are good.
- Places a "first line" next to related lines instead of first; say exactly which existing line it
  goes before.
- Asked to remove lines, it may comment them out instead; say "delete them entirely".
- Adds filters or options the spec did not ask for (a `--task` filter that matched nothing); check
  every condition in a draft against the spec.
- Big files: several edits in one patch job run out of answer room or get half-applied; send one
  edit per job. "Replace X with Y" can come back as "delete X"; check the line is there.
- A patch job told to fix a function it wrote earlier may add a corrected copy of the block next
  to the old one instead of replacing it; check for duplicated blocks. Two such fix rounds on new
  logic: Claude writes it (pipeline check_loop, 2026-10-06).
- With excerpts (`focus`), a patch may copy the `// ...` skip marker into the code and close the
  function after it, splitting it in two. Check the edited function is whole (RUN-01, 2026-10-06).
- Build-check fix rounds whose error sits in another file: the model "fixes" unrelated code in its
  own file until the build is worse. Give the check to the job whose file holds the error, or let
  Claude fix it (RUN-01, 2026-10-06).
