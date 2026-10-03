#!/usr/bin/env python3
"""Queue drafting jobs to the local model on soucouyant (Ollama, gemma4:12b-it-qat), back to back.

Each job: draft -> self-review against the role's skills -> final file.
jobs.json: [{"name": "...", "role": "worker/rust"|"worker/web"|..., "prompt": "...",
             "context": ["path/to/file", ...], "out": "path/to/write"}]
Usage: pipeline.py jobs.json [log.jsonl]
Writes the final code to each job's "out" and one log line per job
(seconds on the GPU, tokens). Code fences are stripped from the output.
"""
import json, os, re, sys, time, urllib.request

OLLAMA = os.environ.get("OLLAMA_URL", "http://192.168.178.80:11434/v1/chat/completions")
# One model name per Ollama host: a second name makes Ollama reload on every switch.
MODEL = os.environ.get("QWEN_MODEL", "gemma4:12b-it-qat")
NOTES = "gemma4" if MODEL.startswith("gemma4") else MODEL.split(":")[0].split(".")[0]  # skills/_model-notes/<NOTES>
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def skills(role):
    parts = []
    for rel in [f"skills/{role}/SKILL.md", f"skills/_model-notes/{NOTES}/SKILL.md", "skills/shared/SKILL.md"]:
        p = os.path.join(REPO, rel)
        if os.path.exists(p):
            parts.append(open(p).read())
    return "\n\n".join(parts)

def ask(system, user, max_tokens):
    body = json.dumps({"model": MODEL, "max_tokens": max_tokens, "temperature": 0.2,
                       "reasoning_effort": "none",
                       "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]}).encode()
    req = urllib.request.Request(OLLAMA, body, {"Content-Type": "application/json"})
    t = time.time()
    with urllib.request.urlopen(req, timeout=1200) as r:
        v = json.load(r)
    choice = v["choices"][0]
    if choice.get("finish_reason") == "length":
        # Never write a cut-off answer over a file.
        raise RuntimeError(f"answer cut off at the context limit ({v.get('usage', {})})")
    return choice["message"]["content"], time.time() - t, v.get("usage", {}).get("completion_tokens", 0)

def strip(text):
    """The code from an answer: from the first opening fence to the LAST closing
    one (code can contain fences itself, e.g. a ```toml example in a doc
    comment); an answer without fences is taken as it is."""
    text = text.strip()
    m = re.search(r"^```[a-zA-Z0-9_+-]*[ \t]*\n", text, re.M)
    if not m:
        return text + "\n"
    body = text[m.end():]
    end = body.rfind("\n```")
    if end != -1:
        body = body[:end]
    return body.strip() + "\n"

def main():
    jobs = json.load(open(sys.argv[1]))
    log = open(sys.argv[2] if len(sys.argv) > 2 else os.devnull, "a")
    for job in jobs:
      try:
        rules = skills(job["role"])
        ctx = "".join(f"\n--- {c} ---\n{open(os.path.join(REPO, c)).read()}" for c in job.get("context", []))
        system = "You write exact, compiling code for this repository. Follow these rules strictly:\n\n" + rules
        draft, s1, t1 = ask(system, job["prompt"] + ("\n\nRelevant files:" + ctx if ctx else ""), job.get("max_tokens", 4000))
        # The self-review needs the draft in the prompt; skip it when that
        # wouldn't leave room for a full answer in the 8k context (~4 chars/token).
        if (len(system) + len(job["prompt"]) + 2 * len(draft)) / 4 > 6000 or not job.get("review", True):
            out = os.path.join(REPO, job["out"])
            os.makedirs(os.path.dirname(out), exist_ok=True)
            open(out, "w").write(strip(draft))
            rec = {"name": job["name"], "out": job["out"], "gpu_seconds": round(s1, 1), "tokens": t1,
                   "lines": strip(draft).count("\n"), "review": "skipped", "at": time.strftime("%Y-%m-%dT%H:%M:%S")}
            log.write(json.dumps(rec) + "\n"); log.flush(); print(json.dumps(rec), flush=True)
            continue
        review_prompt = ("Review the code below against every rule above and the task. Fix every problem you find. "
                         "Output ONLY the corrected complete file, nothing else.\n\nTask:\n" + job["prompt"] + "\n\nCode:\n" + strip(draft))
        final, s2, t2 = ask(system, review_prompt, job.get("max_tokens", 4000))
        out = os.path.join(REPO, job["out"])
        os.makedirs(os.path.dirname(out), exist_ok=True)
        open(out, "w").write(strip(final))
        rec = {"name": job["name"], "out": job["out"], "gpu_seconds": round(s1 + s2, 1), "tokens": t1 + t2,
               "lines": strip(final).count("\n"), "at": time.strftime("%Y-%m-%dT%H:%M:%S")}
        log.write(json.dumps(rec) + "\n"); log.flush()
        print(json.dumps(rec), flush=True)
      except RuntimeError as e:
        print(json.dumps({"name": job["name"], "error": str(e)}), flush=True)

if __name__ == "__main__":
    main()
