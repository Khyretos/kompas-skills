import os, re, fnmatch, glob

SKILLS = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "skills")
LOCAL = os.environ.get("KOMPANION_SKILLS_LOCAL", "/skills-local")

def parse(text):
    """(header dict, body) of a card. Header lines are `key: value`; a value in [ ] is a list of
    comma-separated items (quotes stripped). No header: ({}, text)."""
    if not text.startswith("---\n"):
        return {}, text
    end = text.find("\n---\n", 4)
    if end == -1:
        return {}, text
    meta = {}
    for line in text[4:end].splitlines():
        if ":" not in line:
            continue
        k, v = line.split(":", 1)
        v = v.strip()
        if v.startswith("[") and v.endswith("]"):
            meta[k.strip()] = [x.strip().strip('"').strip("'") for x in v[1:-1].split(",") if x.strip()]
        else:
            meta[k.strip()] = v
    return meta, text[end + 5:]

def all_cards(root=SKILLS):
    """Every .md under root except README.md: dicts with rel (path under root, no .md), meta, body."""
    out = []
    for path in sorted(glob.glob(os.path.join(root, "**", "*.md"), recursive=True)):
        rel = os.path.relpath(path, root)[:-3]
        if os.path.basename(rel) == "README":
            continue
        meta, body = parse(open(path, encoding="utf-8").read())
        out.append({"rel": rel, "meta": meta, "body": body.strip()})
    return out

def merged_cards(root=SKILLS, local=LOCAL):
    """Merge cards from general, kompanion, and private layers. Later layers override or extend earlier ones."""
    layers = []
    if os.path.isdir(os.path.join(root, "general")):
        layers.append(("general", all_cards(os.path.join(root, "general"))))
    layers.append(("kompanion", [c for c in all_cards(root) if not c["rel"].startswith("general/")]))
    if os.path.isdir(local):
        layers.append(("private", all_cards(local)))
    
    result = {}
    for layer, cards in layers:
        for c in cards:
            c["layer"] = layer
            target = c["meta"].get("overrides") or c["meta"].get("extends")
            if not target:
                result[c["rel"]] = c
                continue
            if target not in result:
                raise ValueError(f"{c['rel']}: overrides unknown card {target}")
            old = result[target]
            body = c["body"] if c["meta"].get("overrides") else old["body"] + "\n\n" + c["body"]
            result[target] = dict(old, body=body, layer=layer)
    return result

def words(text):
    return set(w for w in re.split(r"[^a-z0-9]+", text.lower()) if w)

def score(card, task_words, paths):
    """3 per job path that matches one of the card's `paths` globs, 1 per tag in the task text."""
    s = 0
    for p in paths:
        if any(fnmatch.fnmatch(p, g) for g in card["meta"].get("paths", [])):
            s += 3
    s += sum(1 for t in card["meta"].get("tags", []) if t.lower() in task_words)
    return s

def select(role, task_text="", paths=(), notes="", budget_tokens=1500, extra=(), root=SKILLS, local=LOCAL):
    """(list of rel names, text) for a job of `role` (e.g. "worker/web")."""
    cards = merged_cards(root, local)
    always = ["work-habits", "shared/SKILL", f"{role}/SKILL"] + [e for e in extra] + ([f"_model-notes/{notes}/SKILL"] if notes else [])
    picked = []
    for name in always:
        if name in cards and name not in picked:
            picked.append(name)
        elif name in extra and name not in cards:
            raise ValueError(f"unknown skill card {name}")
    family = role.split("/")[0]
    task_words = words(task_text)
    candidates = []
    for rel, c in cards.items():
        if rel in picked or rel.endswith("/SKILL") or rel.startswith("_model-notes/") or rel == "work-habits":
            continue
        roles = c["meta"].get("roles", [])
        if roles and family != "shared" and family not in roles:
            continue
        # Area rule: a card with 3 parts (e.g. "worker/rust/sql") is picked only for a job of that area.
        # Skip it when the role has 2 parts (e.g. "worker/web") with the same first part but different second part.
        parts_card = rel.split("/")
        parts_role = role.split("/")
        if len(parts_card) == 3 and len(parts_role) == 2 and parts_card[0] == parts_role[0] and parts_card[1] != parts_role[1]:
            continue
        sc = score(c, task_words, paths)
        if sc > 0:
            candidates.append((-sc, rel))
    used = 0
    for _, rel in sorted(candidates):
        size = len(cards[rel]["body"]) // 4
        if used + size > budget_tokens:
            continue
        picked.append(rel)
        used += size
    return picked, "\n\n".join(cards[r]["body"] for r in picked)
