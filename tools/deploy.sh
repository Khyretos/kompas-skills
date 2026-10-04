#!/bin/sh
# Deploys Kompanion on kireserver. Refuses to build from a tree that is behind
# origin/main: production may already have migrations from other branches
# (2026-10-04: a build without 0100_assets.sql crash-looped the server).
set -eu
cd "$(dirname "$0")/.."
git fetch -q origin main
if ! git merge-base --is-ancestor origin/main HEAD; then
    echo "deploy: HEAD does not contain origin/main. Merge it first: git merge origin/main" >&2
    exit 1
fi
nice docker compose build -q
docker compose up -d
for i in $(seq 1 30); do
    if curl -sf -o /dev/null http://127.0.0.1:8095/api/status; then
        echo "deploy: $(git rev-parse --short HEAD) is up"
        exit 0
    fi
    sleep 1
done
echo "deploy: not answering; see docker logs kreative-kompanion" >&2
exit 1
