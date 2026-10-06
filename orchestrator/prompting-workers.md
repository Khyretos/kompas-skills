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
