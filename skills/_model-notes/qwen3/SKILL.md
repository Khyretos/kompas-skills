# qwen3:14b (Ollama on soucouyant), notes for prompts

Measured 2026-10-01..03 (details: ~/Docker/docs/ai-capability/qwen3-14b-soucouyant-2026-10.md).

- Turn thinking off with `reasoning_effort: "none"`; Ollama ignores `chat_template_kwargs`.
- Good: UI code in an existing style when given an example file; restructuring existing text faithfully.
- Weak: systems/stateful code (rates, sysfs), following negative rules ("never use X") — give a positive example of the allowed pattern instead.
- Invents paths, hosts and settings when the prompt doesn't contain them; put the real config in the prompt.
- Sometimes copies an instruction from the prompt into its output; strip lines like "ensure no additional ...".

## Colour themes / palettes (2026-10-03, VS Code theme)

Full write-up: kreative-kompas-vscode-theme `docs/ai-capability/vscode-theme-drafting.md`.

- With `think: false` and "output only JSON" it returns valid JSON first try (~50 s).
- "X is the main accent" makes it use X for almost everything; it ignored the secondary accents.
  Give a role-to-colour table instead.
- Cannot judge contrast: put violet text on plum (2:1). Give it pre-checked text colours; run the contrast script on the result.
- Made selection and current line the same colour; ask for each state as a separately named colour.
- Invents/uses deprecated theme keys (`tab.unselectedOddBackground`, `scrollbar.background`, ...) and
  mis-maps terminal ANSI and diff colours. Give it the exact key list to fill.
