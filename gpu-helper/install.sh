#!/bin/sh
# Installs the GPU helper on this machine (needs sudo once). Run from this folder:
#   sudo ./install.sh
set -eu
[ "$(id -u)" = 0 ] || { echo "run with sudo"; exit 1; }
getent group kompanion-gpu >/dev/null || groupadd --system --gid 10050 kompanion-gpu
install -m 0755 kompanion-gpu-helper /usr/local/bin/kompanion-gpu-helper
install -m 0644 kompanion-gpu-helper.service /etc/systemd/system/kompanion-gpu-helper.service
systemctl daemon-reload
systemctl enable --now kompanion-gpu-helper
sleep 1
ls -l /run/kompanion-gpu/stats.sock && echo "kompanion-gpu-helper is running"
