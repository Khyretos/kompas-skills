# Worker: web app (vanilla TypeScript)

1. (2026-10-01) Templates: only the escaping `html` tagged template; nested `html` values and arrays of them are fine. Never `innerHTML`, never `.join("")` on `html` arrays (it escapes the markup).
2. (2026-10-01) Clicks go through `onAction` with `data-action`; it calls `preventDefault`, so radio buttons and checkboxes must use the `change` event instead of `data-action`.
3. (2026-10-03) The settings sheet and the task editor must not re-render on unrelated store changes (live machine stats arrive every second). Add new state to the right pane's key list in `render()`.
4. (2026-10-03) `tsc` fails the build on unused imports: remove them.
5. (2026-10-03) Colours: use the tokens (`--accent`, `--accent-strong`, `--accent-soft`, `--on-accent`); never white text on orange; orange text on light only as `--accent-strong` (#8f4700).
6. (2026-10-03) Never pass a function with optional extra parameters straight to `.map()` (it receives the index as the second argument): `.map((c) => row(c))`.
