---
name: orchestrator/prompting-workers
description: How to write a job prompt any worker model can carry out, and how to run fix rounds.
roles: [orchestrator, reviewer]
tags: [prompt, drafting, fix-round, review]
---
# Prompting a worker model

Learned from Qwen3.5-9B, qwen3:14b and gemma4 drafts (2026-10-01 to 10-05). True for any model;
smaller models just fail sooner. Evidence per lesson: `docs/model-notes/qwen3-history-2026-10.md`.

1. Put the real config, paths, hosts, API signatures and file excerpts in the prompt. A worker
   invents whatever is missing (paths, settings, endpoints, constructors).
2. A positive example of the allowed pattern beats a "never do X" rule.
3. Give tables, not adjectives: a role-to-colour table, the exact key list to fill, the variables
   or selectors to touch with their values, byte offsets for a binary format, the icon names
   that exist. "X is the main accent" makes it use X everywhere.
4. Compute numbers yourself (colour channels, contrast ratios, scales, offsets) and paste them in;
   workers copy given values well and invent computed ones.
5. Say what not to touch (gradients, headings, other functions); workers "improve" things unasked.
6. Keep prompts small: under about 3,800 tokens for the 9B on OVMS (it cut a 13 KB prompt at
   4,096 tokens). Send only the code the change needs. To prove the whole prompt arrived, end it
   with "start your answer with END-OF-PROMPT-SEEN".
7. Keep output small: under about 250 lines per draft. Split a module into 2-4 functions with
   exact signatures, one function per branch, and write the glue yourself.
8. Give the worker pure logic (decisions, parsing, markup, CSS, graph JSON). Write or tightly
   skeleton the stateful parts yourself: queues, locks, event plumbing, process pipes, socket I/O.
9. Tests: their own file, the exact public API list, and one complete example test to copy.
10. Fix rounds: send the previous draft plus numbered findings, repeat the exact API block, and
    reject a fix whose public items changed. After two failed rounds on the same finding, write
    it yourself and file the lesson.
11. Long change lists: count that every item was done (one draft did a third of them).
12. Never let a model grade its own output; it grades kindly and follows a wrong reviewer comment.
    Use a judge's critique to steer a repair, never paste its suggested fix.
13. Guard in code, not only in the prompt: filter plans, cap tool calls, hard-fail outputs much
    longer than the input or containing markup, names or context markers the input did not have.
14. Long scripts: ask for no comments or raise `max_tokens`; check the end of the file is there.
15. Never ask a model to output its own chat-template tokens (`<|im_start|>`): generation stops there.
16. Before a long batch, smoke-test about 5 real items and check every answer is non-empty.
17. Build job prompts with a quoted heredoc (`<<'EOF'`) or from a file.
18. (2026-10-05) Patch jobs with `focus`: name only lines that are inside a focus region; a prompt
    that mentions an import outside it made Coder put the `// ...` marker into its SEARCH, and the
    patch failed twice. After a fix round, count the edit blocks against the numbered findings
    (5 findings came back as 4 blocks: one was skipped silently) and check each finding in the diff.
19. (2026-10-05) A patch can delete code next to its target: adding tests at the end of a module
    removed the last existing test. Compare `cargo test` name lists (or the diff's `-` lines)
    before and after every patch job, and count edit blocks against the edits asked (2 for 3,
    1 for "function plus tests" were both silent skips). Before asking to add to a test module,
    check that one exists; to append at the end of a file, have a script do it.
20. (2026-10-05) A small planner's labels are hints, not facts: Coder put a step on
    `server/src/forge.rs` under worker/cpp-games. Let facts in the step (the files it names,
    matched against each area core's `paths` globs) override the label, in code.
21. (2026-10-05) Coder writes about 150 correct lines per job at most: a 520-line module in one
    job came back with 15 compile errors. Split a new module into parts of 3 to 5 functions, write
    its `use` lines yourself in the prompt (and state which helpers return `()` or need `.await`),
    and join the parts by script. Tests are a job of their own; give the `db()`/`state()` helpers
    in the prompt and splice them in by script when the answer leaves them out (it did three times).
22. (2026-10-05) Tests that set a process-wide env var race each other (cargo runs tests in
    parallel): hold one `tokio::sync::Mutex` per module around them, and run a new suite three
    times before trusting it.
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
