#!/bin/sh
set -eu

# Settings at the top (overridable from the environment)
REPO=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
# .env and dist/ are not in git: take them from the main checkout (also right in a worktree).
MAIN=$(dirname "$(git -C "$REPO" rev-parse --path-format=absolute --git-common-dir)")
FIXTURE=${FIXTURE:-$REPO/../kompanion-w2-demo}
FIXTURE_REV=${FIXTURE_REV:-3562395}
IMAGE=${IMAGE:-kreative-kompanion:latest}
MODEL_CONTAINER=${MODEL_CONTAINER:-ovms}
RESULTS=${RESULTS:-$HOME/.local/share/kompanion-nightly}
PORT=${PORT:-18095}
LIVE=${LIVE:-kreative-kompanion}
OWNER=${OWNER:-khyretos}
DATE=$(date +%F)

# 1. Arguments: only `--now` is allowed
if [ $# -eq 0 ]; then
    hour=$(date +%H)
    if [ "$hour" = "03" ] || [ "$hour" = "04" ]; then
        exec "$0" --now
    else
        echo "outside 03:00-05:00, skipped" >&2
        exit 0
    fi
fi

case "$1" in
    --now) ;;
    *) echo "Usage: $0 [--now]" >&2; exit 2 ;;
esac

# 2. Check for GPU usage by studio apps
for app in comfyui comfyui-rocm heartmula moss-sfx; do
    if docker ps --format '{{.Names}}' | grep -qx "$app"; then
        echo "a studio app is using the GPU, skipped" >&2
        exit 0
    fi
done

# 3. Temp dir and cleanup trap
T=$(mktemp -d)
N="kk-nightly-$$"
RUNNER=""
cleanup() {
    kill "$RUNNER" 2>/dev/null || true
    docker rm -f "$N" >/dev/null 2>&1 || true
    docker volume rm "$N" >/dev/null 2>&1 || true
    if [ "${KEEP:-0}" = 1 ]; then echo "kept $T" >&2; else rm -rf "$T"; fi
}
trap cleanup EXIT

# 4. Fixture
git clone -q "$FIXTURE" "$T/fixture" && git -C "$T/fixture" checkout -q "$FIXTURE_REV"

# 5. Config for the throwaway server
cat > "$T/kompanion.toml" <<EOF
bind = "0.0.0.0:8080"
database = "/data/kompanion.db"
web_dir = "/app/web"
secure_cookies = false
allowed_origins = ["http://127.0.0.1:$PORT"]
[[provider]]
id = "local"
name = "Nightly model"
kind = "openai-compatible"
base_url = "http://MODEL_CONTAINER:8000/v3"
local = true
api_key_env = "OVMS_API_KEY"
extra_body = { chat_template_kwargs = { enable_thinking = false } }
[roles]
orchestrator = { provider = "local", model = "Coder" }
worker = { provider = "local", model = "Coder" }
reviewer = { provider = "local", model = "Coder" }
[features]
assets = false
gpus = false
voice = false
windshift = false
EOF
sed -i "s/MODEL_CONTAINER/$MODEL_CONTAINER/g" "$T/kompanion.toml"
sed -i "s/PORT/$PORT/g" "$T/kompanion.toml"

# 6. Network and API Key
NET=$(docker inspect "$MODEL_CONTAINER" --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' | awk '{print $1}')
KEY=$(grep '^OVMS_API_KEY=' "$MAIN/.env" | cut -d= -f2-)

docker run -d \
    --name "$N" \
    --network "$NET" \
    -p "127.0.0.1:$PORT:8080" \
    -e OVMS_API_KEY="$KEY" \
    -v "$N:/data" \
    -v "$T/kompanion.toml:/config/kompanion.toml:ro" \
    "$IMAGE" >/dev/null

# 7. Wait for server to be ready
i=0; while ! curl -sf "http://127.0.0.1:$PORT/api/status" >/dev/null; do i=$((i + 1)); [ $i -lt 60 ] || break; sleep 1; done; [ $i -lt 60 ] || {
    docker logs "$N" 2>&1 | tail -20 >&2
    exit 1
}

# 8. Account setup
CODE=$(docker logs "$N" 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -oE 'create one: [^ ]+' | awk '{print $NF}')
PW=$(head -c 18 /dev/urandom | base64 | tr -d '/+=')
api() {
    curl -sf -b "$T/jar" -c "$T/jar" \
        -H 'X-Kompanion: 1' \
        -H "Origin: http://127.0.0.1:$PORT" \
        -H 'Content-Type: application/json' \
        "$@"
}
api -d "{\"code\":\"$CODE\",\"name\":\"nightly\",\"password\":\"$PW\"}" "http://127.0.0.1:$PORT/api/setup" >/dev/null

# 9. Runner pairing and start
PAIR=$(api -d '{"name":"nightly"}' "http://127.0.0.1:$PORT/api/machines/pair-code" | python3 -c 'import json,sys;print(json.load(sys.stdin)["code"])')
OUT=$(api -d "{\"code\":\"$PAIR\",\"hostname\":\"nightly\"}" "http://127.0.0.1:$PORT/api/pair")
MID=$(echo "$OUT" | python3 -c 'import json,sys;print(json.load(sys.stdin)["machineId"])')
echo "$OUT" | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])' > "$T/token"
chmod 600 "$T/token"

