# kompas-skills

General lessons for AI models that plan, write, review and run code: work habits, prompting
worker models, fix rounds, shell, git, Python, Rust, web, Android, localization, colour themes,
and quirks of model families (`_model-notes/<family>/`).

Each card is a Markdown file with a small header (`name`, `description`, `roles`, `tags`,
`paths`) and numbered, dated lessons. Kreative Kompanion
loads these cards as its general skill layer and adds its own and a setup's private cards on top
(`extends:` adds to a card, `overrides:` replaces it).

This library holds no setup facts: CI runs `tools/privacy_check.py`, which fails on IP addresses,
home paths, email addresses and private host names.

Licence: [CC BY-SA 4.0](LICENSE).
