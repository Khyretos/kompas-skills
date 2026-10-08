---
name: shared/models-and-gpus
description: Running local models on shared GPUs: Ollama hosts, model tags, night batches, CPU use, VRAM, voice.
roles: [worker, reviewer, orchestrator]
tags: [ollama, gpu, vram, model, batch, llama]
---
# Shared: Models and gpus

## 5. Ollama hosts: one model name, and what the RAM is (2026-10-03)

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

## 6. Ollama/llama-server CPU use with the model fully on the GPU (2026-10-03)

- llama-server's CPU thread pool spin-waits (`--poll 50` by default) between GPU steps, so a model that is
  100% on the GPU still burned ~3 cores while generating (qwen3:0.6b test: 313% CPU by default, 31% with
  `-t 1` or `--poll 0`, same tok/s). Idle it uses nothing; a steady job like kk-localize keeps it spinning.
- Only for models that are fully on the GPU: a partly offloaded model needs its CPU threads.

## (2026-10-04) VRAM and voice

- VRAM planning must count KV-cache growth per parallel sequence, not only the weights.
- Build WAVs for Whisper yourself (44-byte PCM header with real sizes). ffmpeg writing WAV to a pipe
  leaves 0xFFFFFFFF sizes and a LIST chunk, and Whisper answers 400.
- Check what you forward (length cap, minimum, whole samples), one request in flight per user.
- CPU Whisper large-v3 int8 needs about 11 s for a 3 s clip (4 cores): too slow for chat voice.
