# kompanion-gpu-role (M6-03)

Switches kireserver's A770 between **coder** (OVMS Coder loaded) and **artist** (Coder
unloaded, one studio app: ComfyUI, HeartMuLa or MOSS sound effects) through
`Services/ai/gpu-share/gpu-mode.sh`. Kees approved automatic switching on 2026-10-05.

- The helper is a systemd user service of khyretos (`./install-user.sh`). It listens on
  `~/.local/run/kompanion-gpu-role/role.sock` (0660) and accepts ONE line from a fixed list:
  `status`, `artist`, `coder`, `studio comfyui|heartmula|moss-sfx`, `restart-ovms`,
  `ovms-log`. No shell, no other arguments; one command at a time.
- Kompanion (server/src/gpus/role.rs) decides with `role_policy::decide` on every scheduler
  tick: never during a running GPU job, at most one switch per 5 minutes, coder is the
  normal state. After switching back it checks that Coder answers a real chat within 60 s,
  else restarts OVMS (CL_INVALID_EVENT) and waits two more minutes.
- Deploy (docker-compose.override.yml on kireserver):
  `- /home/khyretos/.local/run/kompanion-gpu-role:/host-gpu-role` under volumes and
  `GPU_ROLE_GPU: a770` under environment. Without GPU_ROLE_GPU nothing switches.
- Undo: `Services/ai/gpu-share/gpu-mode.sh coder`, or unset GPU_ROLE_GPU and
  `systemctl --user disable --now kompanion-gpu-role`.
- Live test (2026-10-05, `kompanion-server gpu-role comfyui` then `gpu-role coder`):
  coder → artist 3.0 s (ComfyUI answering after ~19 s), artist → coder 33 s with Coder
  answering a chat 0.8 s after loading; Coder away ~61 s in all; no OVMS restart needed.