cat > "$T/runner.toml" <<EOF
server = "http://127.0.0.1:$PORT"
machine_id = "$MID"
token_file = "$T/token"
grants_file = "$T/grants.json"
EOF

BIN=$(ls -1 "$MAIN"/dist/kompanion-runner-*-x86_64-linux-musl | sort -V | tail -1)
"$BIN" "$T/runner.toml" > "$T/runner.log" 2>&1 &
RUNNER=$!

# 10. Grant access
api -d "{\"target\":\"$T/fixture\",\"rights\":[\"read\",\"write\",\"shell\"],\"expires_hours\":1}" "http://127.0.0.1:$PORT/api/machines/$MID/grants" >/dev/null

# 11. Task definition
printf '%s\n' '{"projects":[{"id":"nightly","name":"Nightly","tasks":[{"id":"nightly-w2","title":"Make the initials test pass","description":"Add initials(name) to names.py so that python3 -m unittest passes. Done when: python3 -m unittest passes and nothing else changed.","state":"queued"}]}]}' > "$T/task.json"
docker exec -i "$N" sh -c 'cat > /tmp/task.json && kompanion-server import /tmp/task.json nightly' < "$T/task.json" >/dev/null

# 12. Start task
START=$(date +%s)
api -d "{\"machine_id\":\"$MID\",\"folder\":\"$T/fixture\",\"check\":\"python3 -m unittest\"}" "http://127.0.0.1:$PORT/api/tasks/nightly-w2/start" >/dev/null

# 13. Wait for completion
for i in $(seq 1 45); do
    STATE=$(api "http://127.0.0.1:$PORT/api/tasks" | python3 -c 'import json,sys;v=json.load(sys.stdin);t=v if isinstance(v,list) else v.get("tasks",[]);print(next((x["state"] for x in t if x["id"]=="nightly-w2"),"?"))')
    case "$STATE" in
        done|failed|needs_input) break ;;
    esac
    sleep 20
done

# 14. Result analysis
SECS=$(( $(date +%s) - START ))
TEST=ok
(cd "$T/fixture" && python3 -m unittest -q >/dev/null 2>&1) || TEST=failed

# Passing by editing the tests does not count.
if ! git -C "$T/fixture" diff --quiet "$FIXTURE_REV" -- 'test_*.py'; then TEST=tests-changed; fi

CALLS=$(api "http://127.0.0.1:$PORT/api/calls" | python3 -c 'import json,sys;v=json.load(sys.stdin);c=v if isinstance(v,list) else v.get("calls",[]);print(len(c))')

mkdir -p "$RESULTS"
printf '%s\n' "{\"date\":\"$DATE\",\"state\":\"$STATE\",\"test\":\"$TEST\",\"seconds\":$SECS,\"model_calls\":$CALLS,\"model\":\"Coder\"}" > "$RESULTS/$DATE.json"
cat "$RESULTS/$DATE.json"

# 15. Pass check
if [ "$STATE" = "done" ] && [ "$TEST" = "ok" ]; then
    echo "nightly W2 passed"
    exit 0
fi

# 16. Failure handling
echo "=== runner.log ===" >&2
tail -30 "$T/runner.log" >&2
echo "=== docker logs ===" >&2
docker logs "$N" 2>&1 | tail -30 >&2

printf '%s\n' "{\"projects\":[{\"id\":\"nightly-failures\",\"name\":\"Nightly checks\",\"tasks\":[{\"id\":\"nightly-w2-$DATE\",\"title\":\"Nightly W2 failed $DATE\",\"description\":\"The nightly real-model test ended $STATE (test: $TEST) after $SECS s. Numbers: $RESULTS/\$DATE.json. Rerun by hand: tools/nightly/w2.sh --now\",\"state\":\"needs_input\"}]}]}" | \
    docker exec -i "$LIVE" sh -c "cat > /tmp/f.json && kompanion-server import /tmp/f.json $OWNER; rm /tmp/f.json"

exit 1
