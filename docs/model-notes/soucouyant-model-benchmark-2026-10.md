# Which model for soucouyant's RX 9070 XT (2026-10-03)

Ollama 0.35.0 (Vulkan, llama.cpp's llama-server underneath) on soucouyant,
RX 9070 XT 16 GB. The desktop itself holds about 2.3 GB of VRAM, so roughly
13.7 GB is free for a model. All models are FOSS (Apache 2.0). Scripts and raw
answers: `bench-soucouyant-2026-10/` next to this file (`long.txt`, the long-context
input, is kreative-kompanion's `docs/*.md` plus `skills/**/*.md` concatenated).

## Short answer

- **Recommended: `gemma4:12b-it-qat`.** It reads images, did as well as or
  better than qwen3:14b on every test, writes the best Dutch of the set, runs
  a bit faster (65 vs 60 tok/s) and needs about 3 GB less VRAM. Even at 128k
  context the card is only at 12.4 GB, so context is no longer the limit.
- `qwen3.5:9b` is the fastest model with vision (84 tok/s). Its Dutch is the
  weakest: it translated "Our fork" as "Onze vork" (a kitchen fork) again, the
  same mistake it made on OVMS.
- `gpt-oss:20b` is by far the fastest (125 tok/s) and the only one that passed
  all three basic coding tests. But it has no vision, it always "thinks" (even
  on low, so it needs 4x the `max_tokens`), it put Dutch words into an
  English-only enum, and it fills the card to 15.6 GB.
- `qwen3.6:35b-a3b` (MoE) does not fit: Ollama puts 45% on the CPU, which gives
  26 tok/s and uses about 20 GB of system RAM. Its quality was at the top, but
  not by enough to give up gaming headroom and speed.
- **Why qwen3:14b was there:** the choice was never written down. It is the
  largest Qwen that fits fully at 4-bit with some room for context, and the
  Intel side already ran Qwen3.5-9B. It has no vision, and above 16k context it
  no longer fits (at 32k, 1.5 GB spills to the CPU).

## Speed and memory

Context 16k unless noted, temperature 0, one request at a time. "Card" is the
total VRAM in use, desktop included (`mem_info_vram_used`).

| Model | Vision | Generate tok/s | Prompt tok/s (12k prompt) | Card at 16k | Larger context |
|---|---|---|---|---|---|
| qwen3:14b (Q4_K_M) | no | 60 | 1,865 | 14.0 GB | 32k: spills 1.5 GB to CPU |
| qwen3.5:9b (Q4_K_M) | yes | 84 | 3,531 | 9.4 GB | 64k: 11.5 GB |
| **gemma4:12b-it-qat** | yes | 65 | 2,612 | 11.1 GB | 128k: 12.4 GB |
| gpt-oss:20b (MXFP4) | no | 125 | 4,874 | 15.6 GB | 32k: 15.8 GB |
| qwen3.6:35b-a3b (Q4_K_M) | yes | 26 | 555 | 16.2 GB + ~20 GB RAM | (45% on CPU) |

Load times from the page cache: 3.6 to 7.9 s; qwen3.6 17 s.

## Quality

Fixed tasks, scored automatically where possible (unit tests, tool-call
arguments, exact values) and by hand for the free-text Dutch.

| Test | qwen3:14b | qwen3.5:9b | gemma4:12b | gpt-oss:20b | qwen3.6:35b |
|---|---|---|---|---|---|
| Basic code with hidden tests (3: Python duration parser, Python rate meter, Rust PCI address) | 1 | 2 | 2 | **3** | **3** |
| Harder code (Python TTL/LRU cache) | pass | pass | pass | pass | pass |
| Rust fdinfo aggregator | fail | fail | fail | fail | fail |
| Tool calls (5: weather + conversion, file lookup, no tool needed, two tasks with enum/date, JSON schema) | 5 | 5 | 5 | 4 | 5 |
| Agent: find issue, read code, post exactly one Dutch comment | pass | pass | pass | pass | pass |
| Find one fact in a 12k-token document | pass | pass | pass | pass | pass |
| Dutch reading comprehension | pass | pass | pass | pass | pass |
| English summary with rules (2 sentences, ≤40 words, no "very") | fail (44 words) | pass | pass | fail (45 words) | pass |
| Vision (3: status panel to JSON, bar chart, Dutch road sign) | n/a | 3 | 3 | n/a | 2 (misread chart) |
| Vision: invoice image, sum quantity × price | n/a | pass | pass | n/a | pass |

Notes on what failed:

- **Rust fdinfo**: all five split `key:\tvalue` on the tab and kept the colon
  in the key (`"drm-pdev:"`), so nothing matched. gpt-oss would also have
  counted a client twice. Same weakness as the fdinfo reader rewrite earlier
  this week: give them the exact parsing line.
- qwen3:14b's duration parser read the unit as `" ns"` (with the space).
- gpt-oss sent `priority: "hoge"` and `"normaal"` where the schema only allows
  `high`/`normal`: it translates enum values when the prompt is Dutch.

Dutch, by hand:

| | UI strings with context | Idiom: "de kat uit de boom kijken", "er een hard hoofd in hebben" | Short release note |
|---|---|---|---|
| qwen3:14b | good ("Diensten actief", "Aanmelden") | both wrong ("check the situation", "I'm determined") | grammar errors ("De machines panel", "Het runner") |
| qwen3.5:9b | **"Onze vork"**, "Diensten gezond", "Erkentenissen" | half: "keep an eye on the cat"; pessimism right | fine |
| gemma4:12b | best: "Over ons", "Onze fork", "Services actief", "Stichting" | both right (but gave three options instead of one) | fine, but bullets and a header where prose was asked |
| gpt-oss:20b | good ("Dankbetuigingen", "Alle servers zijn online") | half: "wait and see" right, "I'm confident" is the opposite | changed the meaning ("het is mogelijk om") |
| qwen3.6:35b | good | both right | turned "Fixed: runner reconnects" into a bug that caused reconnects |

## Recommendation and switching

**Decision (Kees, 2026-10-03): gemma4:12b-it-qat is the model on soucouyant.**
The drafting pipeline defaults to it (kreative-kompanion 8b76c19); the test
models were removed; kk-localize's judge moved over the same evening, and
qwen3:14b was removed at 22:21 once nothing had requested it for 4 minutes.
gemma4:12b-it-qat is now the only model on soucouyant.

`gemma4:12b-it-qat` is the best fit for this card: vision, the best Dutch, all
tool tests passed, about 8.8 GB for the model with room for very long contexts,
and faster than qwen3:14b. It works over the OpenAI endpoint with tools
(checked: `reasoning_effort: "none"` gives a plain tool call, no thinking).

Kompanion's own config (`kompanion.toml`) does not name an Ollama model: all
three roles use OVMS "Coder", and the ollama-soucouyant provider only lists
what Ollama has. The callers that do name a model are kk-localize (judge,
`qwen3:14b`) and Kompanion's drafting pipeline (`tools/qwen/pipeline.py`,
`QWEN_MODEL`, default `qwen3:14b`). qwen3:14b is left installed and unchanged
so the running localization scores stay comparable; moving those two callers
to gemma4 is a separate step.

Use **one model name per host**. On 2026-10-03 kk-localize used `qwen3:14b`
while the drafting pipeline used `qwen3:14b-16k`. Ollama cannot hold both, so
it reloaded on almost every request (44-65 s each instead of under 1 s).

## Where Ollama's ~8 GB of system RAM goes

Measured on the qwen3:14b runner that had been serving kk-localize
(2026-10-03 20:59, cgroup `ollama.service`: 9.5 GB in use):

| Part | Size | What it is |
|---|---|---|
| llama-server prompt cache | **~8.0 GB** (anonymous) | llama-server's `--cache-ram`, default **8192 MiB**. It saves the KV state of finished prompts in RAM so a repeated prefix is not computed again. The journal shows it full: `cache state: 178 prompts, 8183.423 MiB (limits: 8192.000 MiB)`. With qwen3:14b each token of context is ~160 KB (40 layers × 8 KV heads × 128 × K+V × 2 bytes), so a ~300-token judge prompt is ~46 MB. |
| Model file mapped into the runner | 0.14-0.5 GB | Only the token embedding table stays mapped on the CPU side; all layers are on the GPU. |
| `ollama serve` itself | 33 MB | The API server and scheduler. |
| Model file in the page cache | up to 9.3 GB, counted as "buff/cache" | The kernel keeps the file cached after loading; that is why a reload takes 1-4 s. It is freed as soon as anything else needs the memory, so it is not really in use. |

A freshly loaded runner uses about 0.5 GB of RAM (qwen3:14b: 0.06 GB anonymous
+ 0.49 GB mapped). During this benchmark the anonymous part grew to 3-5 GB per
model within about 20 requests, which is the prompt cache filling up.

The KV cache for the context itself is on the GPU (part of the ~12 GB of VRAM).
The swap peak of 3.5 GB in the service's history is this prompt cache being
pushed out under memory pressure.

To cap it, add one line to `/etc/systemd/system/ollama.service.d/override.conf`
and restart Ollama (needs sudo):

```
Environment="LLAMA_ARG_CACHE_RAM=2048"
```

Applied on soucouyant 2026-10-03 (`~/ollama-cache-cap.sh`, value 2048): Ollama
passes the variable on, and the journal shows `limits: 2048.000 MiB`.
0 turns the cache off. For one-request-at-a-time batch jobs with different
prompts it gives little; for agent loops that resend a long system prompt it
saves prompt time, so 1-2 GB is a reasonable middle.

## Side findings

- The running Ollama is 0.35.0 while the package is 0.35.1: it was upgraded
  but not restarted.
- The system `rustc` on soucouyant is broken (`undefined symbol ... LLVM_23.1`,
  rust 1.99.0-1 against a newer LLVM); the Rust tests ran in the `rust:1` image.
- Ollama splits a too-large MoE by layers. llama.cpp can instead keep only the
  expert weights on the CPU (`--n-cpu-moe`), which is usually much faster for
  models like qwen3.6:35b-a3b, but Ollama does not expose it.

## Follow-up: CPU use (2026-10-03, 23:20)

With gemma4 fully on the GPU and kk-localize sending ~100 requests a minute,
llama-server still used ~290% CPU (7 threads at ~45% each; about half of the
8-core 9800X3D). Cause: llama.cpp's CPU thread pool spin-waits for work
between GPU steps (`--poll 50`, 8 threads by default). A test with my own
llama-server showed 313% CPU by default and 31% with `-t 1` or `--poll 0`, at
the same speed. OVMS doesn't do this, which is why it looks idle.

Fix, no sudo: the gemma4:12b-it-qat tag was rebuilt with `PARAMETER num_ctx
16384` and `PARAMETER num_thread 1` (Ollama passes `-t 1`). Result under the
same load: ~32% of one core, total CPU 55% → 7%, and speed unchanged (67 tok/s
generate, 2,727 tok/s prompt). The prompt cache was not the cause; it only
costs RAM.

## Follow-up: qwen3.5:9b-q8_0 (2026-10-04)

Kees approved `qwen3.5:9b-q8_0` as soucouyant's coding and PC-control model
next to gemma4. Its tag was rebuilt the same way (`num_ctx 16384`,
`num_thread 1`). Measured alone: 59 tok/s generate, 3,962 tok/s prompt, 12.8 GB
card total at 16k (desktop 2.3 GB). gemma4 (~8.8 GB) and this model (~10.5 GB)
do not fit together, so jobs that alternate between them make Ollama swap.
