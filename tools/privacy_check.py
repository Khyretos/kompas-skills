#!/usr/bin/env python3
"""
privacy_check.py

Scans markdown files for sensitive information that should not be in the public skills library.
Checks for:
- IPv4 addresses (excluding 127.0.0.1 and 0.0.0.0)
- Home paths (/home/ or ~/)
- Email addresses
- Hostnames listed in a local deny list

Usage:
    privacy_check.py <path>... [--deny FILE]

Exit codes:
    0: All files are clean.
    1: One or more findings were detected.
"""

import os
import re
import sys
from pathlib import Path


def load_deny_list(deny_path):
    """Load hostnames from the deny list file."""
    hosts = set()
    if not os.path.exists(deny_path):
        return hosts

    with open(deny_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            # Ignore blank lines and comments
            if not line or line.startswith("#"):
                continue
            hosts.add(line.lower())
    return hosts


def findings(text, deny_hosts):
    """
    Scan text for privacy-sensitive patterns.

    Returns a list of tuples: (line_number, kind, match_string)
    """
    results = []
    lines = text.splitlines()

    # Regex patterns
    ip_pattern = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
    email_pattern = re.compile(r"[\w.+-]+@[\w-]+\.[\w.-]+")
    home_pattern = re.compile(r"/home/|~/")

    for line_num, line in enumerate(lines, start=1):
        # Check IP addresses
        for match in ip_pattern.finditer(line):
            ip = match.group()
            if ip not in ("127.0.0.1", "0.0.0.0"):
                results.append((line_num, "ip", ip))

        # Check home paths
        match = home_pattern.search(line)
        if match:
            results.append((line_num, "home path", match.group()))

        # Check emails
        for match in email_pattern.finditer(line):
            results.append((line_num, "email", match.group()))

        # Check deny-list hosts (whole word, case-insensitive)
        line_lower = line.lower()
        for host in deny_hosts:
            # Match whole word using regex \b
            pattern = r"\b" + re.escape(host) + r"\b"
            if re.search(pattern, line_lower):
                results.append((line_num, "host", host))

    return results


def main():
    args = sys.argv[1:]

    # Parse arguments
    deny_file = None
    paths = []

    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "--deny":
            if i + 1 >= len(args):
                print("Error: --deny requires a filename", file=sys.stderr)
                sys.exit(1)
            deny_file = args[i + 1]
            i += 2
        else:
            paths.append(arg)
            i += 1

    # Determine default deny path
    default_deny = (
        os.environ.get("KOMPANION_SKILLS_LOCAL", "/skills-local") + "/deny-hosts.txt"
    )
    if deny_file is None:
        deny_file = default_deny

    # Load deny list
    deny_hosts = load_deny_list(deny_file)

    all_findings = []
    files_checked = 0

    for path_arg in paths:
        p = Path(path_arg)
        if not p.exists():
            print(f"privacy check: no such path: {path_arg}", file=sys.stderr)
            sys.exit(2)

        if p.is_file():
            files_checked += 1
            try:
                content = p.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                # Skip binary files or files with encoding issues
                continue

            results = findings(content, deny_hosts)
            all_findings.extend([(str(p), r) for r in results])

        elif p.is_dir():
            # Recursive search for .md files
            for md_file in p.rglob("*.md"):
                files_checked += 1
                try:
                    content = md_file.read_text(encoding="utf-8")
                except UnicodeDecodeError:
                    continue

                results = findings(content, deny_hosts)
                all_findings.extend([(str(md_file), r) for r in results])

    # Output results
    if all_findings:
        for file_path, (line_num, kind, match) in all_findings:
            print(f"{file_path}:{line_num}: {kind}: {match}")
        sys.exit(1)
    else:
        print(f"privacy check: {files_checked} files clean")
        sys.exit(0)


if __name__ == "__main__":
    main()
