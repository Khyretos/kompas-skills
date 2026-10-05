#!/bin/sh
set -eu

KEYS="${KK_ANDROID_KEYS:-$HOME/.config/kompanion-android}"

if [ -f "$KEYS/release.jks" ]; then
    echo "Key already exists: $KEYS/release.jks (never overwrite it: phones only accept updates signed with the same key)"
    exit 0
fi

umask 077
mkdir -p "$KEYS"

PASS=$(head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 32)

docker run --rm -u "$(id -u):$(id -g)" -v "$KEYS":/keys eclipse-temurin:21-jdk-noble \
    keytool -genkeypair -keystore /keys/release.jks -storetype PKCS12 -alias kompanion \
    -keyalg RSA -keysize 4096 -validity 10000 -storepass "$PASS" -keypass "$PASS" \
    -dname "CN=Kreative Kompanion, O=Kreative Kompas"

printf 'KK_KEYSTORE_PASS=%s\n' "$PASS" > "$KEYS/keystore.env"
printf 'KK_KEY_ALIAS=%s\n' "kompanion" >> "$KEYS/keystore.env"
printf 'KK_KEY_PASS=%s\n' "$PASS" >> "$KEYS/keystore.env"

chmod 600 "$KEYS/release.jks" "$KEYS/keystore.env"

echo "Signing key made in $KEYS. Store keystore.env's password in Passbolt and back up release.jks; losing it means users must uninstall to update."
