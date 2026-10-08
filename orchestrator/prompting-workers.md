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
69. (2026-10-06) A correct draft failed its check and burned three fix rounds: the worker
    wrapped the signatures over several lines (rustfmt style), so the one-line needles never
    matched, and Claude's own wiring in the same tree (routes calling the next, unwritten
    module) broke the compile check. Match needles with whitespace and trailing commas ignored
    (`tools/qwen/chk.py` does this), and wire callers only after the module they call exists.
70. (2026-10-06) Two "insert a new function after/before <anchor>" patch jobs replaced the
    anchor instead: one SEARCH/REPLACE dropped `current()`, the other deleted `comfy_target`,
    and three fix rounds then edited a broken file. A patch check must name every existing
    public function of the edited file (not only test names, lesson 66) so a lost neighbour fails
    the check. Better, for a new function: let the job write only the function into a scratch
    file (whole-file mode) and splice it in at the anchor yourself.
72. (2026-10-07) A negative needle in a check (`! grep refused`) matched the doc line the prompt
    itself dictated ("nothing is refused"), so a correct draft failed and the fix rounds pasted
    the check's needles into a junk comment. Keep negative needles out of the text you ask for
    (search the exact old phrase, e.g. `licences are refused`), build checks with `chk.py` or
    `shlex.quote()` (lesson 67, broken again by `${...}` needles), and test a binary crate's
    module with `cargo test -q <module::path>`: `--lib` fails with "no library targets".
74. (2026-10-07) A correct draft failed its `chk.py` check three times: chk.py drops a trailing
    comma before a closing bracket (`pub add: i64,\n}` becomes `pub add: i64}`), so the needles
    `pub add: i64,` and `pub also: Vec<Also>,` never matched and the fix rounds deleted a test.
    Never end a chk.py needle with `,`; cut it at the last word (`pub add: i64`). A fill-loop
    insertion sent as a patch job was gamed in the test module (the needle `for a in &p.also`
    went into a test): a new block goes through a scratch file and a splice script (lesson 70).
75. (2026-10-07) Lesson 74 was broken twice more in the next task (`count: u32,`, `pub also:
    Vec<Also>,` as the last struct fields): a rule Claude keeps forgetting belongs in the tool.
    chk.py now drops a needle's own trailing comma. Literal code in a spec (one `let` line, one
    struct field) is still applied by Claude: sent as a job it landed at the wrong anchor and the
    fix rounds renamed `g` to `t` across `fill` to make it compile.
76. (2026-10-07) A scratch-file test job whose check was only needles came back with the helper as
    `todo!()` under `#[test]` and the TOML pasted loose between functions: every needle matched.
    A scratch file's check splices it into its place (idempotent script) and compiles and runs
    it (`cargo test -q <module>`, Playwright for a spec); the second draft passed first time.
77. (2026-10-07) A patch job got three edits in one function (queue, execute signature, upload
    block) and twice came back with the block in the wrong function and invented signatures;
    split into one edit per job, each passed first time. A `focus` entry is a regex: `studioMake(`
    crashed the pipeline mid-batch (pipeline.py now matches an invalid pattern literally).
78. (2026-10-07) A needle that is only a message string ("takes no face photo.") was met by a
    `// takes no face photo.` comment in another function. Needles carry code around the text
    (`format!("{} takes no face photo."`, `return Err(`), never the bare message. Fix rounds left
    a spliced function badly indented: rustfmt only that function (`rustfmt --edition 2024` on
    the extracted text) when the file itself is not rustfmt-clean.
79. (2026-10-07) A test check `cargo test ... | grep -E '^error|test result: ok'` passed on a
    failing test: cargo's `error: test failed` line matched. Capture the output, print the error
    lines for the fix round, and pass only on `grep -q 'test result: ok'`. The same job's prompt
    lost all its code (lesson 17 broken: an unquoted heredoc ran the backticks), and Coder then
    put the wanted SQL into a comment, which the needle accepted.
80. (2026-10-07) A scratch-file job's fix rounds could not fix a one-character error: the
    compiler named the spliced target (`gaming.rs:119`) while the job's file was the scratch, and
    the error sat between twenty old warnings from other files. Filter the check's output to the
    edited file (`grep -A5 'gaming.rs'`), and name the scratch file in the fix prompt.
81. (2026-10-07) A test of a rounding formula passed on wrong code: the asserted inputs (1, 3,
    5 seconds) were all multiples of the step, so a draft that rounded up and reassigned a
    `const` (a crash for any other input) went green. Assert at least one input where rounding
    down and up differ (2.5 s -> 61 frames). In plain CommonJS review destructuring: Coder
    dropped the `const` from `[w, h] = ...`, which silently makes globals.
