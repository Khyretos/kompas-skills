**Goal:** the task board never drifts from reality: a merged PR marks its task done, an opened PR marks it in review, by itself.

**Machine / role:** kireserver; server (Rust, new `server/src/forge.rs`), Forgejo settings.

**Depends on:** nothing.

**Steps**
1. Convention: a PR title or branch carries the task id, for example `[M6-05]` in the title or `m6-05-...` as the branch; the id matches the task's id or its title prefix.
2. Add a new endpoint `POST /api/forge/webhook` in `server/src/forge.rs` that only accepts Forgejo's signed requests (HMAC SHA-256 with a secret from `.env`, `FORGE_WEBHOOK_SECRET`) and refuses everything else; no other open endpoint is added.
3. Implement logic for `pull_request` events: if opened, set task to `in_review` with the PR link; if merged, set to `done`; if closed without merge, set back to `queued`. Each change emits a task event over the live stream so open pages update without a reload.
4. Create setup notes in `docs/forge-webhook.md` documenting the Forgejo repo webhook URL, content type JSON, the secret, and events "Pull request".
5. Write unit tests with recorded Forgejo payloads for opened, merged, and closed states, plus a test for a wrong signature.

**Done when:** merging a PR titled `[TEST-1] ...` marks task TEST-1 done in the open app within seconds, and a request with a wrong signature gets 401.

**How to test:** the unit tests; one real test PR on Forgejo.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC).

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
