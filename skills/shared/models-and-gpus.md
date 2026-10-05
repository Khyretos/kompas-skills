---
name: shared/models-and-gpus
description: Running local models on shared GPUs: Ollama hosts, model tags, night batches, CPU use, OVMS VRAM.
roles: [worker, reviewer, orchestrator]
tags: [ollama, ovms, gpu, vram, model, batch, llama]
---
# Shared: Models and gpus

## 5. Ollama hosts: one model name, and what the RAM is (2026-10-03)

Source: soucouyant benchmark, docs/model-notes/soucouyant-model-benchmark-2026-10.md.

- Ollama 0.35 runs models through llama.cpp's llama-server. It keeps a prompt cache in system RAM
  (`--cache-ram`, default 8192 MiB) that fills after a few hundred different prompts: that, not the model,
  is the ~8 GB of RAM next to ~12 GB of VRAM. `journalctl -u ollama | grep "cache state"` shows it.
  Cap it with `Environment="LLAMA_ARG_CACHE_RAM=2048"` in the service override.
- Every distinct model name (including `:14b-16k` style context variants) is a separate load. Two callers
  on one 16 GB card with different names make Ollama reload on almost every request. Agree one name per host.
- Before benchmarking or swapping models on a shared host, check `journalctl -u ollama --since -3m | grep GIN`
  for other callers and ask them to pause; a swap stalls their jobs and spoils the timings.
- A model that doesn't fit is split by layers onto the CPU (qwen3.6:35b-a3b: 45% CPU, 26 tok/s, ~20 GB RAM).
  Check `ollama ps` says 100% GPU before trusting a speed number.

## 6. One model tag at a time on a shared GPU (2026-10-03)

Every request naming a different tag makes Ollama swap models (~45 s each). Drafting scripts, Kompanion roles and other tools on the same Ollama must use the same tag; after a switch, update the defaults first, then start jobs. Health checks only list models (`/v1/models`, `/api/tags`, `/api/ps`), never generate.

## 7. Batch jobs on a shared GPU run at night (2026-10-03, from Kees)

A batch job that calls a model on a machine people also use by day (kk-localize's judge on soucouyant's Ollama) only calls it in a night window: kk-localize starts at 00:30, finishes the language it is on and stops calling the judge at 07:00, and leaves the rest for the next night. Daytime and manual runs skip those calls (`FORCE_JUDGE=1` overrides) and leave the work pending instead of failed. Otherwise Ollama swaps models against the daytime drafting model (seconds become tens of seconds per request). Check other scheduled jobs before picking the time (kk-engine CI runs at 03:00).

## 6. Ollama/llama-server CPU use with the model fully on the GPU (2026-10-03)

- llama-server's CPU thread pool spin-waits (`--poll 50` by default) between GPU steps, so a model that is
  100% on the GPU still burned ~3 cores while generating (qwen3:0.6b test: 313% CPU by default, 31% with
  `-t 1` or `--poll 0`, same tok/s). Idle it uses nothing; a steady job like kk-localize keeps it spinning.
- Fix without sudo: `PARAMETER num_thread 1` in the model's Modelfile (Ollama passes it as `-t 1`). Rebuild
  the same tag (`FROM <tag>` + parameters, `ollama create <tag>`) so callers keep one name. On soucouyant
  gemma4:12b-it-qat went from ~290% to ~32% of one core with no speed loss (67 tok/s, 2,700 tok/s prompt).
- Only for models that are fully on the GPU: a partly offloaded model needs its CPU threads.

## (2026-10-04) OVMS is shared: when the A770 runs out of VRAM, every model goes down

The GPU OVMS on kireserver (Coder, Autocomplete, Whisper) segfaulted twice. The cause was VRAM, not a
request: the kernel logged `xe ... VM worker error: -12` one second before each segfault, because
Coder's dynamic KV cache with 8 parallel sequences outgrew the 16 GB card. Coder now runs with
max_num_seqs 2 (parallel calls queue). Lessons:

- When OVMS dies, look in the kernel log for xe/i915 memory errors before blaming the last request.
- VRAM planning must count KV-cache growth per parallel sequence, not only the weights.
- Try unknown requests on a throwaway OVMS first: same image, the model folder mounted read-only,
  `graph.pbtxt` overlaid with `target_device: "CPU"`, its own name on the network. Remove it after.
  (Whisper's `language` field was tested that way and is fine: "en", "es"; "<|es|>" is refused.)
- Build WAVs for Whisper yourself (44-byte PCM header with real sizes). ffmpeg writing WAV to a pipe
  leaves 0xFFFFFFFF sizes and a LIST chunk, and Whisper answers 400.
- Check what you forward (length cap, minimum, whole samples), one request in flight per user.
- CPU Whisper large-v3 int8 needs about 11 s for a 3 s clip (4 cores): too slow for chat voice.