82. (2026-10-07) `node --test test/` (Node 22) loads the directory as a module ("Cannot find
    module .../test") and failed every round of a correct draft. Name the files:
    `node --test test/*.test.js`, and pass only on `# fail 0`.
83. (2026-10-07) A needle written as one assignment form (`job.kompanionId = await ...`)
    rejected an equal draft that put `kompanionId` into the object literal; three fix rounds
    were spent. Needle the call itself (`kompanion.makeAudio(req.userEmail,`) and prove the
    rest with a behaviour run (a stub server), not with the shape of the code.
84. (2026-10-07) Lesson 83 again, twice in one task: a needle `'/types'` missed the template
    literal `` `${API_BASE}/types` ``, and a listener needle assumed
    `document.querySelector(..).addEventListener` while the correct draft kept the element in a
    variable. For jobs whose behaviour a later job's test covers, needle only declarations
    (`function x(`, `async function y(`) plus a parse check; the behaviour test decides.
85. (2026-10-07) Conditions and filters given in prose were rewritten: "append 'rating' only
    when `!t || t.rating`" became `if (t && t.rating) append('rating', t.rating)` (the boolean
    sent as the value), "drop tags that contain a said word" became a per-character compare,
    "not one of presetType's values" filtered on the label. Give such lines as code
    (`if (!t || t.rating) formData.append('rating', ratingSelect.value);`) and test each branch.
86. (2026-10-07) A check script prefixed its errors with `sed 's/^/error: /tmp/x/f.js: /'`: the
    slashes in the path ended the sed expression, so the check printed only a sed error and
    the fix round ran on that instead of the test failure. Prefix with `sed 's|^|error: PATH: |'`
    (or awk), and run each check once by hand on a known-bad tree before the pipeline.
87. (2026-10-07) A server module that loads its saved state only under `require.main === module`
    gives a test that requires it an empty state: the behaviour test failed on a correct draft.
    Before sending the job, check what the module does on require; export the loader and
    await it in the test (or set the state through the API).
88. (2026-10-07) `pkill -f <pattern>` inside a shell command whose own text contains the pattern
    kills that shell (exit 144) and skips the rest of the command. Stop a test server by its
    port's PID (`ss -ltnp`) or a saved `$!`, never by a pattern written in the same command.
89. (2026-10-07) A check written as `a && b | grep . && exit 1 || exit 0` passes when `a` fails:
    the `|| exit 0` catches every earlier failure. End each step with `|| exit 1` and run the
    check by hand on the untouched tree first; it must fail there for a job that adds the code.
90. (2026-10-07) A keyboard test on a UI that re-renders on the next animation frame must wait
    for the focus to land (`toBeFocused()`) after each key. Two presses in a row hit the old,
    removed element and the second key does nothing: a correct draft looks broken.
91. (2026-10-07) When a job replaces a wrapper element with a helper call (`<span class=x>${v}</span>`
    -> `${helper(v)}`), positive needles for the call pass while the old wrapper stays around it
    (nested spans). Add a negative check that the old wrapper text is gone, e.g.
    `grep -n 'x">\${helper' f && exit 1`.
92. (2026-10-07) A new CSS rule meant to restyle elements that already carry a shared class (`.task`)
    must out-rank that class (`.task.cap`, not `.cap`): equal specificity loses to whichever file
    loads later. Let the test compare a computed style (the card's background against the page's).
93. (2026-10-07) A patch job's check that runs only the new test (a name pattern) lets fix rounds
    break other functions in the same file unnoticed: chasing a syntax error, a round rewrote an
    unrelated rounding formula. Run the whole test file(s) of the edited module in every check.
94. (2026-10-07) When a CSS prompt lists rules in order, the model writes them in that order: list a
    `@media (prefers-color-scheme: light)` override AFTER the base rules it overrides, or the base rule
    (same specificity, later in the file) wins and light mode keeps the dark colour.
95. (2026-10-07) Never offer the worker an alternative ("if X is not imported, write Y") that the
    check's needle does not accept: it picked the alternative and three fix rounds chased the needle.
    Check the file's imports yourself and give one exact form.
97. (2026-10-07) Needles that are only CSS selectors (`.x {`) get gamed: asked to append rules,
    the worker wrote `.x { display: block; }` for each needle. Needle whole declarations
    (`.x { gap: 0.35rem;`), or apply a plain CSS append yourself.
98. (2026-10-07) A behaviour asked for in prose ("show the chip and the menu") came back as one or
    the other. Give the condition as code (lesson 85) AND put that behaviour in the check (a test
    that sends twice), not only the markup needles.
