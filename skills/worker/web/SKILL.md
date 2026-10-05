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
17. `html``…`` drops`false`:`aria-pressed="${x === y}"` renders `aria-pressed=""` when false.
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
24. (2026-10-04) Never keep UI state ("this step is open") from the native `toggle` event: it fires
    as a later task, so a re-render that lands between the click and the event replaces the
    `<details>` and the choice is lost (pcagent flake, 1 in ~30 runs). Toggle through an
    `onAction` handler on the `<summary>` that records the state first and then sets `d.open`.
    Prove a flake fix with `npx playwright test <file> --repeat-each=30`, never with retries or skips.
25. (2026-10-04) No `test.skip` inside a test. "Unsaved edits ask first" checked for the e-mail field
    before it opened Settings, so it skipped on every run for a day and nobody noticed. If a test
    answers dialogs itself, call `page.removeAllListeners("dialog")` first: two handlers on one
    dialog throw "already handled".
26. (2026-10-04) In `main.ts`, `wire(shell)` gets the outer root, not `.shell`. An event for code that
    listens on `.shell` (resize.ts) must be dispatched on `$(".shell")`: events bubble up, never down.
    (Opening a task didn't re-open a collapsed panel; the Playwright test caught it.)
27. A field inside a pane that re-renders while you type (a search box) needs its value in the store
    on every keystroke, and `remount` keeps the caret (it saves selectionStart/End). Debounce only
    the request, and drop answers for an older query.
28. A list row that should do something is a `<button>` inside the `<li>` with a `data-action`, never
    a bare `<li>` (the sidebar task lines were dead for that reason). Test the click in Playwright.
29. (2026-10-04) A class that sets `display` (`.btn` is inline-flex) beats the `hidden` attribute, so
    `el.hidden = true` showed nothing. `.btn[hidden] { display: none; }` is in styles.css; for any
    other displayed class you toggle with `hidden`, add the same `[hidden]` rule. Test hidden state
    with `toBeHidden()`.
30. (2026-10-05) Remember UI state by the identity of the thing itself, never by its current
    container: step groups were keyed by message id, and the steps moved to the reply message when
    it arrived, so the user's open/closed choice was lost. Key by the first step's id.
31. (2026-10-05) Search-as-you-type: number every query and drop answers that are not the newest;
    clear the old list only when the new answer arrives (clearing on each keystroke flickers); keep
    the shown results in an array and open `shown[index]` (Qwen read a `data-result` it never set).
    Text from the user or the server goes in with textContent, never innerHTML, also in "Nothing
    found for ..." lines. Static markup too: the app enforces Trusted Types, so `el.innerHTML = ...`
    throws and the overlay never opened (all five Playwright tests failed in CI). Build overlays
    with createElement/append, or the `html` template and `mount`.
32. (2026-10-05) A live re-render between mousedown and mouseup swallows the click: the press
    landed on an element that is gone, so the browser sends no click. On a busy CI machine this
    made "close the step group" and the voice button fail now and then; real users lose clicks the
    same way while steps run. `onAction` now runs the action on pointerup when the pressed element
    was replaced by its twin (same action, id, data-id). Measure "held down" from pointerdown to
    pointerup, never to when the click handler runs (jank adds to it), and let Stop wait for a
    microphone that is still opening, or the recording comes back empty.
33. (2026-10-05) A pane that re-renders on live updates must keep what the user typed: the Run
    form's Folder field was emptied by a task progress tick, so Start did nothing. `remount` keeps
    fields marked `data-edited` (set on input/change, cleared on submit) and refocuses a field by
    form + name when it has no id. When the pointerup fallback ran an action, skip the click
    Chrome may still send to the twin, or a toggle runs twice and cancels out.
34. (2026-10-05) Playwright's click() waits until the element stops moving; a message that
    re-renders 20-30 times a second (steps streaming) delays it by seconds under load, so the
    click lands in a later state (the step group had already closed itself). For state that
    changes by itself, read the state and click in one page.evaluate, then assert the opposite.
