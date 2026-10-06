# Qwen3 model notes up to 2026-10-05 (history)

The full notes as they were before the general lessons moved into role skills and cards
(`skills/orchestrator/prompting-workers.md`, `skills/shared/colour-themes.md`, `skills/worker/*`, `work-habits.md`).
Kept as evidence; not loaded into prompts. Paths starting with `~/Docker/docs/ai-capability/` are task notes on kireserver.

## qwen3:14b (Ollama on soucouyant)

Measured 2026-10-01..03 (details: ~/Docker/docs/ai-capability/qwen3-14b-soucouyant-2026-10.md).

- Turn thinking off with `reasoning_effort: "none"`; Ollama ignores `chat_template_kwargs`.
- Good: UI code in an existing style when given an example file; restructuring existing text faithfully.
- Weak: systems/stateful code (rates, sysfs), following negative rules ("never use X") — give a positive example of the allowed pattern instead.
- Invents paths, hosts and settings when the prompt doesn't contain them; put the real config in the prompt.
- Sometimes copies an instruction from the prompt into its output; strip lines like "ensure no additional ...".

- (2026-10-03) Benchmarked against gemma4:12b, qwen3.5:9b, gpt-oss:20b and qwen3.6:35b-a3b on soucouyant
  (~/Docker/docs/ai-capability/soucouyant-model-benchmark-2026-10.md): 60 tok/s, 14.0 GB card total at 16k;
  at 32k it no longer fits (1.5 GB on the CPU). No vision. Its Python duration parser kept the space in the
  unit (" ns"); it broke "≤40 words" (44) and translated "een hard hoofd in hebben" as "I'm determined".
  gemma4:12b-it-qat matched or beat it on every test.
- (2026-10-03) Use one model tag per Ollama host. A context-size variant (`qwen3:14b-16k`) is a separate model: requests alternating between it and `qwen3:14b` make Ollama unload and reload on almost every switch (about 44 s per request instead of under 1 s). Pick the 16k tag everywhere if any caller needs 16k context.

As translation judge (kk-localize, 2026-10-03):

- Stricter than Qwen3.5-9B grading its own work: it scored 3 on real errors the 9B had given 4-5 (nl non-word "Gededegeerde", typo "joing", "alles else", "Open-source kredieten" for credits).
- Accepts normal tech loanwords (nl "demo games", "fork"), which is right.
- Its suggested fixes can be wrong ("wachtwijzer", "gedediceerde"): use its critique to steer a repair by another model, never paste its suggestion in.
- Misses the wrong sense of one-word labels (nl "Fundering" for Foundation, "Over" for About): those still need overrides or a human check.
- Scoring speed: about 2.5 min for 420 short strings, one request at a time.
- Gave 5 to a wrong sense even with a back-translation beside it (nl "Services healthy" → "Diensten voor een gezonde levensstijl", healthy lifestyle). A judge doesn't replace reading the short labels.

## qwen3.5:9b-q8_0 (Ollama on soucouyant; coding and PC-control model, 2026-10-03)

- Tag rebuilt with `PARAMETER num_ctx 16384` and `PARAMETER num_thread 1` (same name): fully on the GPU,
  extra CPU threads only spin-wait between GPU steps (shared lesson 6). Thinking is on by default: send
  `reasoning_effort: "none"` (OpenAI endpoint) or `think: false` (native API).
- Measured: 59 tok/s, ~4,000 tok/s prompt, 12.8 GB card total at 16k (about 10.5 GB for the model).
- It and gemma4:12b-it-qat don't both fit on the 16 GB card next to the desktop; callers alternating between
  them make Ollama swap (a few seconds per switch). Batch per model where possible.

## Qwen3.5-9B int4 (OVMS on kireserver, A770; served as "Coder")

- (2026-10-03) Same model as `qwen3.5:9b` on Ollama: there it again translated "Our fork" as "Onze vork",
  "Services healthy" as "Diensten gezond" and "kat uit de boom kijken" literally, even with context per string.
  Fast (84 tok/s on the RX 9070 XT) and good at vision and tool calls; don't use it for Dutch without a judge.

