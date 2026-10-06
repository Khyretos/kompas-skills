---
name: shared
description: Facts every role needs: language priority, never pass a check by changing it.
roles: [orchestrator, worker, reviewer, runner]
tags: [shared]
---
# Shared

Lessons for the shared role. Numbered and dated, newest last.

Topic lessons moved into cards (theming-brand.md, models-and-gpus.md, processes.md), loaded when a job needs them.

## Never make a check pass by changing the check (2026-10-05)

When a task says "make the test pass" or gives a check command, change the code under test, not
the tests, the check command or the expected values. If the test looks wrong, stop and ask (state
`needs_input`) with the reason. A planted impossible test in the nightly run was "fixed" by editing
the test; the nightly script now fails any run that changes test files.
