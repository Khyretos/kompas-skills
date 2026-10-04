#!/bin/sh
# Usage: sudo ./install.sh to install, ./install.sh --check to only check, ./install.sh --selftest to run a self-test.
set -eu

cd "$(dirname "$0")"

SOCK="${KK_SOCK:-/run/kompanion-gpu/stats.sock}"

selftest() {
    i=0
    while [ $i -lt 40 ]; do
        if ./kompanion-gpu-helper --probe "$SOCK"; then
            return 0
        fi
        sleep 0.5
        i=$((i + 1))
    done
    return 1
}

if [ "${1:-}" = "--selftest" ]; then
    if [ ! -x ./kompanion-gpu-helper ]; then
        echo "missing helper binary" >&2
        exit 1
    fi
    if selftest; then
        echo "self-test ok"
        exit 0
    else
        echo "self-test failed" >&2
        exit 1
    fi
fi

if [ "${1:-}" = "--check" ]; then
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

    # The known-good fallback, when present
    if [ -f kompanion-gpu-helper-good ] && ! sha256sum -c kompanion-gpu-helper-good.sha256 >/dev/null 2>&1; then
        echo "checksum mismatch: kompanion-gpu-helper-good" >&2
        exit 1
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
[ "$(id -u)" = 0 ] || { echo "run with sudo: sudo ./install.sh"; exit 1; }
getent group kompanion-gpu >/dev/null || groupadd --system --gid 10050 kompanion-gpu
getent passwd kompanion-gpu >/dev/null || useradd --system --uid 10050 --gid kompanion-gpu --no-create-home --shell /usr/sbin/nologin kompanion-gpu
install -m 0755 kompanion-gpu-helper /usr/local/bin/kompanion-gpu-helper
install -m 0644 kompanion-gpu-helper.service /etc/systemd/system/kompanion-gpu-helper.service
systemctl daemon-reload
systemctl enable kompanion-gpu-helper
systemctl restart kompanion-gpu-helper

# Wait for socket (up to 15 seconds)
i=0
while [ ! -S "$SOCK" ] && [ $i -lt 30 ]; do
    sleep 0.5
    i=$((i + 1))
done

if [ ! -S "$SOCK" ]; then
    echo "the helper did not start; see: journalctl -u kompanion-gpu-helper -n 20" >&2
    exit 1
fi

if selftest; then
    echo "kompanion-gpu-helper $(sha256sum /usr/local/bin/kompanion-gpu-helper | cut -c1-12) is running and reports GPUs."
    exit 0
else
    if [ -f ./kompanion-gpu-helper-good ]; then
        echo "The new helper reports no GPUs; going back to the last known-good build."
        install -m 0755 kompanion-gpu-helper-good /usr/local/bin/kompanion-gpu-helper
        systemctl restart kompanion-gpu-helper
        if selftest; then
            echo "Known-good helper $(sha256sum /usr/local/bin/kompanion-gpu-helper | cut -c1-12) is running and reports GPUs."
            exit 0
        else
            echo "Still no GPUs; see: journalctl -u kompanion-gpu-helper -n 20" >&2
            exit 1
        fi
    else
        echo "Still no GPUs; see: journalctl -u kompanion-gpu-helper -n 20" >&2
        exit 1
    fi
fi