- (2026-10-03) Long bash/ImageMagick scripts: wordy comments eat the 2000-token budget and the script gets cut off midway; ask for "script only, no comments" or raise `max_tokens`. Details: ~/Docker/docs/ai-capability/peertube-images-2026-10.md.
- (2026-10-04, docs-mcp) It cannot write its own chat-template tokens: asked to copy a file containing
  `<|im_start|>` (the Reranker prompt), generation stopped right there (542 tokens, file cut off). Keep such
  strings out of what it must output; say "leave RR_PREFIX as a placeholder" and paste them in yourself.
- (2026-10-04, docs-mcp) A 300-line ingest script in one prompt came back unfinished: half the file was
  "I will assume..." comments and `pass`. Ask for 2-4 small functions with exact signatures and write the glue.
- (2026-10-04, docs-mcp) Repeated the same bugs after a review listing them: a `while` loop with `break` that
  drops text, code-block lines thrown away instead of kept, `Connection.fetchall()` (psycopg 3 needs
  `c.execute(...).fetchall()`), and SQL built with f-strings around a filter. Check those four by hand.
- About 30 tok/s generation for one request; a running job slows Open WebUI chat from about 1.7 s to 3.2 s per answer, so batch work runs at night with concurrency 1.
- Turn thinking off with `chat_template_kwargs: { enable_thinking: false }`.
- (2026-10-04, kk-localize) Since the GPU studio switch-back, the A770's OVMS model is served as "Coder" again.
  Every OpenAI-style call must send `"chat_template_kwargs": {"enable_thinking": false}`, the one-token health
  ping included; without it Qwen3.5 spends `max_tokens` on thinking and returns an empty answer. Before a long
  batch, smoke-test about 5 real strings and check the text is non-empty (it took 0.3-1.3 s each for hu, el, ar
  UI strings). The model name lives in the caller's config (kk-localize `config.yaml` › `llm.model`), not in `.env`.
- Good at polishing LibreTranslate drafts in nl, de, es, fr, pt (grammar, idiom, punctuation such as Japanese 。).
- Grades its own work too kindly and follows a wrong reviewer comment (repaired "Our fork" to "Onze vork", a kitchen fork, then scored it 5). Never let it judge its own output.
- Adds things to very short strings ("Powered by" → "Powered by Kreative Kompas") and sometimes wraps labels in `**…**`; check for names and markup not in the source.
- Weak in Irish: its review made correct LibreTranslate drafts worse (crúcaí → "húic agus tiománaí").
- Leaves one-word labels in English in Japanese ("About") and picks the wrong sense without context.
- (2026-10-03) A positive example of the exact query style in the prompt worked: the project-context function came back with runtime queries and no macros. Remaining slips: `query_as` without a type annotation, and tuple rows accessed as `row.field`. Ask for "annotate `let rows: Vec<(String, String)> = ...`" explicitly.
- Writes notes about the translation into the output instead of only the translation (cs "Poznámka: Pro daný kontext…"), and turns junk input (a fragment of template code) into a whole invented paragraph. Reject outputs far longer than the input.

## Colour themes / palettes (2026-10-03, VS Code theme)

Full write-up: kreative-kompas-vscode-theme `docs/ai-capability/vscode-theme-drafting.md`.

- With `think: false` and "output only JSON" it returns valid JSON first try (~50 s).
- "X is the main accent" makes it use X for almost everything; it ignored the secondary accents.
  Give a role-to-colour table instead.
- Cannot judge contrast: put violet text on plum (2:1). Give it pre-checked text colours; run the contrast script on the result.
- Made selection and current line the same colour; ask for each state as a separately named colour.
- Invents/uses deprecated theme keys (`tab.unselectedOddBackground`, `scrollbar.background`, ...) and
  mis-maps terminal ANSI and diff colours. Give it the exact key list to fill.
- (2026-10-04, qwen3.5:9b-q8_0, Kate themes) VS Code theme to Kate `.theme` JSON: valid JSON with every key, dark syntax
  colours exactly per the role table (~3.5 min per variant, partly on CPU). Wrong: filled background roles with
  foreground colours (solid orange SearchHighlight/BracketMatching hid orange keywords; solid yellow ReplaceHighlight),
  `selected-text-color` white everywhere, gutter at 1.5:1, template placeholders as text colours. The light variant
  copied the dark table (yellow, pink-red, cyan, lilac text on mist under 2:1; a dark plum current line) despite the
  ink rules in the prompt. Tell it which roles are backgrounds and give pre-blended tints; give the light variant
  its own value table and don't show it the dark one. Full note: kreative-kompas-vscode-theme
  `docs/ai-capability/kate-theme-drafting.md`.
