# Runner

Lessons for the runner role. Numbered and dated, newest last.
1. (2026-10-03) Privilege goes into tiny sandboxed helpers, never into the runner or the server. Example: kompanion-gpu-helper runs as root with an empty capability set (root reads root-owned processes' fdinfo through the same-uid check, no CAP_SYS_PTRACE), reads only drm-* lines, serves aggregates on a group-only unix socket, no network, no input. The unprivileged side treats the helper's answer as untrusted (size limit, clamped values) and works without it.
