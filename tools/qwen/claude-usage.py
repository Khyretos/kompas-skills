#!/usr/bin/env python3
"""
Sum Claude Code's token usage for a time window from a session transcript.
Usage: claude-usage.py [--since ISO] [--until ISO] [transcript.jsonl]
Default transcript is the newest *.jsonl in ~/.claude/projects/<cwd-with-dashes>/; prints one JSON object.
"""

import argparse
import json
import os
import sys
from datetime import datetime


def newest_transcript(cwd: str) -> str | None:
    """Find the path of the newest .jsonl file in the Claude projects directory."""
    base = os.path.join(os.path.expanduser("~/.claude/projects"))
    target_dir = base + "/" + cwd.replace("/", "-")
    
    if not os.path.isdir(target_dir):
        return None
    
    try:
        files = [f for f in os.listdir(target_dir) if f.endswith(".jsonl")]
        if not files:
            return None
        
        newest_path = None
        newest_mtime = -1
        
        for filename in files:
            filepath = os.path.join(target_dir, filename)
            mtime = os.path.getmtime(filepath)
            if mtime > newest_mtime:
                newest_mtime = mtime
                newest_path = filepath
        
        return newest_path
    except OSError:
        return None


def parse_iso(s: str) -> datetime:
    """Parse an ISO string to a datetime object."""
    # Handle both with and without microseconds
    for fmt in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ", "%Y-%m-%dT%H:%M:%S"):
        try:
            return datetime.strptime(s, fmt)
        except ValueError:
            continue
    raise ValueError(f"Cannot parse ISO string: {s}")


def sums(path: str, since: str | None = None, until: str | None = None) -> dict:
    """Read the file line by line and sum token usage."""
    answers = 0
    total_output = 0
    total_input = 0
    total_cache_read = 0
    total_cache_write = 0
    
    seen_ids = set()
    
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            
            # Check if it's an assistant message with usage
            if obj.get("type") != "assistant":
                continue
            
            message = obj.get("message") or {}
            usage = message.get("usage")
            if not isinstance(usage, dict):
                continue
            
            msg_id = message.get("id")
            if not msg_id or msg_id in seen_ids:
                continue
            
            # Time filtering
            timestamp = obj.get("timestamp")
            if timestamp:
                try:
                    ts_dt = parse_iso(timestamp)
                    if since and ts_dt < parse_iso(since):
                        continue
                    if until and ts_dt > parse_iso(until):
                        continue
                except ValueError:
                    continue
            
            seen_ids.add(msg_id)
            answers += 1
            
            # Sum tokens (missing fields count 0)
            total_output += usage.get("output_tokens", 0)
            total_input += usage.get("input_tokens", 0)
            total_cache_read += usage.get("cache_read_input_tokens", 0)
            total_cache_write += usage.get("cache_creation_input_tokens", 0)
    
    return {
        "answers": answers,
        "output": total_output,
        "input": total_input,
        "cache_read": total_cache_read,
        "cache_write": total_cache_write
    }


def find_git_parent_dirs(start: str, max_depth: int = 5) -> list[str]:
    """Find parent directories up to max_depth that might contain a git repo."""
    parts = start.split("/")
    result = []
    
    for i in range(len(parts), 0, -1):
        candidate = "/".join(parts[:i])
        if candidate == "":
            continue
        result.append(candidate)
        if i >= len(parts) - max_depth:
            break
    
    return result


def main():
    parser = argparse.ArgumentParser(description="Sum Claude Code's token usage from a session transcript.")
    parser.add_argument("--since", type=str, help="ISO timestamp (inclusive)")
    parser.add_argument("--until", type=str, help="ISO timestamp (exclusive)")
    parser.add_argument("transcript", nargs="?", help="Path to transcript file")
    
    args = parser.parse_args()
    
    transcript_path = args.transcript
    
    if not transcript_path:
        cwd = os.getcwd()
        transcript_path = newest_transcript(cwd)
        
        if not transcript_path:
            # Try git parent folders
            parent_dirs = find_git_parent_dirs(cwd)
            for parent in parent_dirs:
                pth = newest_transcript(parent)
                if pth:
                    transcript_path = pth
                    break
    
    if not transcript_path:
        print("Error: No transcript found", file=sys.stderr)
        sys.exit(1)
    
    result = sums(transcript_path, args.since, args.until)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
