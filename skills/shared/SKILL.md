# Shared

Lessons for the shared role. Numbered and dated, newest last.
## 1. Theming: readability is checked by a script, not by eye (2026-10-03)

Source: Kreative Kompas VS Code theme (repo kreative-kompas-vscode-theme, `scripts/check-contrast.mjs`).

- Main accent orange `#f3941f`; secondary accents get distinct roles: blue `#82aaff` (functions, links),
  green `#b5e48c` (strings, added), red `#ff8a8a` (tags, errors, deleted), lilac `#cca9ff` (types).
  "Orange is the main accent" means focus, active state and keywords, not every colour.
- Text on orange is always night `#0c0917` (8.5:1). White on orange is 2.3:1 and fails.
- Violet `#5c398e` is a background/border colour only: 2.1:1 on plum `#1c1329`, never text.
- Every text colour must hit 4.5:1 on the background *and* on every state overlay
  (current line, selection, find match). Blend alpha overlays onto the background before measuring.
- Selection, current line and find match must be different hues, not shades of the same plum.
  Selection `#3d2f78` (blue-violet), current line plum tint plus a border, find match orange box.
- Keep bracket-pair and other decoration colours off the keyword hue, or punctuation reads as a keyword.
- Neighbouring hues (salmon next to orange) blur at a glance; check a screenshot of real code.

Test: `node scripts/check-contrast.mjs` exits non-zero on any failing pair.

## 2. Desktop theming on soucouyant: change the source, not the generated files (2026-10-03)

Soucouyant runs Hyprland (Lua config, `~/.config/hypr/hyprland.lua`) with DankMaterialShell (DMS).
DMS regenerates most app colours from one theme file, so edit that and let it propagate.

- Theme: `~/.config/DankMaterialShell/themes/kreativeKompas/theme.json`, selected with
  `customThemeFile` + `currentThemeName: "custom"` in `DankMaterialShell/settings.json`.
  DMS watches the file and regenerates within seconds: kitty `dank-theme.conf`, GTK `dank-colors.css`,
  qt6ct `colors/matugen.conf`, KDE `~/.local/share/color-schemes/DankMatugen*.colors`.
- Generated outputs are overwritten on every theme change. Put fixes in a file that is included
  *after* them (kitty: `include kreative-kompas.conf` after `include dank-theme.conf`).
- DMS's kitty ANSI palette (dank16) is derived from the primary colour and is unusable here
  (blue = orange, magenta = selection colour). The kitty override restores the VS Code theme's ANSI set.
- GTK 4 `gtk.css` only imported `colors.css` (Breeze/KDE); add `@import 'dank-colors.css';` after it.
  GTK 3 `gtk.css` is a stale copy of dank-colors plus `@import 'colors.css'`; refresh the copy.
- KDE/Qt apps follow `kdeglobals`: `plasma-apply-colorscheme DankMatugenDark` (works without Plasma running).
- Hyprland borders live in `hyprland.lua` `general.col`; `hyprctl reload`, then confirm with
  `hyprctl getoption general:col.active_border`.
- Back up first: `~/.config-backups/kreative-kompas-theme-<timestamp>.tar.gz` holds the previous state.
- Reload without restarting apps: kitty `pkill -USR1 -x kitty`; Code - OSS picks up settings.json live.
- Don't send a full-desktop screenshot as proof: the screen usually shows private documents.

## 3. Brand colour roles: purple first, orange second, text white (2026-10-03, from Kees)

Kees's correction after the desktop theme: "purple is a main color and orange secondary,
i will always prefer my text white or at least readable".

- Primary (fills, active workspace, selection, GTK/KDE accent): purple, lilac `#cca9ff` with night `#0c0917` text.
- Secondary (highlights, focus details, a few icons and links): orange `#f3941f`.
- Body and label text: white / mist `#e9e1f7`. Never colour ordinary UI text orange (or any accent).
- In DMS keep `widgetColorMode: "default"`; "colorful" paints bar text in the primary colour.
- The VS Code theme's orange keywords are fine: that is syntax colouring, not UI text.
- Lesson 2 originally set DMS `primary` to orange; that was wrong and is fixed in theme.json v1.1.0.

## 4. Web app theming: orange accents, purple steps (2026-10-03, from Kees)

Kees asked for more orange and more shades of purple in every themed web app. Rule (full text in `~/Docker/Personal-projects/kreative-kompas/brand/palette.md`, "Accents and surfaces"):

- Links, icons, active markers, focus rings and secondary buttons: orange `#f3941f` on dark, orange-ink `#8a5a00` on light; light-mode active items get `#fde7cc` behind plum text.
- Purple stays primary: primary buttons violet with white text. Apps with a single theme colour (Nextcloud, Outline, Paperless) keep violet, because that colour also fills primary buttons and colours link text in both modes.
- Surfaces step: night page, plum sidebars/panels, plum-2 cards/inputs, violet header bars with white text. On a violet bar use the lilac logo `brand/logo-on-dark.svg` and orange buttons with plum text; the normal logo's violet disappears there.
- Ordinary UI text (nav labels, chat lists) stays white/mist or plum, never orange (lesson 3). Colour icons and indicator bars, not labels.

