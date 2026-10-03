# Orchestrator

Lessons for the orchestrator role. Numbered and dated, newest last.

- Build job prompts with a quoted heredoc (`<<'EOF'`) or a file. In an unquoted heredoc the
  backticks around code names run as shell commands and the model gets a garbled prompt.
- Send long drafts to OVMS on its container address, not through the proxy: the proxy cuts
  answers off at 60 s (504).
