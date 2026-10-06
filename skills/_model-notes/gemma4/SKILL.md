---
extends: _model-notes/gemma4/SKILL
---
## gemma4:12b-it-qat (Ollama on soucouyant, RX 9070 XT)
- (2026-10-03, Kompanion F5) No `query!` macros when given a positive example (good). But: kept binding `serde_json::Value` as an SQLite column through two fix rounds; wrote a security bug (fixed "dummy" machine id in a handler); used made-up fields (`grants.grants`). Needs the exact types of every field in the prompt.
