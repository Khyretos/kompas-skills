---
name: worker/localization
description: Translating the website (kk-localize): what to protect, what needs context.
roles: [worker, reviewer]
tags: [translate, localization, i18n]
paths: ["**/locales/**", "**/*.po"]
---
# Worker: localization (website translation)

These lessons come from the kk-localize pipeline (LibreTranslate draft, LLM review, independent judge). Model-specific notes: `_model-notes/qwen3/`.

Topic lessons moved into cards (languages, pipeline), loaded when a job needs them. Add new lessons to the card they belong to.

1. (2026-09-29) Protect code, markup, URLs and template actions as tokens like `XQ0X`: they survive LibreTranslate and the LLM, while `{0}`, `[[0]]` and `⟦0⟧` get mangled. Check that every token comes back exactly once.
2. (2026-09-29) One-word UI labels need context or a pinned translation: without it "About" becomes a bare preposition (nl "Over", de "Über"), "Foundation" a legal foundation (nl "Stichting", de "Stiftung"), "Build" a building (de "Gebäude"), "Creator" de "Schöpfer". Pass the linked page's description or a per-file note, and pin menu labels and page titles in `overrides/<lang>.yaml`.
3. (2026-09-29) Keep a heading or sentence in one string. Never split it around markup: mark the styled part with `**…**` and pass links in as parameters (`{{ .p0 }}`). Inline HTML inside a string makes models leave the wrapped word untranslated or drop the link text.
6. (2026-09-29) Glossary names stay exactly as written in Latin-script languages (hard check); other scripts may transliterate them (ja ディスコード for "Discord") (soft check).
12. (2026-10-03) Put the meaning of every short UI string in one context file in the source language, keyed by the English text (kk-localize `i18n-context.yaml`); one note fixes every language at once ("Services healthy" became "health services" or "a healthy lifestyle" in nl, cs and da until the note said it is a status label). Use per-language overrides only for what is left.
14. (2026-10-03) The glossary holds proper names only. A common noun in it breaks languages that capitalise nouns (de "Fork") and sends correct strings back to English; describe such terms in the context file instead.
19. (2026-10-03) Context notes can leak into the translation (it "mission-critical backends" → "backend critici per il governo e le banche", ja "Powered by" → "runs on open-source software"). Tell the model the notes are only for understanding and must never appear in the text, and audit strings that got a meaning note.
20. (2026-10-03) A model can return the context label instead of a translation (es "engine modules" → "1.label (item: módulos de motor)", scored 5 by the judge). Hard-fail outputs that contain context markers (`(item:`, `›`, `.yaml`, `0.label`) that the source doesn't have.
21. (2026-10-03) Placeholders for numbers are words, not word parts: models attach or move them (es "All {{ .p0 }} services" → "Todos los servicios {{ .p0 }}", nl "Alle {{ .p0 }}-diensten"). Check every string with a placeholder by reading it with a real number in it.
