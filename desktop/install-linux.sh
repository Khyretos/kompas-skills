#!/bin/sh
set -eu

AUTOSTART=0

if [ $# -eq 0 ]; then
    : # No argument means install, no autostart
elif [ "$1" = "--autostart" ]; then
    AUTOSTART=1
else
    echo "Usage: $0 [--autostart]" >&2
    exit 2
fi

cd "$(dirname "$0")"

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}" nice -n 15 npx --yes @tauri-apps/cli@2 build --no-bundle

INSTALL_BIN="$HOME/.local/bin/kreative-kompanion"
install -Dm755 src-tauri/target/release/kreative-kompanion-desktop "$INSTALL_BIN"

install -Dm644 src-tauri/icons/32x32.png "$HOME/.local/share/icons/hicolor/32x32/apps/kreative-kompanion.png"
install -Dm644 src-tauri/icons/128x128.png "$HOME/.local/share/icons/hicolor/128x128/apps/kreative-kompanion.png"
install -Dm644 src-tauri/icons/128x128@2x.png "$HOME/.local/share/icons/hicolor/256x256/apps/kreative-kompanion.png"

DESKTOP_FILE="$HOME/.local/share/applications/kreative-kompanion.desktop"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Kreative Kompanion
Comment=Your AI, your machines
Exec=$INSTALL_BIN
Icon=kreative-kompanion
Categories=Development;Utility;
Terminal=false
StartupWMClass=kreative-kompanion
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications" || true
fi

if [ "$AUTOSTART" = "1" ]; then
    mkdir -p "$HOME/.config/autostart"
    cp "$DESKTOP_FILE" "$HOME/.config/autostart/kreative-kompanion.desktop"
fi

echo "Installed: $INSTALL_BIN (start it from your app menu)"
