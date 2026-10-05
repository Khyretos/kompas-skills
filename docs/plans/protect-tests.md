**Goal:** A task run can never end "done" because the worker edited the tests or the check instead of the code. The nightly test found it on 2026-10-05: given an impossible test, the worker rewrote the expected value in the test and the reviewer approved it.

**Machine / role:** kireserver; server (`server/src/taskrun/`, `server/src/pcagent/`), skills.

**Depends on:** TEN-04 (the nightly test that found it).

**Steps**
1. When a run starts, record which files the check command depends on: by default every file matching `test_*.py`, `*_test.go`, `*.spec.ts`, `tests/**` and the check command itself; a task can list more in a "protected" field.
2. Every write step the worker makes on a protected file is refused unless the task description says tests may change (a checkbox "this task may change tests", off by default), with the reason shown in the step card.
3. Before the reviewer step, the server lists the changed files; a changed protected file is a review finding the reviewer must answer, and the run cannot end done while it stands.
4. The run report shows the protected files and whether they changed.
5. Tests: a run whose worker tries to edit `test_names.py` is refused at that step; the nightly planted-bug fixture ends "needs you", not "done".

**Done when:** the nightly planted-bug run ends needs you with the reason "the task may not change tests", and normal runs still pass.

**How to test:** `KEEP=1 FIXTURE=<planted copy> sh tools/nightly/w2.sh --now` and the normal nightly.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC); simple for regular users, extras optional for technical users.

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
