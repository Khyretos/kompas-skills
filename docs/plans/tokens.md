**Goal:** Kees sees what working with the local models really saves: per task, the tokens Coder generated versus the tokens Claude spent on prompts, reviews and fixes.

**Machine / role:** kireserver; `tools/qwen/pipeline.py`, server (call log), web app (task detail and Activity).

**Depends on:** nothing.

**Steps**
1. In `pipeline.py`, log per job the prompt tokens sent (from the response `usage`) and the prompt's character count.
2. Create a small script `tools/qwen/claude-usage.py` that reads Claude Code's session transcript (`~/.claude/projects/<project>/<session>.jsonl`, the `usage` field of each assistant message) and sums input, cached input and output tokens per hour; Claude writes the task id into the drafting log entry so both can be matched by time and task.
3. Per task: Coder output tokens, Claude output tokens, Claude input tokens (cached and not), lines by Coder versus lines Claude changed, review rounds; written to `docs/qwen-log/<branch>.jsonl` as one summary line.
4. In the app: the task detail shows these numbers; the Activity tab shows a weekly sum per task type (code, docs, ops).
5. Never store prompts or code from the transcript, only counts.

**Done when:** for the next three tasks the summary line exists and the task detail shows it; the weekly sum appears in Activity.

**How to test:** run the script on this session's transcript and compare with the usage card; open a task detail.

**Principles:** FOSS only; local models draft, a stronger model reviews and writes lessons; Kreative Kompas palette with bright text (7:1 headers and buttons); every action updates without a reload; English and Spanish at least; one SSO account per person; security first (no open endpoints, grants for every action on a PC).

_Kees, 2026-10-05. Drafting by the local AI (Coder on kireserver), reviewed by Claude._
