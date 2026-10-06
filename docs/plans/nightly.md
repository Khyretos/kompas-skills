**Goal:** catch regressions in the part that matters most, a real model doing a real task, not only demo data.

**Machine / role:** kireserver CI runner (nightly, after the lint-fix job), Coder on OVMS.

**Depends on:** nothing.

**Steps**

1. Create a fixed test repo `kompanion-w2-fixture` with one function and a failing test, plus a task "make the test pass" with a "done when".
2. Configure a nightly CI job to start a Kompanion server from the PR-merged main using a test config that points to the real Coder provider on OVMS, running in the job container with a grant for the fixture folder.
3. Run the task and wait up to 15 minutes; if it ends `done`, the fixture's test passes, the reviewer approves, and the run report exists, record time, model calls, tokens and fix rounds in `docs/nightly/<date>.json`.
4. On failure, post in the Kompanion project thread with the run report link and open a task "Nightly W2 failed <date>".
5. Ensure runs only between 03:00 and 05:00 and skip when a GPU job is running so it never blocks Kees.

**Done when:** three nights in a row the job runs and records its numbers; a planted bug in the fixture's expected answer makes it fail and post in the thread.

**How to test:** run the job by hand once with the real model; once with the planted bug.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC).

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
