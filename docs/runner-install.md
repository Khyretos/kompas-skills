# Installing kompanion-runner on a PC

The runner reports the PC's CPU, RAM, disk and GPU stats to Kompanion. It only
makes outbound HTTPS requests; nothing listens on the PC. Linux, x86-64.

## 1. Pair the PC (in the app)

Machines tab → "Pair a computer" → name (e.g. `soucouyant`) → Pair. The app
shows the machine id, the token (only once) and the config below.

## 2. Install the binary

Static binary, no dependencies. Built by CI on soucouyant (artifact
`kompanion-runner-x86_64-linux-musl`), or locally with
`cd runner && cargo build --release`.

```bash
install -Dm755 kompanion-runner-x86_64-linux-musl ~/.local/bin/kompanion-runner
```

On soucouyant, `~/Docker` from kireserver is mounted over NFS; the binary is at
`~/Docker/Personal-projects/kreative-kompanion/dist/kompanion-runner-x86_64-linux-musl`.

## 3. Config and token

```bash
mkdir -p ~/.config/kompanion-runner && chmod 700 ~/.config/kompanion-runner
# paste the token from the app into this file:
install -m600 /dev/null ~/.config/kompanion-runner/token && nano ~/.config/kompanion-runner/token
```

`~/.config/kompanion-runner/config.toml`:

```toml
server = "https://kompanion.kreative-kompas.com"
machine_id = "<id shown in the app>"
token_file = "~/.config/kompanion-runner/token"
```

## 4. Run it as a systemd user service

`~/.config/systemd/user/kompanion-runner.service`:

```ini
[Unit]
Description=Kreative Kompanion runner (machine stats)
After=network-online.target

[Service]
ExecStart=%h/.local/bin/kompanion-runner
Restart=always
RestartSec=10

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now kompanion-runner
loginctl enable-linger "$USER"   # keep it running when logged out
journalctl --user -u kompanion-runner -f
```

The PC shows up in the Machines tab within a minute. It reports every 60 s
when nobody looks, at the slider's rate while the tab is open, and every
second on "Live". Exit code 3 means the token was refused: pair again.

## What it reads

`/proc/stat`, `/proc/meminfo`, `/proc/uptime`, `/proc/loadavg`, the root
filesystem's usage, and per GPU `/sys/class/drm/cardN/device` plus its hwmon
(amdgpu: busy %, VRAM, power, temperature, clocks, fan; Intel i915/xe: clocks,
temperature, power from the energy counter, busy share from idle residency).
No root needed.
