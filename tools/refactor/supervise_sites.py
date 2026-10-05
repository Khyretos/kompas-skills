"""Wraps the first `tokio::spawn(async move {` inside a named function into
crate::util::supervise(name, move || { <clones>; async move { ... }}). Brace-matched, so the
body is untouched. Usage: run from server/; prints each change."""
import re, sys

SITES = [  # file, function signature start, job name, variables to clone per run
    ("src/gpus/mod.rs", "pub fn spawn(s: AppState)", "gpus", ["s"]),
    ("src/hoststats.rs", "pub fn spawn_live(state: AppState)", "machine-stats", ["state"]),
    ("src/notify.rs", "pub fn spawn_daily(db: SqlitePool)", "daily-summary", ["db"]),
    ("src/windshift.rs", "pub fn spawn(db: SqlitePool, ws: std::sync::Arc<Windshift>)", "windshift-sync", ["db", "ws"]),
    ("src/import.rs", "pub fn watch(s: crate::AppState)", "import-watch", ["s"]),
    ("src/assets/mod.rs", "pub fn spawn(state: AppState)", "asset-games", ["state"]),
]

def match_brace(text, open_idx):
    depth, i, n = 0, open_idx, len(text)
    in_str = None
    while i < n:
        c = text[i]
        if in_str:
            if c == "\\": i += 2; continue
            if c == in_str: in_str = None
        elif c == '"': in_str = '"'
        elif c == "/" and text[i:i+2] == "//":
            i = text.index("\n", i); continue
        elif c == "{": depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0: return i
        i += 1
    raise ValueError("no matching brace")

for path, sig, name, clones in SITES:
    t = open(path).read()
    f = t.index(sig)
    s = t.index("tokio::spawn(async move {", f)
    brace = s + len("tokio::spawn(async move ")
    end = match_brace(t, brace)
    assert t[end:end+2] == "})", (path, t[end:end+5])
    body = t[brace:end+1]
    lets = " ".join(f"let {v} = {v}.clone();" for v in clones)
    new = f'crate::util::supervise("{name}", move || {{ {lets} async move {body} }})'
    t = t[:s] + new + t[end+2:]
    open(path, "w").write(t)
    print(f"{path}: {name} supervised ({len(body.splitlines())} lines kept)")
