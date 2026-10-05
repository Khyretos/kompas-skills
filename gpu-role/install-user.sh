#!/bin/sh
# Installs kompanion-gpu-role as a systemd user service of the current user (no root).
# The Kompanion container mounts ~/.local/run/kompanion-gpu-role at /host-gpu-role and
# runs with gid 1000, so the socket (mode 0660) is reachable from it and nobody else.
set -eu
cd "$(dirname "$0")"
cargo build --release -q
install -Dm755 target/release/kompanion-gpu-role "$HOME/.local/bin/kompanion-gpu-role"
install -Dm644 kompanion-gpu-role.service "$HOME/.config/systemd/user/kompanion-gpu-role.service"
mkdir -p "$HOME/.local/run/kompanion-gpu-role"
chmod 0750 "$HOME/.local/run/kompanion-gpu-role"
systemctl --user daemon-reload
systemctl --user enable --now kompanion-gpu-role.service
systemctl --user restart kompanion-gpu-role.service
# Self-test: the helper answers "status" with JSON.
i=0
while [ ! -S "$HOME/.local/run/kompanion-gpu-role/role.sock" ] && [ $i -lt 20 ]; do sleep 0.5; i=$((i + 1)); done
printf 'status\n' | python3 -c 'import socket,sys,json; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(b"status\n"); d=b"".join(iter(lambda: s.recv(65536), b"")); v=json.loads(d); print("self-test:", "ok" if v["ok"] else "failed"); print(v["output"][-400:])' "$HOME/.local/run/kompanion-gpu-role/role.sock"
