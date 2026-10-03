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
