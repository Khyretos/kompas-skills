---
name: runner
description: Running tools on computers within the access grants.
roles: [runner]
tags: [runner, grant, shell]
---
# Runner

Lessons for the runner role. Numbered and dated, newest last.

1. (2026-10-03) Privilege goes into tiny sandboxed helpers, never into the runner or the server. Example: kompanion-gpu-helper runs as its own system user with only CAP_SYS_PTRACE (the kernel also requires the reader to hold every capability of the target, so an empty-cap root helper reads nothing from container processes), reads only drm-* lines, serves aggregates on a group-only unix socket, no network, no input. The unprivileged side treats the helper's answer as untrusted (size limit, clamped values) and works without it.

2. Report what actually happened: a path that doesn't exist is "no such file or folder: …",
   never "not granted". Otherwise the model treats a typo as a refusal and works around it.
3. system_info carries what the model would otherwise guess: home, user, the Hyprland version and
   which config file exists (hyprland.lua or hyprland.conf).
4. (2026-10-04) A job waits for the runner's next report, so job latency is the report interval.
   The server answers 1 s while an agent works on that computer (a step in flight, a W2 task
   running there, or a step in the last 2 minutes, for the model's thinking time between steps)
   and the normal interval otherwise. Measured in a W2 run: median 0.11 s, max 0.84 s per tool
   call (it was up to 60 s). No runner change was needed.
5. (2026-10-05) Paths differ per computer. kireserver's ~/Docker is NFS-mounted on soucouyant at
   /home/khyretos/Server-Docker, not ~/Docker: the W2 demo there is
   /home/khyretos/Server-Docker/Personal-projects/kompanion-w2-demo. Never give a path for another
   computer from this server's view; check it on that computer (runner list_dir) first.
6. (2026-10-05) kireserver has its own runner (systemd user service, linger on, grants.json empty
   = deny-all), paired with `docker exec kreative-kompanion kompanion-server pair-code khyretos
   kireserver` and `kompanion-runner pair http://127.0.0.1:8095 <code>`. The Machines entry "server"
   is only the server's own stats and can't run steps.
