"""
Split numbered lessons from role cores into topic cards.
Refuses to run twice: checks if any card file already exists under skills/.
"""

import json
import os
import re
import sys
from collections import Counter

MAP = "tools/skills/split_cores.json"


def header(name, desc, tags, paths):
    h = ["---", "name: " + name, "description: " + desc, "roles: [worker, reviewer]", "tags: [" + ", ".join(tags) + "]"]
    if paths:
        h.append("paths: [" + ", ".join('"' + p + '"' for p in paths) + "]")
    return "\n".join(h + ["---", ""])


def split(text):
    end = text.index("\n---\n", 4) + 5  # the header's closing "---" line
    head, body = text[:end], text[end:]
    parts = re.split(r"(?m)^(?=\d+\. )", body)
    intro = parts[0]
    lessons = parts[1:]
    return head, intro, lessons


def plan(core, cards, text):
    head, intro, lessons = split(text)
    strip = lambda l: l.split(". ", 1)[1]
    moved = {}
    for card in cards:
        for prefix in cards[card]["lessons"]:
            hits = [i for i, l in enumerate(lessons) if strip(l).startswith(prefix)]
            if len(hits) != 1:
                sys.exit(core + ": " + str(len(hits)) + " lessons start with " + repr(prefix))
            if hits[0] in moved:
                sys.exit(core + ": lesson " + str(hits[0]) + " reused by " + card)
            moved[hits[0]] = card
    title = None
    for line in intro.split("\n"):
        if line.startswith("# "):
            title = line[2:]
            break
    if not title:
        sys.exit(core + ": no title found in intro")
    out = {}
    for card in cards:
        name = card[:-3]
        base = name.split("/")[-1]
        body = "".join(lessons[i] for i in sorted(moved) if moved[i] == card).rstrip() + "\n"
        out[card] = header(name, cards[card]["description"], cards[card]["tags"], cards[card]["paths"]) + "# " + title + ": " + base.replace("-", " ") + "\n\n" + body
    kept = "".join(l for i, l in enumerate(lessons) if i not in moved)
    names = ", ".join(card[:-3].split("/")[-1] for card in cards)
    out[core] = head + intro.rstrip() + "\n\nTopic lessons moved into cards (" + names + "), loaded when a job needs them. Add new lessons to the card they belong to.\n\n" + kept.rstrip() + "\n"
    before = Counter(l for l in text.splitlines() if l.strip())
    after_lines = []
    for v in out.values():
        after_lines.extend(v.splitlines())
    after = Counter(l for l in after_lines if l.strip())
    missing = before - after
    if missing:
        sys.exit(core + ": lines lost: " + repr(list(missing)[:5]))
    return out


if __name__ == "__main__":
    data = json.load(open(MAP))
    cores = [k for k in data.keys() if k != "_doc"]
    for core in cores:
        cards = list(data[core].keys())
        for card in cards:
            path = "skills/" + card
            if os.path.exists(path):
                sys.exit("already split: " + path)
    plans = {}
    for core in cores:
        text = open("skills/" + core).read()
        plans[core] = plan(core, data[core], text)
    for core in cores:
        cards = list(data[core].keys())
        for card in cards:
            path = "skills/" + card
            text = plans[core][card]
            open(path, "w").write(text)
            print(path, len(text))
        path = "skills/" + core
        text = plans[core][core]
        open(path, "w").write(text)
        print(path, len(text))
