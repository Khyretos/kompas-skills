# qwen3:14b (Ollama on soucouyant), notes for prompts

Measured 2026-10-01..03 (details: ~/Docker/docs/ai-capability/qwen3-14b-soucouyant-2026-10.md).

- Turn thinking off with `reasoning_effort: "none"`; Ollama ignores `chat_template_kwargs`.
- Good: UI code in an existing style when given an example file; restructuring existing text faithfully.
- Weak: systems/stateful code (rates, sysfs), following negative rules ("never use X") — give a positive example of the allowed pattern instead.
- Invents paths, hosts and settings when the prompt doesn't contain them; put the real config in the prompt.
- Sometimes copies an instruction from the prompt into its output; strip lines like "ensure no additional ...".

## Qwen3.5-9B int4 ("Coder" on OVMS, kireserver)

- (2026-10-03) Long bash/ImageMagick scripts: wordy comments eat the 2000-token budget and the script gets cut off midway; ask for "script only, no comments" or raise `max_tokens`. Details: ~/Docker/docs/ai-capability/peertube-images-2026-10.md.