## 3. Waiting for a background process (2026-10-03)

`pgrep -f <name>` also matches the shell that runs the wait loop (its command line contains the name), so `until ! pgrep -f x` never ends. Wait on the PID instead (`wait $pid`, or `while kill -0 $pid`), or match with a pattern that can't match itself (`pgrep -f '[p]ipeline.py'`).

## 4. Inspecting containers without leaking secrets (2026-10-03)

Never print environment values when inspecting a container: a grep for a model name matched "14B" inside an SMTP password and printed it. List variable names only, e.g. `docker inspect <c> | jq -r '.[0].Config.Env[] | split("=")[0]'`; when a value is really needed, print it only for names that don't match `PASS|PWD|KEY|SECRET|TOKEN|CREDENTIAL|AUTH` (case-insensitive). The same goes for `.env` files: `grep -oE '^[A-Z_]+='`.

## 5. Ollama hosts: one model name, and what the RAM is (2026-10-03)

Source: soucouyant benchmark, ~/Docker/docs/ai-capability/soucouyant-model-benchmark-2026-10.md.

- Ollama 0.35 runs models through llama.cpp's llama-server. It keeps a prompt cache in system RAM
  (`--cache-ram`, default 8192 MiB) that fills after a few hundred different prompts: that, not the model,
  is the ~8 GB of RAM next to ~12 GB of VRAM. `journalctl -u ollama | grep "cache state"` shows it.
  Cap it with `Environment="LLAMA_ARG_CACHE_RAM=2048"` in the service override.
- Every distinct model name (including `:14b-16k` style context variants) is a separate load. Two callers
  on one 16 GB card with different names make Ollama reload on almost every request. Agree one name per host.
- Before benchmarking or swapping models on a shared host, check `journalctl -u ollama --since -3m | grep GIN`
  for other callers and ask them to pause; a swap stalls their jobs and spoils the timings.
- A model that doesn't fit is split by layers onto the CPU (qwen3.6:35b-a3b: 45% CPU, 26 tok/s, ~20 GB RAM).
  Check `ollama ps` says 100% GPU before trusting a speed number.

## 5. CI jobs never use the host toolchain (2026-10-03)

Every job runs in a container image (rust:1, rust:1-alpine, node:22); a broken system update on the runner host must not break builds.

## 6. One model tag at a time on a shared GPU (2026-10-03)

Every request naming a different tag makes Ollama swap models (~45 s each). Drafting scripts, Kompanion roles and other tools on the same Ollama must use the same tag; after a switch, update the defaults first, then start jobs. Health checks only list models (`/v1/models`, `/api/tags`, `/api/ps`), never generate.

## 7. Batch jobs on a shared GPU run at night (2026-10-03, from Kees)

A batch job that calls a model on a machine people also use by day (kk-localize's judge on soucouyant's Ollama) only calls it in a night window: kk-localize starts at 00:30, finishes the language it is on and stops calling the judge at 07:00, and leaves the rest for the next night. Daytime and manual runs skip those calls (`FORCE_JUDGE=1` overrides) and leave the work pending instead of failed. Otherwise Ollama swaps models against the daytime drafting model (seconds become tens of seconds per request). Check other scheduled jobs before picking the time (kk-engine CI runs at 03:00).

## 6. Ollama/llama-server CPU use with the model fully on the GPU (2026-10-03)

- llama-server's CPU thread pool spin-waits (`--poll 50` by default) between GPU steps, so a model that is
  100% on the GPU still burned ~3 cores while generating (qwen3:0.6b test: 313% CPU by default, 31% with
  `-t 1` or `--poll 0`, same tok/s). Idle it uses nothing; a steady job like kk-localize keeps it spinning.
- Fix without sudo: `PARAMETER num_thread 1` in the model's Modelfile (Ollama passes it as `-t 1`). Rebuild
  the same tag (`FROM <tag>` + parameters, `ollama create <tag>`) so callers keep one name. On soucouyant
  gemma4:12b-it-qat went from ~290% to ~32% of one core with no speed loss (67 tok/s, 2,700 tok/s prompt).
- Only for models that are fully on the GPU: a partly offloaded model needs its CPU threads.

## 8. Language priority: English, Spanish, Dutch (2026-10-03, from Kees)

Kees's languages, in order: English, Spanish, Dutch. Every other language is for reach. Every system he runs should offer at least English and Spanish. On the website, Spanish and Dutch are tier 1: always published, listed right after English, audited in full (natural, neutral Spanish; natural Dutch; the CV in the first person), and if they fail the automatic publish rule the last good version stays and the failure is fixed with pins.
