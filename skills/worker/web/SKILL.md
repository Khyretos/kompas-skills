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
