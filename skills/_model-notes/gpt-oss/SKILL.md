---
name: _model-notes/gpt-oss
description: Quirks of the gpt-oss models only (settings, speed, memory, typical slips).
models: [gpt-oss]
---
# gpt-oss, notes for prompts

## gpt-oss:20b (Ollama on soucouyant, RX 9070 XT)

Benchmarked 2026-10-03 (details: docs/model-notes/soucouyant-model-benchmark-2026-10.md).

- Fastest local option: 125 tok/s, ~4,900 tok/s prompt. No vision. Fills the 16 GB card to 15.6 GB at 16k.
- Thinking cannot be turned off (`think: "low"` is the minimum). Give it about 4x the usual `max_tokens`,
  or the answer comes back empty or cut off mid-sentence because the budget went to reasoning.
- With a Dutch prompt it translated enum values in tool calls (`priority: "hoge"` for `high`). Validate
  enum arguments and say "use the enum values exactly as listed".
- Passed all three small coding tests, but got the Dutch idiom "er een hard hoofd in hebben" backwards
  ("I'm confident") and changed the meaning of a release note line.
