# Lessons learned (Kompanion)

Every review finding for Kompanion work gets one row here, and its rule goes into the skill the
models read (`skills/`). The goal: any model that fills a role (today Qwen3.5-9B, later a bigger
one) works as close as it can to the strongest reviewer. Rules live in the skills; this file is
the index. `skills/work-habits.md` holds the habits behind most rows and goes to every job.

Columns: **Who** = who made the mistake (Coder, Claude, tool). **Rule** = what to do now.
**Filed in** = where the models read it. Newest last.

| Task | Who | What went wrong | Rule now | Filed in |
|---|---|---|---|---|
| README refresh | tool | `pipeline.py` `strip()` took the README's first ```` ```sh ```` block as the answer's start and cut 70 lines | An unwrapped `.md` answer is kept whole; check every docs draft's line count | `pipeline.py`, `skills/_model-notes/qwen3` |
| README refresh | Coder | Dropped a "keep this sentence" asked for in passing inside a long item | Every must-keep sentence gets its own numbered item, quoted in full; list the must-keeps before answering | `skills/worker/docs` 7, `qwen3` notes, `work-habits` |
| README refresh | Claude | Told Coder the desktop app was "in review" minutes before Kees merged it | Re-check PR states right before writing a facts list | this file |
| README screenshots | Claude | Wrote `shoot.mjs` itself instead of handing it to Coder | Local models draft every change, docs and scripts included | memory `local-ai-first` |
| README screenshots | Claude | No Playwright browser on the host | Run in `mcr.microsoft.com/playwright:v1.63.0-noble` | `skills/worker/web` 37 |
| README screenshots | Claude | ESM script outside `web/` could not import `playwright` | `createRequire(join(process.cwd(), "x.js"))` | `skills/worker/web` 38 |
| README screenshots | Claude | `scrollIntoView` in the settings dialog also moved the page behind it | Scroll only the dialog's scroll box | `skills/worker/web` 39 |
| README screenshots | Claude | Waited for finished step cards to be visible; they fold shut | Wait `attached`, then open them in `page.evaluate` | `skills/worker/web` 40 |
| PR #27 update | Claude | Plain `git push origin <branch>` made a stray branch instead of updating the PR | Push to `refs/for/main -o topic=<branch>` | `ai-skills/git` |
| PR #27 update | Claude | Drafting-log merge conflict | `git merge-file --union`, then check every line is JSON | `ai-skills/git` |
| Install script | Coder | `case ... *)` rejected running with no argument; option checked after the 1-minute build | Parse options at the top, `[ $# -eq 0 ]` first | `ai-skills/shell`, `qwen3` notes |
| Install script | Coder | Comments copied the prompt's numbered steps | No comments that repeat the step list | `qwen3` notes |
| Desktop README patch | Claude | Unquoted heredoc ran the prompt's backticks as commands; the code spans vanished and Coder wrote the gaps | Quote heredoc delimiters (`<<'EOF'`) when building prompts | `skills/orchestrator`, `ai-skills/shell` |
| Desktop install | Claude | `hyprctl dispatch exec` fails on this Hyprland (Lua config) | `setsid -f` with WAYLAND_DISPLAY; prove it with `hyprctl clients -j` + `grim -g` and look at the shot | `ai-skills/homelab-ops` |
| Desktop install | Claude | soucouyant's login shell is fish; `$?`, `"^$"` broke over SSH | `ssh host "bash -lc '...'"` | `ai-skills/shell` |
| Skills layout | Claude | General rules (tests, prompt size, themes, fix rounds) lived only in `_model-notes/qwen3`, so another model (a 27B later) would never read them, and every Qwen prompt carried 15k characters of notes | General rules in role cores and cards; model notes only for that family's quirks; old notes kept as history | `skills/README.md`, `orchestrator/prompting-workers.md`, `shared/colour-themes.md`, `worker/python.md`, `docs/ai-capability/qwen3-notes-history-2026-10.md` |
| Skills layout | Claude | Added a heredoc lesson to the orchestrator skill that was already its first bullet | Read the whole target file before adding a lesson (work-habits: read before you write) | this file |
