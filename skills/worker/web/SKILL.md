# Worker: web app (vanilla TypeScript)

1. (2026-10-01) Templates: only the escaping `html` tagged template; nested `html` values and arrays of them are fine. Never `innerHTML`, never `.join("")` on `html` arrays (it escapes the markup).
2. (2026-10-01) Clicks go through `onAction` with `data-action`; it calls `preventDefault`, so radio buttons and checkboxes must use the `change` event instead of `data-action`.
3. (2026-10-03) The settings sheet and the task editor must not re-render on unrelated store changes (live machine stats arrive every second). Add new state to the right pane's key list in `render()`.
4. (2026-10-03) `tsc` fails the build on unused imports: remove them.
5. (2026-10-03) Colours: use the tokens (`--accent`, `--accent-strong`, `--accent-soft`, `--on-accent`); never white text on orange; orange text on light only as `--accent-strong` (#8f4700).
6. (2026-10-03) Never pass a function with optional extra parameters straight to `.map()` (it receives the index as the second argument): `.map((c) => row(c))`.
7. (2026-10-03) Map names with a lookup table (`Record<string, string>`), not chained `.replace()` calls: those also hit substrings.
8. (2026-10-03) Never inline event handlers (`onchange="..."`): the CSP blocks them and they bypass `onAction`. Forms are handled by the shell's submit listener.
9. (2026-10-03) In `${cond ? list.map(...) : html`...`}` the `}` comes after the whole ternary; a stray `)}` after the map closes the expression early (tsc: "':' expected").

10. Before you output a file, count the braces of the last function. An extra `}` at the end
    of a module is a syntax error that breaks the whole build. When told to remove one, check
    the last 3 lines of your output.
11. Never make the user refresh (Kees, standing rule). Every action that changes data
    updates the store at once (optimistic), rolls back with an error toast on failure,
    and disables its button with a spinner while pending. Live changes from the server
    arrive over the `/api/events` SSE stream. Each page has a Playwright test that does an
    action and checks the screen without reloading.
12. Code you are given "at module level" or "after the function" stays outside the function.
    State that two exported functions share (like a `busy` Set) must be declared at the top
    level of the module, or the second function can't see it.
13. Playwright: `expect()` takes a Locator (`expect(page.locator("#left"))`), never a selector string.
    A Locator's `fill(value)` and `click()` take no selector; narrow first with
    `.locator(sel)`, `.first()` or `.filter({ hasText })`. Counts use `toHaveCount(n)` or
    `expect(await l.count()).toBeGreaterThan(0)`. "Gone" is
    `expect(page.locator("li", { hasText: t })).toHaveCount(0)`.
14. When you remove lines with an edit block, SEARCH for exactly those lines. Never include the
    line that opens the surrounding block (`test.beforeEach(async ({ page }) => {`) unless the
    REPLACE keeps it.
15. Every overlay (modal, sheet, dialog) closes three ways: a click on the backdrop outside the
    window (only when both pointerdown and pointerup land on the backdrop, so dragging a text
    selection out doesn't close it), a visible × button top-right, and Escape. If a form inside
    has unsaved edits, ask before closing. Return focus to the element that opened it. Use the
    shared helper `core/modal.ts`; never write a one-off.
16. `null` is not `undefined`. A field typed `string | null` needs a truthiness check (`!!a.result`)
    or `?? ""`, never `!== undefined`; otherwise a string function gets `null` and the whole
    render throws, so nothing shows.
17. `html``…`` drops `false`: `aria-pressed="${x === y}"` renders `aria-pressed=""` when false.
    Write `aria-pressed="${String(x === y)}"` for every true/false attribute (Assets chips, 2026-10-04).
18. A CSS file pulled in with `@import` comes before every rule of the file that imports it. With
    equal specificity the later `.pane { display: flex }` beat `.assets-pane { display: none }`, so
    the hidden section showed. Raise the selector (`.shell .assets-pane`) instead of `!important`.
19. Long lists (thousands of rows) are virtualised: a spacer with the full height, and only the
    visible rows plus two above and below in the DOM, positioned with `transform`; fetch pages of
    200 by offset and drop answers from an older filter (a generation counter). See
    `src/views/assets.ts` (47,000 assets, 20 cards in the DOM).
17. A test must do what its name says. Don't leave the action as a comment ("// check the box");
    write the call (`await page.check("#activity-failed")`). Use the class names the view
    really renders.
20. A mock that hands its objects to the view must replace changed objects, never edit them in
    place: the view holds the same objects, so an in-place edit shows up without any event and a
    "live update" test passes for the wrong reason. `this.items = this.items.map((a) => changed.has(a.id) ? { ...a, x } : a)`.
21. An `<img>` in a fixed-height grid cell (`display: grid`) can stretch the cell. Give the box
    `position: relative; overflow: hidden` and the img `position: absolute; inset: 0; object-fit: contain`.
22. A button inside a card button is invalid HTML (and breaks clicks). Put the second button
    (play) next to the card button in the same `li`, positioned over it.
23. Media served to `<audio>` needs HTTP Range support (Safari will not play without it):
    tower-http `ServeFile` gives it; check with a `Range: bytes=0-99` request expecting 206.
