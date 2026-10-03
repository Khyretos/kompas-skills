# Gemma 4 family, notes for prompts

## gemma4:12b-it-qat (Ollama on soucouyant, RX 9070 XT)

Benchmarked 2026-10-03 against qwen3:14b, qwen3.5:9b, gpt-oss:20b and qwen3.6:35b-a3b
(details: ~/Docker/docs/ai-capability/soucouyant-model-benchmark-2026-10.md).

- Apache 2.0, vision, tools. 65 tok/s, ~2,600 tok/s prompt; 8.8 GB VRAM at 16k and still only
  12.4 GB card total at 128k context (sliding-window attention keeps the KV cache small).
- Thinking off: `think: false` (native API) or `reasoning_effort: "none"` (OpenAI endpoint); tool calls
  come back clean either way.
- Best Dutch of the five: UI strings with context right ("Onze fork", "Stichting", "Over ons"), idioms right.
- Asked to "translate", it offered three versions with headings. Say "give one translation, nothing else".
- Asked for a short prose release note, it wrote a header and bullets. Say "prose, no list, no heading".
- Read a status panel, a bar chart and an invoice table correctly, including summing quantity × price.
- Like the others, it split fdinfo `key:\tvalue` lines on the tab and kept the colon in the key; it also
  first assumed `&[&str]` were file paths. Say "each element is file content" and give the parsing line.