- (2026-10-03) DOM code (code blocks): typechecked first try and avoided innerHTML, but skipped the no-language case with `continue`, forgot button labels, and moved a node before replacing it (`appendChild(pre)` then `replaceChild(..., pre)` throws). Ask it to "insert the wrapper before the node, then move the node in".
- (2026-10-03) fdinfo reader (Rust): wrong aggregation key, no dedupe, could not parse "123 ns" or a PCI slot with colons, dropped the requested tests. Rewritten. The GPU panel view (TS) was good: only cosmetic fixes.

## Config files from a role table (2026-10-03, DMS theme + kitty colours)

- Given an example file plus an explicit role-to-colour table, it transcribes accurately (dark variant: no errors).
- Keys the table doesn't cover get copied from the example file (light-variant containers came from dankViolet).
  List every key with a value, or review those keys specifically.
- It doesn't think about what a colour is *for*: `cursor_text_color` = cursor colour (text under the cursor
  becomes invisible), lilac `#cca9ff` as secondary on a light background (1.7:1). Contrast-check pairs after it.

## CSS theme edits against a brand rule (2026-10-03, orange re-theme)

29 runs over 14 apps; 3 first drafts usable, the rest needed one or two review rounds. Report: `~/Docker/docs/ai-capability/theming-orange-2026-10.md`.

