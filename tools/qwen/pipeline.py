#!/usr/bin/env python3
"""Queue drafting jobs to Coder (Qwen3.5 9B int8, OVMS on kireserver's A770), back to back.

Kees, 2026-10-05: all Kompanion work runs on kireserver; soucouyant is only for image
and audio generation, so this never calls soucouyant's Ollama.

Each job: draft -> self-review against the role's skills -> final file.
jobs.json: [{"name": "...", "role": "worker/rust"|"worker/web"|..., "prompt": "...",
             "context": ["path/to/file", ...], "out": "path/to/write"}]
Usage: pipeline.py jobs.json [log.jsonl]
Writes the final code to each job's "out" and one log line per job
(seconds on the GPU, tokens). Code fences are stripped from the output.
"""
import json, os, re, sys, time, urllib.request

OVMS = os.environ.get("OVMS_URL", "http://172.16.1.25:8000/v3/chat/completions")  # direct: the proxy cuts long answers at 60 s
OVMS_MODEL = os.environ.get("OVMS_MODEL", "Coder")  # served name; today Qwen3.5-9B int8 on the A770
# Only quirks of one model family live in skills/_model-notes/<NOTES>; every general rule is in
# the role skills and work-habits.md, so a bigger model loaded later (MODEL_NOTES=gemma4, ...)
# reads the same lessons (Kees, 2026-10-05).
NOTES = os.environ.get("MODEL_NOTES", "qwen3")

import threading
LANE = threading.local()  # .model: the model the last call used

def backend():
    """Coder on OVMS (A770), thinking off."""
    env = os.popen("docker inspect ovms --format '{{range .Config.Env}}{{println .}}{{end}}'").read()
    key = next((l[len("API_KEY="):] for l in env.splitlines() if l.startswith("API_KEY=")), "")
    return (ovms_url(), OVMS_MODEL, {"chat_template_kwargs": {"enable_thinking": False}}, key)

def ovms_url():
    """OVMS_URL, else the container's current address: it changes when kireserver
    reboots (2026-10-04: .25 became .28 and every OVMS job got a 404)."""
    if "OVMS_URL" in os.environ:
        return OVMS
    ip = os.popen("docker inspect ovms --format '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}'").read().split()
    return f"http://{ip[0]}:8000/v3/chat/completions" if ip else OVMS

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def card(rel):
    """A skill card without its front matter (the header is for the loader, not the model)."""
    text = open(rel).read()
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end != -1:
            text = text[end + 5:]
    return text.strip()

def skills(role, extra=()):
    """work-habits + the role's core + the cards the job names (job["skills"], e.g.
    ["shared/colour-themes"]) + this model's quirks. Until the smart loader exists,
    the orchestrator picks the cards per job."""
    parts = []
    # Only the role's lessons and the model's notes, to keep prompts small.
    # work-habits.md: how the reviewer works, for every role (also in ai-skills/_shared).
    rels = ["skills/work-habits.md", f"skills/{role}/SKILL.md"] + [f"skills/{c}.md" for c in extra] + [f"skills/_model-notes/{NOTES}/SKILL.md"]
    for rel in rels:
        p = os.path.join(REPO, rel)
        if os.path.exists(p):
            parts.append(card(p))
        elif rel.startswith("skills/") and rel[7:-3] in extra:
            raise RuntimeError(f"unknown skill card {rel}")
    return "\n\n".join(parts)

