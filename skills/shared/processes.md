---
name: shared/processes
description: Background processes, long jobs, containers and CI: waiting, detaching, inspecting without leaking secrets.
roles: [worker, reviewer, orchestrator]
tags: [process, background, container, docker, ci, secrets, detached]
---
# Shared: Processes

## 3. Waiting for a background process (2026-10-03)

`pgrep -f <name>` also matches the shell that runs the wait loop (its command line contains the name), so `until ! pgrep -f x` never ends. Wait on the PID instead (`wait $pid`, or `while kill -0 $pid`), or match with a pattern that can't match itself (`pgrep -f '[p]ipeline.py'`).

## 4. Inspecting containers without leaking secrets (2026-10-03)

Never print environment values when inspecting a container: a grep for a model name matched "14B" inside an SMTP password and printed it. List variable names only, e.g. `docker inspect <c> | jq -r '.[0].Config.Env[] | split("=")[0]'`; when a value is really needed, print it only for names that don't match `PASS|PWD|KEY|SECRET|TOKEN|CREDENTIAL|AUTH` (case-insensitive). The same goes for `.env` files: `grep -oE '^[A-Z_]+='`.

## 5. CI jobs never use the host toolchain (2026-10-03)

Every job runs in a container image (rust:1, rust:1-alpine, node:22); a broken system update on the runner host must not break builds.

## 9. Long jobs run detached; whoever pauses a job un-pauses it (2026-10-04)

A batch run started from an agent session dies when the session parks (a re-judge stopped after one language at 21:46 and nothing ran for two hours). Start long jobs fully detached, with their own log: `setsid -f script.sh … < /dev/null`, or `docker compose run -d`. Pausing a scheduled job (a `.paused` file) needs a reason, an owner and an expiry line (`expires: 2026-10-05 00:00`) so the job resumes by itself; the one who pauses removes the file as soon as the reason is gone. Don't `pkill -f` a pattern that is also in your own command line: it kills your shell.
