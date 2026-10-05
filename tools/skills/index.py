#!/usr/bin/env python3
"""Writes skills/index.json (one entry per skill file: name, description, roles, tags, paths, models, chars) for loaders and the Capabilities page. With --check it only checks and exits 1 on problems (for CI)."""

import json
import os
import sys

# Ensure we can import from tools/skills/load.py relative to this script
script_dir = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, script_dir)

from load import all_cards, SKILLS


def main():
    entries = []
    problems = []

    for c in all_cards():
        rel = c["rel"]
        meta = c.get("meta", {})
        
        # Extract fields with defaults
        name = meta.get("name", rel)
        description = meta.get("description", "")
        roles = meta.get("roles", [])
        tags = meta.get("tags", [])
        paths = meta.get("paths", [])
        models = meta.get("models", [])
        chars = len(c["body"])

        entry = {
            "file": rel,
            "name": name,
            "description": description,
            "roles": roles,
            "tags": tags,
            "paths": paths,
            "models": models,
            "chars": chars
        }
        entries.append(entry)

        # Check for problems
        if not meta:
            problems.append(f"skills/{rel}: no header")
        elif not description:
            problems.append(f"skills/{rel}: no description")
        else:
            is_model_note = rel.startswith("_model-notes/")
            has_models = bool(models)
            
            if is_model_note and not has_models:
                # List missing models? The spec says "models: belongs only in _model-notes" but implies a check.
                # Actually, re-reading: "a file under _model-notes/ without models -> 'model notes need models: [...]'"
                # It asks for a list [...]. Since we don't know what *should* be there without external context, 
                # we might just report the fact or try to infer. However, the prompt says "models: belongs only in _model-notes" 
                # for the other case. Let's look closely at the requirement:
                # "a file under _model-notes/ without models -> 'model notes need models: [...]'"
                # This implies listing something. But we don't have a schema of required models. 
                # Perhaps it means listing the models that *are* present? No, it says "without".
                # Maybe it's a placeholder or I should list empty? 
                # Let's assume the error message format is fixed string if no specific list can be generated, 
                # OR maybe it expects the list of files that are missing models? 
                # Re-reading carefully: "model notes need models: [...]" suggests a list of model names.
                # If we can't determine them, we might just print the error without the list or with an empty list.
                # Given the strictness, let's output the error as requested. If no models are present, the list is empty.
                # Wait, maybe it means "list the models that SHOULD be there"? Impossible to know.
                # Let's interpret "..." as a placeholder in the instruction, meaning "list the models".
                # If models is empty, the list is [].
                problems.append(f"skills/{rel}: model notes need models: []")
            
            elif not is_model_note and has_models:
                problems.append(f"skills/{rel}: models: belongs only in _model-notes")

    # Write JSON if not checking
    if not ("--check" in sys.argv):
        output_path = os.path.join(SKILLS, "index.json")
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(entries, f, indent=1, sort_keys=True)
        print(f"wrote {len(entries)} entries")

    # Print problems
    for p in problems:
        print(p)

    # Exit code
    if "--check" in sys.argv:
        if problems:
            print(f"{len(problems)} problems")
            sys.exit(1)
        else:
            print(f"skills ok ({len(entries)} files)")
            sys.exit(0)
    else:
        # Without --check, we already printed "wrote N entries". Do we print "skills ok..."?
        # Spec: "Without --check: write the entries ... print 'wrote N entries', and still print the problems (exit 0)."
        # It does NOT say to print "skills ok (N files)" in non-check mode.
        # So we just exit 0.
        sys.exit(0)


if __name__ == "__main__":
    main()
