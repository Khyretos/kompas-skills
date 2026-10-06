---
name: worker/web
description: Writing the vanilla TypeScript web app and its Playwright tests.
roles: [worker, reviewer]
tags: [typescript, web, css, html, playwright]
paths: ["web/**"]
---
# Worker: web app (vanilla TypeScript)

Topic lessons moved into cards (live-updates, playwright, css-layout, controls), loaded when a job needs them. Add new lessons to the card they belong to.

1. (2026-10-01) Templates: only the escaping `html` tagged template; nested `html` values and arrays of them are fine. Never `innerHTML`, never `.join("")` on `html` arrays (it escapes the markup).
2. (2026-10-01) Clicks go through `onAction` with `data-action`; it calls `preventDefault`, so radio buttons and checkboxes must use the `change` event instead of `data-action`.
4. (2026-10-03) `tsc` fails the build on unused imports: remove them.
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
14. When you remove lines with an edit block, SEARCH for exactly those lines. Never include the
    line that opens the surrounding block (`test.beforeEach(async ({ page }) => {`) unless the
    REPLACE keeps it.
16. `null` is not `undefined`. A field typed `string | null` needs a truthiness check (`!!a.result`)
    or `?? ""`, never `!== undefined`; otherwise a string function gets `null` and the whole
    render throws, so nothing shows.
17. `html``…`` drops`false`:`aria-pressed="${x === y}"` renders `aria-pressed=""` when false.
    Write `aria-pressed="${String(x === y)}"` for every true/false attribute (Assets chips, 2026-10-04).
42. (2026-10-04) Never mutate store items in place, even when it looks harmless: replace the
    object (`items.map((x) => x.id === id ? { ...x, done: true } : x)`), or keyed updates miss it.
43. (2026-10-04) Union-typed fields in helper return types: use `Pick<Type, "a" | "b">`, not `string`.
46. (2026-10-04) Boolean attributes in `html` templates: `${x ? "selected" : ""}`, never `${String(x)}`.
    Use only icon names that exist in `views/icons.ts`. One small function per template branch.
47. (2026-10-05) "Plain JavaScript" means no type annotations; run `node --check` on every `.mjs`.
