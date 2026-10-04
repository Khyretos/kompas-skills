#!/bin/sh
# Usage: sudo ./install.sh to install, ./install.sh --check to only check.
set -eu

cd "$(dirname "$0")"

if [ "$1" = "--check" ]; then
    # Verify files exist
    if [ ! -f kompanion-gpu-helper ] || [ ! -f kompanion-gpu-helper.sha256 ] || [ ! -f kompanion-gpu-helper.service ]; then
        echo "missing required files" >&2
        exit 1
    fi

    # Verify checksum
    if ! sha256sum -c kompanion-gpu-helper.sha256 >/dev/null 2>&1; then
        echo "checksum mismatch" >&2
        exit 1
    fi

    # Verify systemd unit if systemd-analyze is available
    if command -v systemd-analyze >/dev/null 2>&1; then
        if ! systemd-analyze verify ./kompanion-gpu-helper.service >/dev/null 2>&1; then
            echo "systemd unit verification failed" >&2
            exit 1
        fi
    fi

    echo "check ok"
    exit 0
fi

# Normal run: perform checks first, then install
if [ ! -f kompanion-gpu-helper ] || [ ! -f kompanion-gpu-helper.sha256 ] || [ ! -f kompanion-gpu-helper.service ]; then
    echo "missing required files" >&2
    exit 1
fi

if ! sha256sum -c kompanion-gpu-helper.sha256 >/dev/null 2>&1; then
    echo "checksum mismatch" >&2
    exit 1
fi

if command -v systemd-analyze >/dev/null 2>&1; then
    if ! systemd-analyze verify ./kompanion-gpu-helper.service >/dev/null 2>&1; then
        echo "systemd unit verification failed" >&2
        exit 1
    fi
fi

# Install
getent group kompanion-gpu >/dev/null || groupadd --system --gid 10050 kompanion-gpu
getent passwd kompanion-gpu >/dev/null || useradd --system --uid 10050 --gid kompanion-gpu --no-create-home --shell /usr/sbin/nologin kompanion-gpu
install -m 0755 kompanion-gpu-helper /usr/local/bin/kompanion-gpu-helper
install -m 0644 kompanion-gpu-helper.service /etc/systemd/system/kompanion-gpu-helper.service
systemctl daemon-reload
systemctl enable kompanion-gpu-helper
systemctl restart kompanion-gpu-helper

# Wait for socket (up to 15 seconds)
i=0
while [ ! -S /run/kompanion-gpu/stats.sock ] && [ $i -lt 30 ]; do
    sleep 0.5
    i=$((i + 1))
done

if [ -S /run/kompanion-gpu/stats.sock ]; then
    echo "kompanion-gpu-helper is running"
    exit 0
else
    echo "the helper did not start; see: journalctl -u kompanion-gpu-helper -n 20" >&2
    exit 1
fi
