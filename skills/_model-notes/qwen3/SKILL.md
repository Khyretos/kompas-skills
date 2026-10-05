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

- Thinking off for drafting. OVMS ("Coder", Qwen3.5-9B int8 on the A770): every call sends
  `"chat_template_kwargs": {"enable_thinking": false}`, health pings included, or it spends
  `max_tokens` thinking and answers empty. Ollama: `reasoning_effort: "none"` (OpenAI endpoint)
  or `think: false` (native); Ollama ignores `chat_template_kwargs`.
- OVMS: call the container address, not the proxy (the proxy cuts answers at 60 s). Prompts are
  cut at about 4,096 tokens (8k for some configs: `prompt_tokens` 4096/8194 is the sign).
- Ollama on soucouyant: one tag per host (`qwen3:14b` and `qwen3:14b-16k` alternating reload the
  model every request, ~44 s each). `qwen3.5:9b-q8_0` is built with `num_ctx 16384`, `num_thread 1`.

## Speed and memory

- Qwen3.5-9B: ~30 tok/s on the A770 (one request; a running job slows Open WebUI chat from ~1.7 s
  to ~3.2 s per answer, so batches run with concurrency 1), 59-84 tok/s on the RX 9070 XT,
  12.8 GB card total at 16k.
- qwen3:14b: 60 tok/s, 14.0 GB at 16k; at 32k it no longer fits. No vision.
- Qwen3.5-9B has vision (image_url data URLs through OVMS): fine as a check, not the only gate.

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
