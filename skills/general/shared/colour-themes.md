---
name: shared/colour-themes
description: Writing colour themes, palettes and CSS re-themes (editors, terminals, web apps) that stay readable.
roles: [worker, reviewer]
tags: [theme, palette, css, colours, contrast, brand]
paths: ["**/*.css", "**/*theme*", "**/*.theme", "**/*colors*"]
---
# Colour themes

From 29 CSS re-theme runs over 14 apps and the VS Code, Kate, DMS and kitty themes (2026-10-03/04).

1. Work from a role-to-colour table and the exact key list; fill every key from the table. Keys
   the table does not cover get copied from the example file, so list them all.
2. Know which roles are backgrounds: never put a foreground colour in a background role (solid
   orange search highlights hid orange keywords). Use pre-blended tints for backgrounds.
3. Each state gets its own named colour (selection is not the current line).
4. Think about what a colour is for: `cursor_text_color` is the text under the cursor, so it must
   contrast with the cursor colour.
5. Light and dark variants each get their own value table; do not derive one from the other.
7. A bare `a` or generic selector (`.card`, `.sidebar`) recolours the whole UI; touch only the
   selectors you were given.
8. Starting from a working example theme of the same app beats describing the app.
