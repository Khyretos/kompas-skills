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
  browser; browser tests run in CI (kireserver's runner), which builds main and pull requests only: open
  the PR without a token with `git push origin HEAD:refs/for/main -o topic=<item>` (AGit).
- (2026-10-05, Kees) All Kompanion work runs on kireserver; soucouyant is only for image and
  audio generation. Drafts, roles, PR-Agent and CI use Coder on OVMS (A770) at all hours, never
  soucouyant's Ollama (it supersedes "soucouyant outside 03:00–08:00"). Coder serves 2 sequences
  at once: drafting uses one lane, PR-Agent one worker. Every PR push makes PR-Agent call the
  model up to 8 times, so batch pushes instead of pushing each small fix.
- (2026-10-05) The kireserver runner pulls a CI image once; later runs start in under a second
  (MegaLinter's 10.9 GB image: 1 min 8 s the first time, 0.4 s after). Images live in
  Services/forgejo-runner/dind-data, force_pull is off and nothing prunes them: keep it so.
  Before asking Kees to merge, every check of the PR must be green.
- (2026-10-05) Before telling Kees a PR is ready, check it is still mergeable against the
  current main (other threads and his merges move main): when several PRs touch the same files
  (skills/, docs/qwen-log.jsonl), merge main into the branch first, re-run the checks, then
  announce. Append-only files conflict often: keep both sides, in order.
- (2026-10-05, M6-03) A CLI path that needs no config or database must run before main loads
  them (`kompanion-server gpu-role` ran a migration on a host database otherwise). Live tests of
  privileged actions go through the same helper the server uses, with the socket path as an
  environment override, not through a second code path.
- (2026-10-05) Drafting logs go to `docs/qwen-log/<branch>.jsonl` (one file per branch): the
  shared docs/qwen-log.jsonl conflicted on almost every merge. Never `git add -A` a folder in a
  worktree: it committed the worktree's web/node_modules symlink into main (#29). Run tools
  from the main checkout's node_modules instead of symlinking.
- (2026-10-05) `tools/qwen/pipeline.py` takes the model from the worker role in kompanion.toml
  (KOMPANION_CONFIG, else the repo root, else the main checkout for worktrees). When you change
  the pipeline itself, run its jobs with `KOMPANION_CONFIG=<main checkout>/kompanion.toml` so a
  half-finished change cannot stop the job that finishes it.
- (2026-10-05) Mechanical code changes (wrap a call, rename, renumber, move a line) are cheaper and
  safer as a small script than as a model draft: `tools/refactor/supervise_sites.py` wraps a
  `tokio::spawn` body by matching braces. Give models the logic; give scripts the shapes.
- (2026-10-05) Name the task in every PR title: `[TEN-03] ...` (the end of the task id or the start of
  its title). The forge webhook then moves the task to in review, done or queued by itself
  (`docs/forge-webhook.md`).
- (2026-10-05) After a branch's work, add its cost line: `tools/qwen/summary.py <branch> --task <id>`
  (Coder tokens from the drafting log, Claude tokens from the session transcript, counts only).
  It shows when a task type is cheaper to hand off and when it is not.
