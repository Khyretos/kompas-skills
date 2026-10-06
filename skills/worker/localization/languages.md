---
name: worker/localization/languages
description: Per-language rules for website translation: form of address, scripts, RTL, verb order, dates, loaded words, which languages get a full audit.
roles: [worker, reviewer]
tags: [language, languages, address, rtl, script, dates, audit, pt, ru, nl, de, es, tr, ja, ko, ar, irish, cv]
---
# Worker: localization (website translation): languages

4. (2026-09-29) The CV is written in the first person singular ("I"); say so in a per-file note, or models switch to "he".
7. (2026-09-29) Hold low-resource languages (Irish): the 9B reviewer made LibreTranslate's correct Irish worse, and scores from small models are unreliable there.
9. (2026-10-03) English tech loanwords are normal in Dutch and German IT text ("demo games", "fork", "open-source"); don't "fix" them into literal words ("vork" is a kitchen fork).
13. (2026-10-03) Set the form of address for each language before translating (tú, vous, du, Sie, vy…), or the model mixes formal and informal on one page (es "Haga una pregunta" next to "Ponte en contacto").
22. (2026-10-03) Kees's own languages (es, nl) get a full audit, every string, not a sample: in each, about 40% of all strings needed a fix, most of them scored 4-5 by the judge (wrong sense, register, CV voice, date format, Title Case from English). Pinned text counts as reviewed: only placeholder and leak checks may block it.
23. (2026-10-04) Verb-final languages (tr, ja, ko, hi, and others) break UI sentences that end before a link or a rotating word: tr "… ile geliştirildi <a>Hugo</a>.", zh "…建置 Hugo.", ko "우리는 구축합니다 <rotating word>". Use colon wording that works with anything after it (tr "Yapım aracı: Hugo", "Geliştirdiklerimiz:", zh "构建工具：", "我们打造的是"), or pass the link in as a parameter. Audit every string that a layout continues after the text.
25. (2026-10-04) LibreTranslate's "pt" is Brazilian. A reviewer told only "European Portuguese" left a mix (Mídia, seus, em um, roda em, grade next to partilha, equipa). Name the markers in the style note: ficheiro, equipa, ecrã, multimédia, alojar, guardar, grelha, "o nosso", num/numa, "a correr".
26. (2026-10-04) Stat labels follow a number ("1 login for the whole team", "12 services in production"). CJK needs a counter word at the start of the label (zh "个账号，全团队通用", "项在线服务"); otherwise it reads as a heading. Read every stat label with its number in front.
27. (2026-10-04) Russian took "Engine" for a car motor (Двигатель) and "fork" for cutlery (вилка) even with context; pin those two in every Slavic language and check "headless" (ru gave a non-word, "Беспоголовый").
29. (2026-10-04) Right-to-left languages (ar, fa, he, ur) need `dir="rtl"` on `<html>`: set Hugo's `languageDirection = "rtl"` per language and emit `dir` only when it is set, so English output stays byte-identical. Flex layouts mirror by themselves; check borders and absolute positions (a timeline's line and dots, a dropdown anchored `right: 0`) with a screenshot at desktop and phone width.
30. (2026-10-04) Hard-fail letters from a third script, one that is neither the language's own nor Latin: ko "FBX 및 OBJ импорт", el "Κικбокσινγκ", bn "**сoздаем**", bg "Прыгане". A share-of-script check misses one foreign word in a long sentence. Allow º, 々, modifier letters and full-width Latin.
31. (2026-10-04) Dates are formatted, not translated: one CV came back with "Set 2025", "Outubro 2022 – Jul 2025" and "Março 2020". Format date ranges with CLDR data (Babel, BSD) per locale (ja "2017年3月", hu "2017. márc.") and keep pins for audited ones. Watch pt_PT, whose yMMM pattern is "03/2017".
32. (2026-10-04) Watch for words with a religious or other loaded sense: bn "Creator" became "সৃষ্টিকর্তা" (God the Creator); use "content creator". Slavic languages confirmed again: bg "Engine" = "Двигател" (a motor); use the loanword (bg "Енджин", uk "Рушій").
33. (2026-10-04) Set the form of address for EVERY language before its first run, including the reach languages: hu, el, ko and ro ran without one and mixed formal and informal on one page. A style note changes the cache key, so adding it later re-reviews the whole language.
37. (2026-10-05) "Used" for hardware stays ambiguous in many languages even with a context note: the model picked "used up / spent" (ja 使用済み, uk Використані), "employed" (sv Använda) or plain "used" (cs/sk Použité). Pin the unambiguous word (中古, Вживані, Begagnade, z druhé ruky) and check the line across all languages side by side after every change.
