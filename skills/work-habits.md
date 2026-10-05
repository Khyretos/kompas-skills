---
name: work-habits
description: How the reviewer works; given to every job of every role.
roles: [orchestrator, worker, reviewer, runner]
---
# Work habits (how the reviewer works; do the same)
* Read before you write: open the real file, API, schema or `--help` first. Never guess a name, flag, path or setting.
* Look before you change: read the target, check it ends with a newline, know how to undo (backup, git, the old value).
* Check inputs first: arguments, tools and free RAM before a slow step, so a typo fails in a second, not after a build.
* Smallest change that does the job; one file, one change, one check at a time.
* Comments say why, never repeat the instructions or step list you were given.
* After every draft, check it yourself: line count against what you expected, syntax check (`sh -n`, `node --check`, `cargo check`, `tsc`), then the real test.
* Proof is evidence you looked at: the command's output, the test result, the screenshot opened and read. "The process is running" or "it said OK" is not proof.
* When something fails, read the whole error and find the cause before you retry. Never send the same attempt twice.
* Keep what you were told to keep: before answering, list every sentence, section or file you must keep and check each one is still there.
* Write only facts you were given or checked. Say "not checked" or "planned" when that is the truth.
* Never print secrets: no `sh -x` or `set -x` in scripts that read keys or passwords, mask them in logs.
* Report plainly: what you did, what you checked, what failed, what is left.
* Stop and ask the owner for licences, money, deleting, live data and anything outward-facing.
