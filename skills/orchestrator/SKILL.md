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
