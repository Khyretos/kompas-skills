---
name: worker/web/css-layout
description: CSS and layout in the web app: colour tokens, bright text, @import order, grids, display and hidden, long lists.
roles: [worker, reviewer]
tags: [css, style, styles, colour, colours, color, layout, grid, contrast, scroll, virtualised]
paths: ["**/*.css"]
---
# Worker: web app (vanilla TypeScript): css layout

18. A CSS file pulled in with `@import` comes before every rule of the file that imports it. With
    equal specificity the later `.pane { display: flex }` beat `.assets-pane { display: none }`, so
    the hidden section showed. Raise the selector (`.shell .assets-pane`) instead of `!important`.
19. Long lists (thousands of rows) are virtualised: a spacer with the full height, and only the
    visible rows plus two above and below in the DOM, positioned with `transform`; fetch pages of
    200 by offset and drop answers from an older filter (a generation counter). See
    `src/views/assets.ts` (47,000 assets, 20 cards in the DOM).
21. An `<img>` in a fixed-height grid cell (`display: grid`) can stretch the cell. Give the box
    `position: relative; overflow: hidden` and the img `position: absolute; inset: 0; object-fit: contain`.
29. (2026-10-04) A class that sets `display` (`.btn` is inline-flex) beats the `hidden` attribute, so
    `el.hidden = true` showed nothing. `.btn[hidden] { display: none; }` is in styles.css; for any
    other displayed class you toggle with `hidden`, add the same `[hidden]` rule. Test hidden state
    with `toBeHidden()`.
44. (2026-10-04) CSS that looks fine in review but is wrong on screen: a conic-gradient checkerboard
    needs hard stops, `box-shadow: inset 0 0 0 0` is invisible, an icon button needs `fill`.
45. (2026-10-03) DOM: insert the wrapper before the node, then move the node in; `appendChild(pre)`
    followed by `replaceChild(..., pre)` throws.
