---
name: worker/web/playwright
description: Playwright tests and screenshots for the web app: locators, waits, mocks, running browsers in a container.
roles: [worker, reviewer]
tags: [playwright, test, tests, e2e, screenshot, screenshots, mock, demo]
paths: ["web/tests/**", "web/tests-real/**", "web/playwright*.ts", "docs/screenshots/**"]
---
# Worker: web app (vanilla TypeScript): playwright

13. Playwright: `expect()` takes a Locator (`expect(page.locator("#left"))`), never a selector string.
    A Locator's `fill(value)` and `click()` take no selector; narrow first with
    `.locator(sel)`, `.first()` or `.filter({ hasText })`. Counts use `toHaveCount(n)` or
    `expect(await l.count()).toBeGreaterThan(0)`. "Gone" is
    `expect(page.locator("li", { hasText: t })).toHaveCount(0)`.
17. A test must do what its name says. Don't leave the action as a comment ("// check the box");
    write the call (`await page.check("#activity-failed")`). Use the class names the view
    really renders.
20. A mock that hands its objects to the view must replace changed objects, never edit them in
    place: the view holds the same objects, so an in-place edit shows up without any event and a
    "live update" test passes for the wrong reason. `this.items = this.items.map((a) => changed.has(a.id) ? { ...a, x } : a)`.
25. (2026-10-04) No `test.skip` inside a test. "Unsaved edits ask first" checked for the e-mail field
    before it opened Settings, so it skipped on every run for a day and nobody noticed. If a test
    answers dialogs itself, call `page.removeAllListeners("dialog")` first: two handlers on one
    dialog throw "already handled".
34. (2026-10-05) Playwright's click() waits until the element stops moving; a message that
    re-renders 20-30 times a second (steps streaming) delays it by seconds under load, so the
    click lands in a later state (the step group had already closed itself). For state that
    changes by itself, read the state and click in one page.evaluate, then assert the opposite.
37. (2026-10-05) kireserver has no Playwright browser on the host. Run browser scripts and tests
    in the image matching `web/node_modules/playwright` (1.63.0):
    `docker run --rm --network host -u $(id -u):$(id -g) -e HOME=/tmp -v <dir>:<dir> -w <dir>/web mcr.microsoft.com/playwright:v1.63.0-noble node <script>`.
    Never run `npx playwright install` on the host.
38. (2026-10-05) A Node ESM script outside `web/` (e.g. `docs/screenshots/shoot.mjs`) cannot
    `import "playwright"`: ESM resolves packages from the script's folder, not the working folder.
    Use `createRequire(join(process.cwd(), "x.js"))("playwright")` and run it from `web/`.
39. (2026-10-05) Screenshots of a dialog: `el.scrollIntoView()` also scrolls the page behind it.
    Scroll only the dialog's own scroll box (the nearest ancestor with scrollHeight > clientHeight).
41. (2026-10-05) The demo needs `/?demo` plus a click on `button.found-server`; the dev server
    takes `PORT=<n>` (default 5173, often taken by another session).
49. (2026-10-05) Playwright actions: `await page.locator(sel, { hasText: "..." }).click()`. `page.click()`
    takes no `hasText`, and `expect(...)` wraps only assertions (`toBeVisible`, `toContainText`), never
    `.click()`. To move shared steps into `test.beforeEach`, edit the existing one: a `describe` has
    one `beforeEach`, and a second copy runs the setup twice. Read the demo data (`api/mock.ts`)
    before writing expected text, such as which computer a form picks by default.
