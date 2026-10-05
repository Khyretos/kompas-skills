"""SK-01, one-time: split skills/shared/SKILL.md into topic cards (content word for word) and
give every role core the card header, so tools/skills/load.py can pick what a job needs.
Run from the repo root. Prints what it did; refuses to run twice."""
import os, re, sys

CARDS = {  # card file -> (description, tags, heading prefixes that go into it)
    "shared/theming-brand.md": ("Theming apps and desktops in the Kreative Kompas brand: readability checks, colour roles, web, desktop and Kate themes.",
                                ["theme", "css", "colours", "brand", "contrast", "desktop", "kate"],
                                ["## 1. Theming", "## 2. Desktop theming", "## 3. Brand colour", "## 4. Web app theming", "## 10. Kate theming"]),
    "shared/models-and-gpus.md": ("Running local models on shared GPUs: Ollama hosts, model tags, night batches, CPU use, OVMS VRAM.",
                                  ["ollama", "ovms", "gpu", "vram", "model", "batch", "llama"],
                                  ["## 5. Ollama hosts", "## 6. One model tag", "## 7. Batch jobs", "## 6. Ollama/llama-server", "## (2026-10-04) OVMS is shared"]),
    "shared/processes.md": ("Background processes, long jobs, containers and CI: waiting, detaching, inspecting without leaking secrets.",
                            ["process", "background", "container", "docker", "ci", "secrets", "detached"],
                            ["## 3. Waiting for a background", "## 4. Inspecting containers", "## 5. CI jobs never", "## 9. Long jobs run detached"]),
}
CORES = {  # role core -> (description, roles, tags)
    "shared/SKILL.md": ("Facts every role needs: language priority, never pass a check by changing it.", ["orchestrator", "worker", "reviewer", "runner"], ["shared"]),
    "orchestrator/SKILL.md": ("Planning tasks into steps, building job prompts, running the drafting pipeline and PRs.", ["orchestrator"], ["plan", "prompt", "pipeline", "pr"]),
    "reviewer/SKILL.md": ("What to check in a draft, in order.", ["reviewer"], ["review", "check"]),
    "runner/SKILL.md": ("Running tools on computers within the access grants.", ["runner"], ["runner", "grant", "shell"]),
    "worker/rust/SKILL.md": ("Writing Rust for the Kompanion server, runner and helpers.", ["worker", "reviewer"], ["rust", "sqlx", "axum", "tokio"]),
    "worker/web/SKILL.md": ("Writing the vanilla TypeScript web app and its Playwright tests.", ["worker", "reviewer"], ["typescript", "web", "css", "html", "playwright"]),
    "worker/docs/SKILL.md": ("Writing READMEs, guides and task descriptions from given facts.", ["worker", "reviewer"], ["docs", "markdown", "readme"]),
    "worker/localization/SKILL.md": ("Translating the website (kk-localize): what to protect, what needs context.", ["worker", "reviewer"], ["translate", "localization", "i18n"]),
    "worker/cpp-games/SKILL.md": ("kk-engine games in C++: cameras, controllers, assets, tests on soucouyant.", ["worker", "reviewer"], ["cpp", "game", "engine"]),
}
PATHS = {"worker/rust/SKILL.md": ["**/*.rs", "**/Cargo.toml"], "worker/web/SKILL.md": ["web/**"], "worker/docs/SKILL.md": ["**/*.md"],
         "shared/theming-brand.md": ["**/*.css", "**/*theme*"]}

def header(name, desc, roles, tags, paths=None):
    h = ["---", f"name: {name}", f"description: {desc}", f"roles: [{', '.join(roles)}]", f"tags: [{', '.join(tags)}]"]
    if paths:
        h.append("paths: [" + ", ".join(f'"{p}"' for p in paths) + "]")
    return "\n".join(h + ["---", ""])

root = "skills"
shared = open(f"{root}/shared/SKILL.md").read()
if shared.startswith("---"):
    sys.exit("already split")
parts = re.split(r"(?m)^(?=## )", shared)
intro, sections = parts[0], parts[1:]
used = set()
for card, (desc, tags, prefixes) in CARDS.items():
    body = [s for s in sections if any(s.startswith(p) for p in prefixes)]
    used.update(id(s) for s in body)
    title = card.split("/")[1][:-3].replace("-", " ").capitalize()
    text = header(card[:-3], desc, ["worker", "reviewer", "orchestrator"], tags, PATHS.get(card)) + f"# Shared: {title}\n\n" + "".join(body).rstrip() + "\n"
    open(f"{root}/{card}", "w").write(text)
    print(f"{card}: {len(body)} sections, {len(text)} chars")
rest = [s for s in sections if id(s) not in used]
moved = ", ".join(c.split("/")[1] for c in CARDS)
core = intro.rstrip() + f"\n\nTopic lessons moved into cards ({moved}), loaded when a job needs them.\n\n" + "".join(rest)
open(f"{root}/shared/SKILL.md", "w").write(core)
print(f"shared/SKILL.md: {len(rest)} sections kept, {len(core)} chars")
for core_file, (desc, roles, tags) in CORES.items():
    p = f"{root}/{core_file}"
    t = open(p).read()
    if not t.startswith("---"):
        open(p, "w").write(header(core_file[:-3].replace("/SKILL", ""), desc, roles, tags, PATHS.get(core_file)) + t)
        print(f"{core_file}: header added")
