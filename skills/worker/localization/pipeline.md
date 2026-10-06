---
extends: worker/localization/pipeline
---
34. (2026-10-04) Release one language at a time behind an "audited" list: export only audited languages to the release branch, and when releasing language X, export only "already live + X". Otherwise the release carries along another language that passes but whose audit pins aren't applied yet.