- Give it the exact variables or selectors to touch and their values. With only the brand rule it invents generic selectors (`.sidebar`, `.card`, `button.secondary`) and a bare `a` that recolours all UI text.
- Spell out per mode which colour goes where: it puts orange `#f3941f` and orange-light on light backgrounds (focus rings, checkbox fills, hover) and uses the light tint `#fde7cc` on dark themes. Check every pair it writes against the ratio table.
- Compute colour scales, rgb channels and ratios yourself and paste them in: it copies 46 given values flawlessly but invents channels (`255,150,50` for `#f3941f`) and ratios in comments.
- Tell it what not to touch (gradients, headings); it "improves" them unasked, e.g. a violet headline on a dark theme.
- When a list of changes is long, check it did all of them; the first Discourse draft did about a third.
- Feed the previous draft plus numbered findings into the next prompt; that fixed most files in one round.
- (2026-10-03) Fix rounds on a large file (300 lines) at 8k context can return a different, smaller file with a new API (grants.rs lost `allows`/`add`/`revoke`). Always repeat the exact API block in a fix prompt and reject a fix whose public items differ. After two failed rounds Claude fixed it (see lesson worker/rust #11).
- (2026-10-03) Tests: invents constructors (`Grants::new`) and methods (`is_empty`) on types it was told about; give tests their own file and the exact API list.
- (2026-10-03) Test files: three fix rounds for the runner tests and each round broke something new (wrong constructors, wrong field types, then missing temp dirs, then compile errors again). Claude wrote them. For tests, give it one complete example test to copy.

- Qwen3.5-9B (OVMS Coder), 2026-10-03: it failed the proc.rs fix twice. It boxed an
  `Option` as `Box<dyn Read>`, threw away the result of `take`, dropped the `cwd`
  parameter, and kept a misplaced line after being told to move it. For process and
  pipe plumbing, give it the exact code skeleton in the prompt, or write that part yourself.
- (2026-10-04, qwen3.5:9b-q8_0) Theme rewritten from a working example theme (Trilium, Catppuccin structure) plus a full variable-to-value list: 263 lines, one wrong value (kept the example's variable on one rule instead of the given one). Rebuilding on top of the owner's old CSS (Owncast) kept the layout intact but dropped "doubled :root:root". Giving it a known-good example file beats describing the app.

## Qwen3.5-9B "Coder" (OVMS on kireserver's A770), Assets previews 2026-10-04

Three drafts for milestone 2 (demo mock 230 lines, preview CSS 120 lines, 4 Playwright tests).
Details: ~/Docker/docs/ai-capability/assets-previews-2026-10.md.

- Good: Playwright tests from a precise list of facts and selectors (no fixes needed); CSS from a
  numbered rule list; keeping existing code when told "extend, don't rewrite".
- Weak: object identity. Told twice not to mutate items in place (and why), it still did; the
  second round also dropped a requirement it had met in the first. Do such fixes yourself after
  one failed round, and say "replace the object; show the `.map(...)` line" in the prompt.
- Types a union field as `string` in helper return types; ask for `Pick<Type, "a" | "b">`.
- CSS slips that look fine in review: a conic-gradient checkerboard without hard stops (a blur),
  `inset 0 0 0 0` box-shadows (invisible), an icon button without `fill` (black icon on violet).
- Put `start your answer with END-OF-PROMPT-SEEN` as the prompt's last line: it proves the
  whole prompt arrived. One 13 KB prompt reported exactly 4,096 prompt tokens; keep prompts
  under ~3,800 tokens and send only the code the change needs.
- (2026-10-04, W2 tasks) As planner it adds "open/locate" and "run the tests" steps even when the
  prompt forbids them, and as worker it keeps re-checking finished work until the tool-call cap.
  Guard in code, not only in the prompt: filter the plan, and let the check decide after a cap.
  In patch mode (search/replace) it is reliable for small, exactly specified changes; asked for
  "a function plus tests" it skipped the tests and dropped one input shape (array of objects).
- (2026-10-04, web views) In views with nested ternaries inside `html` templates it leaves a template
  unclosed, joins `html` arrays with `.join("")` or `.concat`, writes `${String(x)}` as a bare
  attribute instead of `${x ? "selected" : ""}`, and uses icon names that don't exist. Ask for one
  small function per branch (repoForm, gameAssets) and list the available icon names in the prompt.
- (2026-10-04, binary formats) Writing a WAV parser and its own test fixture, it put every header
  field two bytes early in both, so its tests passed and real audio would have broken. For byte
  layouts, give the offsets in the prompt and check them against a real file in review.

## Coder (Qwen3.5 9B int8, OVMS on the A770), 2026-10-05

- Mermaid: mixes edge syntaxes (`-.text-.`) and leaves labels with `/` or `()` unquoted; repeats
  the same broken line when asked to fix it without the line named. Normalise and render first.
- Vision works through OVMS (image_url data URL): it judged a rendered diagram against the PR
  walkthrough correctly. Use it as a check, not as the only gate.
- Plain JS asked for, TypeScript written: a `.mjs` draft came back with type annotations twice.
  Say "plain JavaScript, no type annotations" and run `node --check` on every draft.
- (2026-10-05, M6-03) Rust with std only (a unix-socket helper): two drafts that did not compile
  (answer printed to stdout instead of the socket, `Instant < Duration` comparisons, panics on
  every error). Pure decision logic (role_policy.rs) came out right on the first try, only its
  tests borrowed temporaries. Give Coder the pure part; write small I/O glue yourself.
- (2026-10-05, README refresh) Docs from a long spec: Coder followed a 6-part structure, 15
  image/caption pairs and three "keep word for word" sections exactly, but dropped one sentence
  that was asked for in passing inside a longer item ("Keep the banner sentence that follows
  now"). Put every must-keep sentence in its own numbered item and quote it in full, and diff
  the kept sections against the old file. A follow-up patch job fixed it on the first try.
- (2026-10-05) Markdown answers come back without a wrapping fence. `pipeline.py` used to take
  the first fence inside the README (a ```sh block) as the start of the answer and cut 70 lines;
  `strip()` now keeps an unwrapped `.md` answer whole. Check the line count of every docs draft.
- (2026-10-05, install script) Given numbered steps, Coder copies them as numbered comments
  ("# 1. cd to the script's own folder"). Say "no comments that repeat the step list; at most two
  short comments where a step is not obvious".
- (2026-10-05, install script) `case "${1:-}" in --x) ;; *) usage; exit 2;; esac` rejects the
  no-argument run, and the option was checked after a one-minute build. Both fixed in one
  follow-up draft once the review named them; now in ai-skills `shell`.
