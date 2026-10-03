# Runner

Lessons for the runner role. Numbered and dated, newest last.
1. (2026-10-03) Privilege goes into tiny sandboxed helpers, never into the runner or the server. Example: kompanion-gpu-helper runs as its own system user with only CAP_SYS_PTRACE (the kernel also requires the reader to hold every capability of the target, so an empty-cap root helper reads nothing from container processes), reads only drm-* lines, serves aggregates on a group-only unix socket, no network, no input. The unprivileged side treats the helper's answer as untrusted (size limit, clamped values) and works without it.
