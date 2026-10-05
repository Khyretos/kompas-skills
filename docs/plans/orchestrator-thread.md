**Goal:** Like Kees's coordinator thread in his Claude Max session: one orchestrator per project splits a task into steps, gives each step the skills it needs, and keeps one project thread up to date, so Kees can follow and steer everything from his phone.

**Machine / role:** kireserver; server code (Rust, `server/src/taskrun/`), web app.

**Depends on:** "Skills: smart loading within a size budget per model".

**Steps**
1. Ensure each project has one dedicated orchestrator thread (a chat marked as the project thread) created at project creation or first use.
2. Implement automatic posting of short updates in the thread by the orchestrator for events: task started (with computer name), step done, waiting for approval (with link to approval card), review findings posted, lesson added (with skill file path), and task done or failed (with report link).
3. When planning, have the orchestrator pick skill cards per step using `select()` and write them into the plan step as `skills: ["worker/web", "shared/colour-themes"]`, ensuring the worker and reviewer receive exactly those skills.
4. After a review with findings, ensure the reviewer writes each finding as a lesson into the right card or role core, adds a row to `docs/lessons-learned.md`, and the orchestrator posts this update in the thread.
5. Allow Kees to write in the thread to steer ("pause", "go on", or questions), with the orchestrator answering directly there.
6. Configure notifications (desktop, mail, phone) to include links to the specific thread message.

**Done when:** running the W2 demo task posts started, each step, the review and done messages in the project thread live without a reload, and the plan shows the skills per step.

**How to test:** Playwright test in demo mode for the thread updates; one real W2 run on soucouyant.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude. FOSS only._
