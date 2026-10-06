---
name: worker/localization/pipeline
description: The kk-localize tooling: LibreTranslate quirks, Hugo language codes, extraction, langdetect, the judge, paused model servers, releasing languages.
roles: [worker, reviewer]
tags: [libretranslate, hugo, langdetect, judge, extract, extraction, release, pipeline, flags, switcher, template, server]
---
# Worker: localization (website translation): pipeline

8. (2026-10-03) LibreTranslate can fail on a pair it advertises (zh-Hans → en returned HTTP 500 for three nights). Catch errors per batch, let an LLM translate or back-translate that batch, and never let one language stop the run.
10. (2026-10-03) Language switcher flags: self-hosted SVGs (flag-icons, MIT), never emoji flags (Windows shows letters). A region-tagged locale uses its own country (pt-BR → Brazil, pt → Portugal, zh-Hant → Taiwan), en → United Kingdom, Arabic → the Arab League flag rather than one country. Keep the mapping in an editable data file; the flag image gets `alt=""` because the language name is right beside it.
11. (2026-10-03) langdetect is unreliable on short technical phrases (Dutch guessed as Afrikaans or Norwegian) and has no Irish model: use it as a soft flag only, on strings of six words or more.
15. (2026-10-03) A translation far longer than its source is usually the model's own notes leaking in (cs "Meet the engine" became "Poznámka: Pro daný kontext…", a note about the translation): hard-fail outputs over three times the source length and over 40 characters.
16. (2026-10-03) Never extract text from inside template code: a `printf` that builds `<a …>` looked like a text node, and the model "translated" that code fragment into a whole invented paragraph about the company.
18. (2026-10-03) Hugo lower-cases language codes (`pt-BR` becomes `pt-br`): look up per-language data such as the flag mapping case-insensitively (the Brazilian flag was missing).
35. (2026-10-04) The judge scores some correct short strings 1-2 (uk "Послуги" for Services, "Контейнери", ro "Proiect"). A low score sends you to look; it is not proof of an error. Pin the correct text instead of re-translating it.
36. (2026-10-05) Lesson 18 again, worse: the per-language data lookup used Hugo's lower-cased language (`pt-br`) against folders named `pt-BR`, so the services, stack and CV data of pt-BR, zh-Hans and zh-Hant were English for a day while the UI text and the "machine translated" note were fine. Compare language codes case-insensitively everywhere. A release check must look at translated *content*, not only the `lang` attribute and the note: fail when a translated page still contains a known English data sentence.
