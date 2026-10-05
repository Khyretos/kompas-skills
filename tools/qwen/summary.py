#!/usr/bin/env python3
"""
summary.py - Generate per-task cost line (TEN-05) for Qwen logs.

Usage: summary.py <branch> [--task TASK_ID] [--transcript PATH]

Reads docs/qwen-log/<branch>.jsonl, aggregates Coder stats, calls claude-usage.py
for Claude stats, and appends a summary JSONL entry to the log file.
"""

import argparse
import datetime
import json
import subprocess
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Summarize Qwen task costs.")
    parser.add_argument("branch", help="Branch name for the log file")
    parser.add_argument("--task", type=str, default=None, help="Task ID to filter by")
    parser.add_argument(
        "--transcript", type=str, default=None, help="Path to transcript for Claude usage"
    )
    return parser.parse_args()


def get_log_path(branch: str) -> Path:
    # Repo root is two levels above this script's location
    script_dir = Path(__file__).resolve().parent.parent.parent
    return script_dir / "docs" / "qwen-log" / f"{branch}.jsonl"


def format_utc_iso(dt: datetime.datetime) -> str:
    """Convert local datetime to UTC ISO string with trailing 'Z'."""
    utc_dt = dt.astimezone(datetime.timezone.utc)
    return utc_dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def call_claude_usage(since: str, until: str, transcript: str | None) -> dict:
    """Run claude-usage.py and parse its JSON output."""
    if transcript is None:
        raise ValueError("Transcript path required for Claude usage calculation")

    script_dir = Path(__file__).resolve().parent
    usage_script = script_dir / "claude-usage.py"

    cmd = [
        sys.executable,
        str(usage_script),
        "--since", since,
        "--until", until,
    ]
    if transcript:
        cmd.append(transcript)

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        check=False,
    )

    if result.returncode != 0:
        print(f"Error running claude-usage.py: {result.stderr}", file=sys.stderr)
        sys.exit(1)

    try:
        data = json.loads(result.stdout.strip())
        return data
    except json.JSONDecodeError as e:
        print(f"Failed to parse claude-usage.py output: {e}", file=sys.stderr)
        sys.exit(1)


def main():
    args = parse_args()

    log_path = get_log_path(args.branch)

    if not log_path.exists():
        print(f"Log file not found: {log_path}", file=sys.stderr)
        sys.exit(1)

    jobs = []
    now_local = datetime.datetime.now()

    with open(log_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue

            if entry.get("summary") is True:
                continue

            if "tokens" not in entry:
                continue

            jobs.append(entry)

    if not jobs:
        print("No job entries found in log.", file=sys.stderr)
        sys.exit(1)

    coder_jobs = len(jobs)
    coder_output = sum(e.get("tokens", 0) for e in jobs)
    coder_prompt = sum(e.get("prompt_tokens", 0) for e in jobs)
    coder_gpu_seconds = round(sum(e.get("gpu_seconds", 0) for e in jobs), 1)
    coder_lines = sum(e.get("lines", 0) for e in jobs)

    at_values = [datetime.datetime.strptime(e["at"], "%Y-%m-%dT%H:%M:%S") for e in jobs]
    first_at = min(at_values)
    last_at = max(at_values)

    since_utc = format_utc_iso(first_at - datetime.timedelta(minutes=30))
    until_utc = format_utc_iso(last_at + datetime.timedelta(minutes=30))

    claude_result = call_claude_usage(since_utc, until_utc, args.transcript)
    claude_output = claude_result.get("output", 0)

    total_output = coder_output + claude_output
    if total_output > 0:
        output_share_coder = round(coder_output / total_output, 2)
    else:
        output_share_coder = 0.0

    summary_entry = {
        "summary": True,
        "branch": args.branch,
        "task": args.task,
        "at": now_local.strftime("%Y-%m-%dT%H:%M:%S"),
        "coder": {
            "jobs": coder_jobs,
            "output": coder_output,
            "prompt": coder_prompt,
            "gpu_seconds": coder_gpu_seconds,
            "lines": coder_lines,
        },
        "claude": claude_result,
        "output_share_coder": output_share_coder,
    }

    with open(log_path, "a", encoding="utf-8") as f:
        f.write(json.dumps(summary_entry) + "\n")


if __name__ == "__main__":
    main()
