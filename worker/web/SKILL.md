---
name: worker/web
description: Writing the vanilla TypeScript web app and its Playwright tests.
roles: [worker, reviewer]
tags: [typescript, web, css, html, playwright]
paths: ["web/**"]
---
# Worker: web app (vanilla TypeScript)

Topic lessons moved into cards (live-updates, playwright, css-layout, controls), loaded when a job needs them. Add new lessons to the card they belong to.

47. (2026-10-05) "Plain JavaScript" means no type annotations; run `node --check` on every `.mjs`.
48. (2026-10-06) Helpers that use a function's parameters or its elements (api, callbacks, the
    sheet's nodes) are closures inside that function, never module-level functions: a draft moved
    them out and every name was undefined. Await an API call before reading its result.
58. (2026-10-06) A patch replaces its whole SEARCH text: every line in SEARCH that you keep must be
    in REPLACE too. A draft searched the import lines to add one name and dropped another import;
    three fix rounds didn't restore it. Search only the line you change.
