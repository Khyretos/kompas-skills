---
extends: runner/SKILL
---
4. (2026-10-04) A job waits for the runner's next report, so job latency is the report interval.
   The server answers 1 s while an agent works on that computer (a step in flight, a W2 task
   running there, or a step in the last 2 minutes, for the model's thinking time between steps)
   and the normal interval otherwise. Measured in a W2 run: median 0.11 s, max 0.84 s per tool
   call (it was up to 60 s). No runner change was needed.
