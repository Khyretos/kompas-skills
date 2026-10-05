#!/bin/sh
set -eu

cd "$(dirname "$0")"

KEYS="${KK_ANDROID_KEYS:-$HOME/.config/kompanion-android}"

if [ ! -f "$KEYS/release.jks" ] || [ ! -f "$KEYS/keystore.env" ]; then
    echo "Error: Missing keystore files in $KEYS." >&2
    echo "Required: release.jks and keystore.env" >&2
    echo "Run 'sh keystore.sh' once to generate them." >&2
    exit 1
fi

nice -n 15 docker build -q -t kompanion-android-build -f Dockerfile.build .

VERSION_CODE="${VERSION_CODE:-$(git rev-list --count HEAD)}"

CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/kompanion-android"
mkdir -p "$CACHE"

docker run --rm \
    -u "$(id -u):$(id -g)" \
    -e HOME=/tmp \
    -e GRADLE_USER_HOME=/cache/gradle \
    -e VERSION_CODE="$VERSION_CODE" \
    --env-file "$KEYS/keystore.env" \
    -e KK_KEYSTORE=/keys/release.jks \
    -v "$PWD":/src \
    -v "$KEYS":/keys:ro \
    -v "$CACHE":/cache \
    kompanion-android-build gradle --no-daemon -q assembleRelease

mkdir -p dist
cp app/build/outputs/apk/release/app-release.apk dist/kompanion.apk
echo "APK: android/dist/kompanion.apk (versionCode $VERSION_CODE)"
