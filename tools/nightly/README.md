# Nightly test with the real model (TEN-04)

`w2.sh` runs one real task end to end every night: a throwaway Kompanion server from the deployed image (`kreative-kompanion:latest`), on the model server's Docker network, with the worker, orchestrator and reviewer all on the local model; a paired runner; a grant for a copy of the fixture repo `kompanion-w2-demo` at commit 3562395; the task "make the initials test pass"; up to 15 minutes.

- Pass: the task ends done, `python3 -m unittest` passes in the fixture and no test file changed. A run that edits the tests to pass fails (`"test":"tests-changed"`).
- Numbers per night: `~/.local/share/kompanion-nightly/<date>.json` (state, test, seconds, model calls).
- On failure it adds the task "Nightly W2 failed <date>" to the live board (needs you).
- Runs only between 03:00 and 05:00, and skips when a studio app (ComfyUI, HeartMuLa, MOSS) is running. By hand: `sh tools/nightly/w2.sh --now`; keep the temp folder and the server's data volume (its database) with `KEEP=1`; test without touching the live board with `LIVE=none`.

Install the timer (systemd user units):
```sh
install -Dm644 tools/nightly/kompanion-nightly.service tools/nightly/kompanion-nightly.timer -t ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now kompanion-nightly.timer
```
