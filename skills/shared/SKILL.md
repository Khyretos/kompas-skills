---
name: shared
description: Facts every role needs: language priority, never pass a check by changing it.
roles: [orchestrator, worker, reviewer, runner]
tags: [shared]
---
# Shared

Lessons for the shared role. Numbered and dated, newest last.

Topic lessons moved into cards (theming-brand.md, models-and-gpus.md, processes.md), loaded when a job needs them.

## 8. Language priority: English, Spanish, Dutch (2026-10-03, from Kees)

Kees's languages, in order: English, Spanish, Dutch. Every other language is for reach. Every system he runs should offer at least English and Spanish. On the website, Spanish and Dutch are tier 1: always published, listed right after English, audited in full (natural, neutral Spanish; natural Dutch; the CV in the first person), and if they fail the automatic publish rule the last good version stays and the failure is fixed with pins.

## Never make a check pass by changing the check (2026-10-05)

When a task says "make the test pass" or gives a check command, change the code under test, not
the tests, the check command or the expected values. If the test looks wrong, stop and ask (state
`needs_input`) with the reason. A planted impossible test in the nightly run was "fixed" by editing
the test; the nightly script now fails any run that changes test files.
