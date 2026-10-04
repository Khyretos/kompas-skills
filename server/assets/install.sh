#!/bin/sh
set -eu

CODE="${1:-}"
if [ -z "$CODE" ]; then
    echo "Usage: curl -fsSL {{SERVER}}/install.sh | sh -s -- PAIRING-CODE"
    exit 1
fi

if [ "$(id -u)" = "0" ]; then
    echo "Run this as your normal user, not root."
    exit 1
fi

if ! { [ "$(uname -s)" = "Linux" ] && [ "$(uname -m)" = "x86_64" ]; }; then
    echo "Only 64-bit Linux (x86_64) is supported for now."
    exit 1
fi

for tool in curl sha256sum systemctl; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing $tool: install it first."
        exit 1
    fi
done

BIN="$HOME/.local/bin"
mkdir -p "$BIN"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "{{SERVER}}/download/kompanion-runner" -o "$TMP/kompanion-runner"
curl -fsSL "{{SERVER}}/download/kompanion-runner.sha256" -o "$TMP/kompanion-runner.sha256"

(cd "$TMP" && sha256sum -c kompanion-runner.sha256) || {
    echo "The download is damaged (checksum mismatch). Try again."
    exit 1
}

install -m 0755 "$TMP/kompanion-runner" "$BIN/kompanion-runner"

"$BIN/kompanion-runner" pair "{{SERVER}}" "$CODE"
"$BIN/kompanion-runner" install-service

echo "Done. This computer shows up in Kompanion within a minute. Kompanion can do nothing here until you grant access in its Access tab."
echo "To keep it running while you are logged out: loginctl enable-linger \"\$USER\""
