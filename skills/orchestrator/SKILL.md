# Orchestrator

Lessons for the orchestrator role. Numbered and dated, newest last.

- Build job prompts with a quoted heredoc (`<<'EOF'`) or a file. In an unquoted heredoc the
  backticks around code names run as shell commands and the model gets a garbled prompt.
- Send long drafts to OVMS on its container address, not through the proxy: the proxy cuts
  answers off at 60 s (504).
- Keep each draft under about 250 lines of output. At 600+ lines the 9B model drops the spec,
  invents code, and copies context files into the output. Split big modules into small files,
  and put signatures in the prompt rather than whole context files.
