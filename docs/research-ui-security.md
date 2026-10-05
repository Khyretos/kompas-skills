# Research: Open WebUI, OpenCode, Goose, framework patterns, security (2026-10-01)

Summary of a source-level study (shallow clones of open-webui, sst/opencode, block/goose; advisories; RFCs). Marked [inferred] where not verified in code.

## Adopted in Kompanion

- **Messages made of typed parts** with a state machine per tool part (pending → running → completed | error), streamed as part updates/deltas over SSE with heartbeat (OpenCode `packages/schema/src/v1/session.ts`, `GET /event`).
- **Coalesce events, flush once per animation frame** (OpenCode client `server-sdk.tsx`): at 30 tokens/s, at most one DOM write per frame.
- **Keyed lists**: reuse DOM nodes by id, only re-render changed items (React/Vue/Solid keyed reconciliation, done by hand).
- **Permissions**: rule action allow | deny | ask; reply once | always | reject; "always" adds a pattern (OpenCode). Session modes Auto / Approve / Smart / Chat and per-tool Always / Ask / Never (Goose). Approval cards show exact command, folder, machine, diff, network targets.
- **Markdown**: `marked` + `DOMPurify` (fragment output), never raw innerHTML of model text. Model and provider output is never executed.
- **Auth**: BFF pattern (RFC 10017 / BCP 212): server is the OIDC client with PKCE, browser gets an HttpOnly Secure SameSite=Strict cookie + CSRF protection; native/Tauri apps use RFC 8252 (system browser + PKCE). WebSocket: check Origin, short-lived ticket, never long-lived tokens in URLs.
- **Strict CSP** with Trusted Types: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; require-trusted-types-for 'script'; trusted-types dompurify app`.
- **Model base URLs are SSRF input**: admin-only, block private ranges unless explicitly allowed (local models are an explicit allow).

## Mistakes others made (avoid)

- Open WebUI CVE-2025-46719 (CVSS 9.6): stored XSS via markdown token, JWT in localStorage → account takeover → admin RCE. More stored XSS: CVE-2025-64495, CVE-2025-65959, GHSA-gf5m-wcrh-7928.
- Open WebUI CVE-2025-64496: a malicious model server's SSE `execute` event ran through `new Function()`.
- OpenCode CVE-2026-22812: unauthenticated local HTTP server exposed a shell endpoint to any web page.
- OpenCode CVE-2026-22813: unsanitised LLM markdown + no CSP → XSS → shell commands. In an agent UI, XSS = remote code execution.
- Goose desktop CSP allows `'unsafe-inline'` scripts and any frame source; don't copy.

## OWASP LLM Top 10 (2025) that apply

LLM01 prompt injection, LLM02 sensitive data disclosure, LLM05 improper output handling, LLM06 excessive agency, LLM10 unbounded consumption (token/task budgets).

## Small dependencies chosen

| Package | Size (min+gz) | Licence | Use |
|---|---|---|---|
| dompurify | 11.6 KB | MPL-2.0 / Apache-2.0 | sanitising rendered markdown |
| marked | 13.5 KB | MIT | markdown to HTML |
Considered: alien-signals / @preact/signals-core (1.7 KB, MIT) if fine-grained reactivity is needed later; lit-html (3.2 KB) only if templating gets painful. TC39 Signals is still Stage 1.

## UI ideas to copy

OpenCode: collapsible step groups with token/cost totals, reasoning shown apart from the answer, revert via snapshots, share links. Goose: always-visible mode toggle by the composer, per-tool permission drop-downs per extension, tool calls with arguments + status + expandable result, recipe consent screen, security alerts with confidence. Open WebUI: per-provider connection page with a "test connection" button; keep account merge-by-email off by default.

Sources: GHSA-cm35-v4vp-5xvx, GHSA-w7xj-8fx7-wfch, GHSA-8wvc-869r-xfqf, GHSA-vxw4-wv6m-9hhh, lilting.ch OpenCode CVE write-up, github.com/tc39/proposal-signals, rfc-editor.org/rfc/rfc10017.
