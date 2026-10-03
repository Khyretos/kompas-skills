# Qwen3 family, notes for prompts

## qwen3:14b (Ollama on soucouyant)

Measured 2026-10-01..03 (details: ~/Docker/docs/ai-capability/qwen3-14b-soucouyant-2026-10.md).

- Turn thinking off with `reasoning_effort: "none"`; Ollama ignores `chat_template_kwargs`.
- Good: UI code in an existing style when given an example file; restructuring existing text faithfully.
- Weak: systems/stateful code (rates, sysfs), following negative rules ("never use X") — give a positive example of the allowed pattern instead.
- Invents paths, hosts and settings when the prompt doesn't contain them; put the real config in the prompt.
- Sometimes copies an instruction from the prompt into its output; strip lines like "ensure no additional ...".

As translation judge (kk-localize, 2026-10-03):
- Stricter than Qwen3.5-9B grading its own work: it scored 3 on real errors the 9B had given 4-5 (nl non-word "Gededegeerde", typo "joing", "alles else", "Open-source kredieten" for credits).
- Accepts normal tech loanwords (nl "demo games", "fork"), which is right.
- Its suggested fixes can be wrong ("wachtwijzer", "gedediceerde"): use its critique to steer a repair by another model, never paste its suggestion in.
- Misses the wrong sense of one-word labels (nl "Fundering" for Foundation, "Over" for About): those still need overrides or a human check.
- Scoring speed: about 2.5 min for 420 short strings, one request at a time.
- Gave 5 to a wrong sense even with a back-translation beside it (nl "Services healthy" → "Diensten voor een gezonde levensstijl", healthy lifestyle). A judge doesn't replace reading the short labels.

## Qwen3.5-9B int4 (OVMS on kireserver, A770; served as "Coder")

- (2026-10-03) Long bash/ImageMagick scripts: wordy comments eat the 2000-token budget and the script gets cut off midway; ask for "script only, no comments" or raise `max_tokens`. Details: ~/Docker/docs/ai-capability/peertube-images-2026-10.md.
- About 30 tok/s generation for one request; a running job slows Open WebUI chat from about 1.7 s to 3.2 s per answer, so batch work runs at night with concurrency 1.
- Turn thinking off with `chat_template_kwargs: { enable_thinking: false }`.
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
