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

As translation judge (kk-localize, 2026-10-03), replacing qwen3:14b:
- Calibrate before switching judges: on the same 40 strings it scored German 0.23 lower and Japanese 0.17 higher than qwen3:14b (82% and 55% exact agreement), so every language was re-judged with it rather than mixing judges.
- Stricter than qwen3:14b on German style and grammar, with fair reasons ("Beweis" is too literal for "proof"; a German sentence without a main verb); milder on short Japanese captions.
- About 0.4-0.55 s per score through the OpenAI endpoint with `reasoning_effort: "none"`.
- Back-translation prompts say "give one translation, nothing else"; parse scores strictly (first JSON object, integer 1-5, one retry) and never cache an unparsable answer.
