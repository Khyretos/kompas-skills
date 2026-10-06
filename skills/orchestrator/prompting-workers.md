---
extends: orchestrator/prompting-workers
---
18. (2026-10-05) Patch jobs with `focus`: name only lines that are inside a focus region; a prompt
    that mentions an import outside it made Coder put the `// ...` marker into its SEARCH, and the
    patch failed twice. After a fix round, count the edit blocks against the numbered findings
    (5 findings came back as 4 blocks: one was skipped silently) and check each finding in the diff.
20. (2026-10-05) A small planner's labels are hints, not facts: Coder put a step on
    `server/src/forge.rs` under worker/cpp-games. Let facts in the step (the files it names,
    matched against each area core's `paths` globs) override the label, in code.
23. (2026-10-05) A patch job sees only its `focus` excerpts: asking it to add a test to a module
    that is not in the excerpt makes it invent the module's lines and fail twice. Put the test
    module's last lines in `focus`, or insert fixed code by script. Never write a correction into
    a prompt ("... NO: add it after X instead"): rewrite the instruction, or the model follows
    both halves (a stray `}` was left behind).
24. (2026-10-06) A test job whose check runs that same test lets the fix rounds change the
    expectations until they match a bug in the code (a "`~/Docker` gives nothing" test hid a
    wrong home-path regex). Give the code job its own check with the spec's key cases as
    asserts (`python3 -c "... assert ..."`), and read every changed expectation in the review.
25. (2026-10-06) Jobs that depend on each other: when a patch fails, the pipeline still runs the
    check on the untouched tree (it passes) and the next job invents the missing code. Check the
    first job's log line for "error" before the next job, or give the next job a check that greps
    for the new function. Test fixtures for layered folders keep each layer in its own temp dir
    (a local folder inside root is read twice).
