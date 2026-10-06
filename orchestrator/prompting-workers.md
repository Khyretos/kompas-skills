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
19. (2026-10-05) A patch can delete code next to its target: adding tests at the end of a module
    removed the last existing test. Compare `cargo test` name lists (or the diff's `-` lines)
    before and after every patch job, and count edit blocks against the edits asked (2 for 3,
    1 for "function plus tests" were both silent skips). Before asking to add to a test module,
    check that one exists; to append at the end of a file, have a script do it.
22. (2026-10-05) Tests that set a process-wide env var race each other (cargo runs tests in
    parallel): hold one `tokio::sync::Mutex` per module around them, and run a new suite three
    times before trusting it.
26. (2026-10-06) Pseudo-code comments in a spec come back as the same comments over stub code
    (`pass`, "approximate"). Give the algorithm as numbered prose steps and ask for one small
    function per job.
27. (2026-10-06) Small models miscount items: asked to tag 9 listed blocks, the answer had 8 or 23
    lines, and fix rounds did not help. For one answer per item, give a numbered template to fill in
    (`1 \n2 \n...`) and each item's text, not only its first line.
28. (2026-10-06) A model's tags (public or private, keep or drop) are a draft: it tagged whole runs the
    same and called a brand card general. Review every block that moves to a public place with a
    keyword scan plus the privacy check.
29. (2026-10-06) A patch job asking for a rename and a new paragraph in one file did only the paragraph.
    Give each edit its own patch job with a focus on the lines it changes, and a check per edit. Appending
    a lesson to a card failed twice with no edit blocks at all: plain appends are applied by the orchestrator.
30. (2026-10-06) A drafted deploy script piped a file into `docker exec -i ... python3 -c` but left out the
    `< file` redirect; the check only grepped for strings and passed, and the first real run wrote an empty
    plugin into the live database. A script with side effects needs a check that runs it against a stub (or
    asserts every step the spec names, the redirect included), and its first real run is verified at once.
    Progress output written with `\r` (git subtree push, curl) is one long line for `tail`: add `tr '\r' '\n'`.
31. (2026-10-06) A check made of `grep -q` prints nothing when it fails, so the pipeline had no errors to
    send back and ran no fix rounds; three wrong drafts passed as done. A content check prints a line the
    fix round can use: `grep -qF 'x' f || { echo 'error: f: what is missing'; exit 1; }`.
32. (2026-10-06) When one job changes a function's parameters and another job edits its callers, the
    second draft guessed the order (twice). Give every caller job the full new parameter list, and check
    the call's exact argument order: type checks miss swapped optional parameters of similar types.
33. (2026-10-06) An unbalanced bracket stops the compiler before any type error, so three fix rounds
    chased one parse error and never saw the rest. When fix rounds repeat the same error, fix that line
    by hand and run one more fix job with the full error list. Test stubs on a framework API the model
    guesses at (axum extractors) are given as literal code and applied by Claude.
34. (2026-10-06) A check for a renamed function must pass once the callers are updated too: rename
    the callers in the same job, or in the check's precondition, or the job can never pass. Read
    every test a fix round added: one asserted an expectation the spec never gave.
35. (2026-10-06) A grep-only check gets gamed: the draft put `menuitemradio` in a class list and
    changed the function's signature, and the check passed. Check the exact signature and every
    attribute the spec names, and prefer a behaviour test (Playwright) as the check. In patch mode
    the model also builds SEARCH text from the prompt's new code when it resembles a real line
    (`if (firstRender || changed(...` that was not in the file): quote the anchor lines from the
    file and say "insert after these lines" instead of describing a block.
57. (2026-10-06) A batch whose checks build the whole program (`cargo check`, `tsc`) shares one
    tree: when one job's check fails, the tree stays broken and every later job's check fails on
    that error, so their fix rounds rewrite code that wasn't wrong (a deleted test, invented
    signatures). Run a job whose code the next jobs depend on in its own pipeline call, check that
    it passes, then queue the rest.
59. (2026-10-06) Give a test job the exact asserts (inputs and expected value), not a prose list
    of cases: from prose the draft wrote an assert that contradicted the rule it tested and built
    cards whose names never matched, and the fix rounds then chased the wrong side. Name the
    module of every symbol to import; a "fix" otherwise invents aliases from the wrong file.
60. (2026-10-06) When a job adds a branch next to existing code, say where it goes ("before the
    line `if (!x) return;`") and which existing lines stay as they are. Asked for "two cases" in
    prose, the draft merged them into one path that dropped the value the user had just chosen.
62. (2026-10-06) A "loop until X, then use it" step in prose comes back as a `while` loop that
    tests the first value and never rebinds it, so the code after the loop uses the wrong item
    (the check still passed, by luck of event order). Give the shape: `let e = loop { let x =
    next(); if ok(&x) { break x; } };`.
63. (2026-10-06) A new-file check written as `test ! -e f || ...` also passes when the job wrote
    nothing: an answer cut off at the token limit left no file and the job counted as passed.
    After the precheck, a new-file job needs a check that requires the file ("precheck": false
    and `test -e f || { echo "error: f: not written"; exit 1; }`). When a cut-off answer goes
    into fix rounds, the fix writes a stub with just the grepped names; for a whole file, check
    every piece of the spec (a list of signatures and literals) and ask for a compact file.
64. (2026-10-06) A card rule is not enough for a test stub server: with lesson 49 in its skills,
    Coder still wrote axum 0.7 (`"/history/:id"`, `std::net::TcpListener`, an invented
    `map_into_response`) and three fix rounds did not get it to compile. Put the stub's handler
    signatures, routes and serve lines in the prompt as code (Claude applies literal code), or
    write the stub yourself and let the job write only the asserts.
66. (2026-10-06) A patch job's check that greps the new signature and counts passing tests
    passed while the worker had put the new public function inside `mod tests` and deleted an
    existing test to make room. For a patch to a file with tests, check that every old test name
    is still there (`grep -c 'fn test_a\|fn test_b'` equals the old count), and that the function
    is outside the tests module (a non-test `cargo check` with a caller in place, or
    `awk '/mod tests/{exit} /fn name/{f=1} END{exit !f}'`).
67. (2026-10-06) A check built by pasting needles in single quotes broke on `&'static`: the
    shell stopped with `Syntax error: "(" unexpected`, the job had `"precheck": false`, so the
    pipeline logged a failed check with no compiler errors and ran no fix round; the draft had 10
    compile errors. Quote every needle with `shlex.quote()` when building checks in Python, and
    run `sh -n -c "<check>"` on each check before starting the pipeline.
68. (2026-10-06) A fix round turned `docker stop <container>` into `docker stop` (the args vec
    lost the container) and still passed: the tests only covered refusals, never the command
    line. For code that runs a program, put a fake program first on PATH in the test (a script
    that writes its argv to a file) and assert the exact argv, or route the command through a
    pure `fn args(...) -> Vec<String>` that a test checks.
