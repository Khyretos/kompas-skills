# Orchestrator

Lessons for the orchestrator role. Numbered and dated, newest last.

- Build job prompts with a quoted heredoc (`<<'EOF'`) or a file. In an unquoted heredoc the
  backticks around code names run as shell commands and the model gets a garbled prompt.
- Send long drafts to OVMS on its container address, not through the proxy: the proxy cuts
  answers off at 60 s (504).
- Keep each draft under about 250 lines of output. At 600+ lines the 9B model drops the spec,
  invents code, and copies context files into the output. Split big modules into small files,
  and put signatures in the prompt rather than whole context files.
- OVMS Coder cuts prompts at about 8k tokens (prompt_tokens 8194 is the sign). For files over
  about 20k characters, use patch mode with `"focus": [regex, ...]` so only the relevant lines
  are sent. Fully literal edits (exact code given in the prompt) are applied by the orchestrator.
- Never `pkill -f <pattern>` with a pattern that appears in your own command line: it kills
  your own shell. Use `pgrep -f '[d]ist/name'` (the bracket trick) and kill the pids you see.
- PC agent prompts: tell the model to finish every part of a request (one tool call after
  another), and to claim success only when a result says "done" with "exit: 0", quoting the key
  output line. Give tool results outcome-first ("The step ran. State: done. Output: ..."), and
  the end of long output, where the exit code is.
- Deploy only with `tools/deploy.sh`. It fetches and refuses to build unless origin/main is an
  ancestor of HEAD: other threads merge too (2026-10-04, a deploy without their 0100 migration
  crash-looped production). Before adding a migration, check the numbers on origin/main
  (`git ls-tree origin/main server/migrations/`); the asset thread uses 0100 and up.
- (2026-10-04, W2 end-to-end) Plan steps are changes, each with a "done when" the worker can see:
  `[{"step": "add char_count to textutil.py", "done_when": "textutil.py defines char_count"}]`.
  Never a step that only opens, reads or finds something, and never "run the tests": the check
  runs by itself after the steps. A small task is one or two steps. The worker gets only its
  step and stops once done_when holds. With 7 fine-grained steps the worker did everything in
  step 1 and re-checked it six times (22 tool calls, 21 min); with this, 7 tool calls, 38 s.
- Test W2 changes on a throwaway instance, not on production: a second container from the new
  image on another port with its own volume, a runner in an Alpine container sharing that
  container's network (`--network container:…`, so `http://127.0.0.1` is allowed) with only a
  test repo mounted, and a project from `kompanion-server import`. Fresh account each run, so
  defaults get tested too.
- (2026-10-04) Qwen3.5 9B still plans "open X and locate Y" steps when told not to, so the
  server drops look-only steps (`parse::look_only`: open, read, find, … and "run/verify the
  tests") unless nothing else is left. The worker gets the task description with every step and
  every fix: without it, it wrote `initials(first, last)` because it never saw the test it had
  to pass. A worker that runs out of tool calls is not a failure: the check and review decide
  (it had made the tests pass, then re-checked until the cap). Demo task, 3 runs in a row: one
  step, 5 approvals, done in round 1, 36–39 s.
- (2026-10-05) `tools/deploy.sh` builds the working tree, untracked files included: a drafted
  migration left in the main checkout would have shipped unreviewed. Draft each item in its own
  worktree (`git worktree add ../kreative-kompanion-<item> -b <item> origin/main`, symlink
  web/node_modules) and deploy only from the clean main checkout. kireserver has no Playwright
  browser; browser tests run in CI on soucouyant, which builds main and pull requests only: open
  the PR without a token with `git push origin HEAD:refs/for/main -o topic=<item>` (AGit).
