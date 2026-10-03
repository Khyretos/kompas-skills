# Worker: localization (website translation)

These lessons come from the kk-localize pipeline (LibreTranslate draft, LLM review, independent judge). Model-specific notes: `_model-notes/qwen3/`.

1. (2026-09-29) Protect code, markup, URLs and template actions as tokens like `XQ0X`: they survive LibreTranslate and the LLM, while `{0}`, `[[0]]` and `⟦0⟧` get mangled. Check that every token comes back exactly once.
2. (2026-09-29) One-word UI labels need context or a pinned translation: without it "About" becomes a bare preposition (nl "Over", de "Über"), "Foundation" a legal foundation (nl "Stichting", de "Stiftung"), "Build" a building (de "Gebäude"), "Creator" de "Schöpfer". Pass the linked page's description or a per-file note, and pin menu labels and page titles in `overrides/<lang>.yaml`.
3. (2026-09-29) Keep a heading or sentence in one string. Never split it around markup: mark the styled part with `**…**` and pass links in as parameters (`{{ .p0 }}`). Inline HTML inside a string makes models leave the wrapped word untranslated or drop the link text.
4. (2026-09-29) The CV is written in the first person singular ("I"); say so in a per-file note, or models switch to "he".
5. (2026-09-29) Never add what the source doesn't have: a name, a brand or markdown markers (ja "Powered by" → "Powered by Kreative Kompas", nl "Collaboration" → "`**Samenwerking**`"). These are hard failures; the string falls back to English.
6. (2026-09-29) Glossary names stay exactly as written in Latin-script languages (hard check); other scripts may transliterate them (ja フォーク for "fork") (soft check).
7. (2026-09-29) Hold low-resource languages (Irish): the 9B reviewer made LibreTranslate's correct Irish worse, and scores from small models are unreliable there.
8. (2026-10-03) LibreTranslate can fail on a pair it advertises (zh-Hans → en returned HTTP 500 for three nights). Catch errors per batch, let an LLM translate or back-translate that batch, and never let one language stop the run.
9. (2026-10-03) English tech loanwords are normal in Dutch and German IT text ("demo games", "fork", "open-source"); don't "fix" them into literal words ("vork" is a kitchen fork).
10. (2026-10-03) Language switcher flags: self-hosted SVGs (flag-icons, MIT), never emoji flags (Windows shows letters). A region-tagged locale uses its own country (pt-BR → Brazil, pt → Portugal, zh-Hant → Taiwan), en → United Kingdom, Arabic → the Arab League flag rather than one country. Keep the mapping in an editable data file; the flag image gets `alt=""` because the language name is right beside it.
11. (2026-10-03) langdetect is unreliable on short technical phrases (Dutch guessed as Afrikaans or Norwegian) and has no Irish model: use it as a soft flag only, on strings of six words or more.
