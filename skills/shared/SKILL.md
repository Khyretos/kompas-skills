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