99. (2026-10-07) A behaviour test against a temp server catches what compiles and passes unit
    tests: a new log kind hit an old CHECK constraint (500). After adding a migration file, touch
    the file with `sqlx::migrate!()` (or main.rs) before building, or the binary keeps the old set.
100. (2026-10-07) Build your own hand edits before a job whose check compiles the program: with
    `precheck: false` a broken line of yours (a wrong variable name in another file) fails the
    worker's correct draft and its fix rounds edit the wrong file. Also check a helper's parameter
    types before naming it in a spec (busyWhile takes a form, not any element).
101. (2026-10-07) Shell scripts by the worker: `'it\'s'` inside single quotes is not an escape
    (sh -n can still pass), and steps given in prose ("an empty answer takes the default", "refuse
    a wrong mode") were dropped or guarded by the wrong `if`. Give the steps as code lines and
    check with a stub-PATH test that runs every branch (tools/test_install_host.sh).
102. (2026-10-07) A docs job checked by keyword needles gets gamed: the worker stuffed the missing
    names into one sentence and put a wrong value in a table cell. For a docs job, needle whole
    facts ("56 s (shape") and read the result; or give it the table rows to copy.
103. (2026-10-07) Lesson 100 twice in one day: a hand edit (a struct inserted in the middle of
    another struct's doc comment) broke the build and burned two drafts and six fix rounds. Run
    `cargo check` after EVERY hand edit, before the pipeline, no exceptions.
105. (2026-10-07) While a job's check builds the whole crate, the shared tree is the job's: a
    test or call Claude adds for the NEXT job breaks that check, and the fix rounds then "fix"
    the wrong file (BUG-02: twice, two jobs lost). Keep next-job tests in a scratch file and
    splice them in only when no pipeline runs on that tree.
106. (2026-10-07) A 9B model given a whole function as literal code still rewrote it and changed
    the given tests (BUG-02 audio and ComfyUI jobs, both models). Lesson 75 holds without
    exceptions: literal code is pasted by Claude; a job gets intent, signatures and a test.
107. (2026-10-07) A check that only needles the new line passes a draft that deleted the lines
    around it (BUG-02: the coder switch vanished, the new call landed outside its branch). A
    patch check needles every line the prompt says to keep and the branch the new line goes in.
108. (2026-10-07) "The body becomes one line that calls the new function" left the old body in
    place with the new line at its end, so every call ran twice; the stub test passed because it
    only checked the answer (BLD-01, Coder). When a job moves a body into a new function, the
    check counts calls on the stub (or needles the old body's absence), not just the result.
109. (2026-10-07) A check that runs a test runner from the wrong folder printed "No tests found";
    the fix rounds treated that as the job's error and added junk (BLD-01). Every check must
    fail with a spec error when the runner finds no tests, and runs once by hand first (86).
110. (2026-10-07) "Only the creation goes under the if, the rest runs every time" made the model
    drop the if block entirely (BLD-01, Coder). For a restructured block, give the whole new
    block as code lines (85) and needle the kept branch (107).
111. (2026-10-07) A 9B chat model writing free-form code for a tool (Blender bpy) failed most
    calls; the same model filled a structured tool (a list of shapes with enums) right on the
    first try (BLD-01). Tools meant for small models take structured arguments; free code stays
    as the expert tool next to it.
112. (2026-10-07) A job that leaves the build broken poisons every later job on the same tree:
    their fix rounds "fix" correct code to make someone else's error go away (CHAT-03b: binds
    dropped from a finished query, invented counters, a test module replaced, callers rewired).
    A shared-tree check prints only errors of the job's own file and otherwise says "another
    file is broken: change nothing"; stop and repair before the next job (57, 69, 80).

115. In a git worktree run tools/qwen/pipeline.py with KOMPANION_CONFIG=<main checkout>/kompanion.toml: the key is read from the .env next to the config, and a symlinked toml has none. HTTP 500 "invalid or missing api-key" is a config error, not a Coder reload.
116. A shell installer with several independent sections (resolve a tool, firewall, self-check) is one job per section, or Claude writes it. One patch job for the whole file piled up duplicate validators and unset variables over 3 fix rounds.
117. (2026-10-08) A cargo check's error filter keeps rustc's `expected`/`found`/`help` notes (`grep -A4 '^error'`), not only the `-->` location lines: with locations alone, three fix rounds on "mismatched types" did not converge (OVR-01). For a line parser, give one worked input -> output pair per rule in the prompt, and say which inputs must NOT be dropped (unknown names stay, with no id).
119. (2026-10-08) Unit tests of the pure helpers do not cover the glue: the RVW-01 draft passed them while it returned a closed HTTP response, joined the repo root one folder short and left the finding text out of the model prompt. The test file also drives the function that calls the model (a fake `pipeline` module in sys.modules) and asserts what the prompt contains.