def ask(system, user, max_tokens):
    url, model, extra, key = backend()
    LANE.model = model
    body = json.dumps({"model": model, "max_tokens": max_tokens, "temperature": 0.2,
                       "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}], **extra}).encode()
    headers = {"Content-Type": "application/json"}
    if key:
        headers["Authorization"] = "Bearer " + key
    req = urllib.request.Request(url, body, headers)
    print(f"[{model}]", file=sys.stderr, flush=True)
    t = time.time()
    with urllib.request.urlopen(req, timeout=1200) as r:
        v = json.load(r)
    choice = v["choices"][0]
    if choice.get("finish_reason") == "length":
        # Never write a cut-off answer over a file.
        raise RuntimeError(f"answer cut off at the context limit ({v.get('usage', {})})")
    return choice["message"]["content"], time.time() - t, v.get("usage", {}).get("completion_tokens", 0)

def strip(text, out=""):
    """The code from an answer: from the first opening fence to the LAST closing
    one (code can contain fences itself, e.g. a ```toml example in a doc
    comment); an answer without fences is taken as it is.
    Markdown files contain fences of their own: an .md answer counts as wrapped
    only when it starts with a fence (2026-10-05: an unwrapped README draft lost
    everything above its first ```sh block)."""
    text = text.strip()
    if out.endswith(".md") and not text.startswith("```"):
        return text + "\n"
    m = re.search(r"^```[a-zA-Z0-9_+-]*[ \t]*\n", text, re.M)
    if not m:
        return text + "\n"
    body = text[m.end():]
    end = body.rfind("\n```")
    if end != -1:
        body = body[:end]
    return body.strip() + "\n"

PATCH_RULES = """Answer ONLY with edit blocks for the file, no whole file and no prose. Each block:
<<<<<<< SEARCH
(lines copied EXACTLY from the current file, enough to be unique, usually 2-6 lines)
=======
(the new lines that replace them)
>>>>>>> REPLACE
Use several blocks for several places. To add code, SEARCH for the lines next to where it goes
and repeat them in REPLACE with the new code added. Never SEARCH for lines that are not in the file."""

BLOCK = re.compile(r"<<<<<<< SEARCH\n(.*?)\n?=======\n(.*?)\n?>>>>>>> REPLACE", re.S)

def find(text, search):
    """(start, end) of the one run of whole lines equal to `search`, comparing lines
    without indentation and trailing spaces. None when missing or not unique."""
    want = [l.strip() for l in search.strip("\n").split("\n")]
    lines = text.split("\n")
    hits = [i for i in range(len(lines) - len(want) + 1)
            if [l.strip() for l in lines[i:i + len(want)]] == want]
    if len(hits) != 1:
        return None
    start = sum(len(l) + 1 for l in lines[:hits[0]])
    end = start + sum(len(l) + 1 for l in lines[hits[0]:hits[0] + len(want)]) - 1
    return start, end

def apply_patch(text, answer):
    """Applies the edit blocks; returns (new text, error or None)."""
    blocks = BLOCK.findall(answer)
    if not blocks:
        return text, "no edit blocks found in the answer"
    for search, replace in blocks:
        where = find(text, search)
        if where is None:
            return text, "this SEARCH text is missing from the file or not unique:\n" + search[:600]
        text = text[:where[0]] + replace + text[where[1]:]
    return text, None

def patch_job(job, log):
    """mode "patch": the model answers with search/replace blocks for job["out"];
    a block that doesn't apply goes back to the model once with the error."""
    out = os.path.join(REPO, job["out"])
    text = open(out).read()
    system = "You edit code in this repository with exact, compiling changes. Follow these rules strictly:\n\n" + skills(job["role"], job.get("skills", ()))
    ctx = "".join(f"\n--- {c} ---\n{open(os.path.join(REPO, c)).read()}" for c in job.get("context", []) if c != job["out"])
    shown = text
    if job.get("focus"):
        # Big files: show only the lines around these regexes (OVMS Coder cuts prompts at ~8k tokens).
        lines = text.split("\n")
        keep = set()
        for pat in job["focus"]:
            for i, l in enumerate(lines):
                if re.search(pat, l):
                    keep.update(range(max(0, i - 6), min(len(lines), i + 25)))
        out_lines, last = [], -2
        for i in sorted(keep):
            if i != last + 1:
                out_lines.append("// ...")
            out_lines.append(lines[i]); last = i
        shown = "\n".join(out_lines) + "\n// ...\n"
    prompt = job["prompt"] + "\n\n" + PATCH_RULES + (("\n\nOther files, for reference only:" + ctx) if ctx else "") + f"\n\nThe file to edit, `{job['out']}`" + (" (excerpts; `// ...` marks skipped lines, never copy it)" if job.get("focus") else "") + f":\n```\n{shown}```"
    secs = toks = 0
    for attempt in range(2):
        answer, s, t = ask(system, prompt, job.get("max_tokens", 3000))
        secs += s; toks += t
        new, err = apply_patch(text, answer)
        if not err:
            break
        prompt += f"\n\nYour previous answer could not be applied: {err}\nAnswer again with blocks whose SEARCH lines are copied exactly from the file."
    if err:
        raise RuntimeError(f"patch failed twice: {err[:300]}")
    open(out, "w").write(new)
    rec = {"name": job["name"], "out": job["out"], "mode": "patch", "gpu_seconds": round(secs, 1), "tokens": toks,
           "lines": len(BLOCK.findall(answer)), "attempts": attempt + 1, "model": getattr(LANE, "model", ""), "at": time.strftime("%Y-%m-%dT%H:%M:%S")}
    log.write(json.dumps(rec) + "\n"); log.flush(); print(json.dumps(rec), flush=True)

def run_job(job, log):
    try:
      if job.get("mode") == "patch":
          patch_job(job, log)
          return
      rules = skills(job["role"], job.get("skills", ()))
      ctx = "".join(f"\n--- {c} ---\n{open(os.path.join(REPO, c)).read()}" for c in job.get("context", []))
      system = "You write exact, compiling code for this repository. Follow these rules strictly:\n\n" + rules
      draft, s1, t1 = ask(system, job["prompt"] + ("\n\nRelevant files:" + ctx if ctx else ""), job.get("max_tokens", 4000))
      # The self-review needs the draft in the prompt; skip it when that
      # wouldn't leave room for a full answer in the 16k context (~4 chars/token).
      if (len(system) + len(job["prompt"]) + 2 * len(draft)) / 4 > 13000 or not job.get("review", True):
          out = os.path.join(REPO, job["out"])
          os.makedirs(os.path.dirname(out), exist_ok=True)
          open(out, "w").write(with_footer(drop_path_line(strip(draft, job["out"]), job["out"]), job))
          rec = {"name": job["name"], "out": job["out"], "gpu_seconds": round(s1, 1), "tokens": t1,
                 "lines": strip(draft, job["out"]).count("\n"), "review": "skipped", "model": getattr(LANE, "model", ""), "at": time.strftime("%Y-%m-%dT%H:%M:%S")}
          log.write(json.dumps(rec) + "\n"); log.flush(); print(json.dumps(rec), flush=True)
          return
      review_prompt = ("Review the code below against every rule above and the task. Fix every problem you find. "
                       "Output ONLY the corrected complete file, nothing else.\n\nTask:\n" + job["prompt"] + "\n\nCode:\n" + strip(draft, job["out"]))
      final, s2, t2 = ask(system, review_prompt, job.get("max_tokens", 4000))
      out = os.path.join(REPO, job["out"])
      os.makedirs(os.path.dirname(out), exist_ok=True)
      open(out, "w").write(with_footer(drop_path_line(strip(final, job["out"]), job["out"]), job))
      rec = {"name": job["name"], "out": job["out"], "gpu_seconds": round(s1 + s2, 1), "tokens": t1 + t2,
             "lines": strip(final, job["out"]).count("\n"), "model": getattr(LANE, "model", ""), "at": time.strftime("%Y-%m-%dT%H:%M:%S")}
      log.write(json.dumps(rec) + "\n"); log.flush()
      print(json.dumps(rec), flush=True)
    except RuntimeError as e:
      print(json.dumps({"name": job["name"], "error": str(e)}), flush=True)

def with_footer(text, job):
    """Fixed boilerplate (a sign-off line, a licence header) is appended in code: models
    drop it even when the prompt shows it (2026-10-05, twice in one day)."""
    footer = job.get("footer")
    if not footer or text.rstrip().endswith(footer.strip()):
        return text
    return text.rstrip("\n") + "\n\n" + footer.strip() + "\n"

def drop_path_line(code, path):
    """The model sometimes puts the file's path on the first line; drop it."""
    first, _, rest = code.partition("\n")
    return rest if first.strip().strip("`/# ") in (path, path.split("/")[-1]) else code

def main():
    """One lane on Coder: OVMS serves 2 sequences at once and Kompanion or PR-Agent
    may need the other. Jobs for the same file stay in order."""
    jobs = json.load(open(sys.argv[1]))
    log = open(sys.argv[2] if len(sys.argv) > 2 else os.devnull, "a")
    groups = {}
    for job in jobs:
        groups.setdefault(job["out"], []).append(job)
    queue = list(groups.values())
    lock = threading.Lock()
    def lane():
        while True:
            with lock:
                if not queue:
                    return
                group = queue.pop(0)
            for job in group:
                run_job(job, log)
    lanes = [threading.Thread(target=lane)]
    for t in lanes: t.start()
    for t in lanes: t.join()

if __name__ == "__main__":
    main()
