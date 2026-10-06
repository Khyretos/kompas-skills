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
39. (2026-10-05) Screenshots of a dialog: `el.scrollIntoView()` also scrolls the page behind it.
    Scroll only the dialog's own scroll box (the nearest ancestor with scrollHeight > clientHeight).
49. (2026-10-05) Playwright actions: `await page.locator(sel, { hasText: "..." }).click()`. `page.click()`
    takes no `hasText`, and `expect(...)` wraps only assertions (`toBeVisible`, `toContainText`), never
    `.click()`. To move shared steps into `test.beforeEach`, edit the existing one: a `describe` has
    one `beforeEach`, and a second copy runs the setup twice. Read the demo data (`api/mock.ts`)
    before writing expected text, such as which computer a form picks by default.
50. (2026-10-06) Run code in the page on an element with `locator.evaluate((el) => ...)`; a locator
    passed into `page.evaluate` cannot be serialized. WCAG luminance: `v <= 0.03928 ? v / 12.92 :
    ((v + 0.055) / 1.055) ** 2.4` (divide, never multiply). Keep every assertion the spec lists
    (focus after a choice, both colour schemes); never drop one to make a run pass.
