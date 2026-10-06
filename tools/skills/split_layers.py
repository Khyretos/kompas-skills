"""Split skill cards into general, kompanion, and private layers.

This module provides a one-off tool to parse skill cards (Markdown files with front matter)
and split their content blocks into three layers: general, kompanion, and private.

Usage:
    python split_layers.py list <card.md>
        Print one line per block: number, tab, first line (stripped, max 120 chars).

    python split_layers.py apply <tags_dir> <private_dir> [--root DIR]
        Apply the layer tags from <tags_dir> to cards in <root> (default: tools/skills/),
        writing results to <root>/general/, <root>/kompanion/, and <private_dir>.
"""

import glob
import os
import sys
import re
from pathlib import Path


def split_card(text: str):
    """Split a card text into header and blocks.

    Args:
        text: The full text of the card.

    Returns:
        A tuple (header, blocks) where:
            - header: The front matter text including both --- lines and trailing newline,
              or "" if no front matter.
            - blocks: A list of lists of raw lines representing the body blocks.
              Each line keeps its "\n". Blank lines stay in the block before them.
              "".join(blocks) == body exactly.
    """
    # Check for front matter
    if not text.startswith("---\n"):
        return "", [text] if text else []

    # Find end of front matter
    end = text.find("\n---\n", 4)
    if end == -1:
        # Malformed front matter, treat as no front matter
        return "", [text.splitlines(keepends=True)] if text else []

    header = text[:end + 5]
    body = text[end + 5:]

    if not body:
        return header, [header] if header else []

    # Split body into blocks
    blocks = []
    current_block = []
    in_code_fence = False

    for line in body.splitlines(keepends=True):
        stripped = line.lstrip()
        
        # Check if entering/exiting code fence
        if stripped.startswith("```"):
            in_code_fence = not in_code_fence
            current_block.append(line)
            continue
        
        if in_code_fence:
            current_block.append(line)
            continue

        # Check if this line starts a new block
        # New block starts at a body line (other than first) that:
        # - starts with "#"
        # - matches "^(- |\* |\d+\. )" at column 0
        is_first_line = len(current_block) == 0
        
        should_start_new = False
        if not is_first_line:
            if stripped.startswith("#"):
                should_start_new = True
            elif re.match(r"^(- |\* |\d+\. )", stripped):
                should_start_new = True
        
        if should_start_new:
            if current_block:
                blocks.append(current_block)
            current_block = [line]
        else:
            current_block.append(line)

    if current_block:
        blocks.append(current_block)

    return header, blocks


def get_section(block_lines):
    """Get the section index for a block.

    Returns the index of the closest heading block before it whose first line starts with "##",
    or None when there is none.
    """
    return None  # Not used in this implementation as we track sections differently


def is_heading_block(block_lines):
    """Check if a block is a heading block (first line starts with "#")."""
    if not block_lines:
        return False
    return block_lines[0].lstrip().startswith("#")


def is_title_block(block_lines):
    """Check if a block is a title block (first line starts with "# ")."""
    if not block_lines:
        return False
    return block_lines[0].lstrip().startswith("# ")


SKILLS = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "skills")
LAYERS = ("general", "kompanion", "private")


def read_tags(path, n):
    """{block number: layer} from "<n> <layer>" lines; raises ValueError naming the problem
    unless every number 1..n appears exactly once with a layer in LAYERS."""
    tags = {}
    for line in open(path, encoding="utf-8"):
        parts = line.split()
        if not parts:
            continue
        if len(parts) != 2 or not parts[0].isdigit():
            raise ValueError(f"bad line {line.strip()!r}")
        num, layer = int(parts[0]), parts[1]
        if layer not in LAYERS:
            raise ValueError(f"block {num}: unknown layer {layer!r}")
        if num in tags or not 1 <= num <= n:
            raise ValueError(f"block {num}: duplicate or out of range 1..{n}")
        tags[num] = layer
    missing = sorted(set(range(1, n + 1)) - set(tags))
    if missing:
        raise ValueError(f"blocks without a tag: {missing}")
    return tags


def sections(blocks):
    """For each block, the index of the closest heading block at or before it whose first
    line starts with "##" (None when there is none)."""
    last, out = None, []
    for i, block in enumerate(blocks):
        if block and block[0].startswith("##"):
            last = i
        out.append(last)
    return out


def layers_of(blocks, tags):
    """(base layer, layer of each block): the title block goes to the base layer."""
    title = 0 if blocks and is_title_block(blocks[0]) else None
    rest = [tags[i + 1] for i in range(len(blocks)) if i != title]
    base = next((l for l in LAYERS if l in rest), "kompanion")
    return base, [base if i == title else tags[i + 1] for i in range(len(blocks))]


def plan(header, blocks, tags, rel):
    """{layer: text} for the layers that get blocks (tags: {1-based number: layer})."""
    base, layer = layers_of(blocks, tags)
    sec = sections(blocks)
    lines = {l: [] for l in LAYERS}
    added = {l: set() for l in LAYERS}
    for i, block in enumerate(blocks):
        l, s = layer[i], sec[i]
        if s is not None and s != i and layer[s] != l and s not in added[l]:
            lines[l].append(blocks[s][0])
            added[l].add(s)
        lines[l].extend(block)
    return {l: (header if l == base else f"---\nextends: {rel}\n---\n") + "".join(lines[l]).rstrip("\n") + "\n"
            for l in LAYERS if lines[l]}


def apply(tags_dir, private_dir, root):
    """Split every tagged card under root into the general, Kompanion and private layers."""
    todo, errors = [], []
    for path in sorted(glob.glob(os.path.join(root, "**", "*.md"), recursive=True)):
        rel = os.path.relpath(path, root)[:-3].replace(os.sep, "/")
        tags_path = os.path.join(tags_dir, rel + ".txt")
        if (os.path.basename(rel) == "README" or rel.startswith("general/")
                or os.path.exists(os.path.join(root, "general", rel + ".md")) or not os.path.exists(tags_path)):
            continue
        header, blocks = split_card(open(path, encoding="utf-8").read())
        if any(l.startswith(("extends:", "overrides:")) for l in header.splitlines()):
            continue
        try:
            todo.append((path, rel, header, blocks, read_tags(tags_path, len(blocks))))
        except ValueError as e:
            errors.append(f"{rel}: {e}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    for path, rel, header, blocks, tags in todo:
        _, layer = layers_of(blocks, tags)
        if all(l == "kompanion" for l in layer):
            continue
        texts = plan(header, blocks, tags, rel)
        targets = {"general": os.path.join(root, "general", rel + ".md"), "kompanion": path,
                   "private": os.path.join(private_dir, rel + ".md")}
        for l, text in texts.items():
            os.makedirs(os.path.dirname(targets[l]), exist_ok=True)
            open(targets[l], "w", encoding="utf-8").write(text)
        if "kompanion" not in texts:
            os.remove(path)
        print(f"{rel}: " + ", ".join(f"{l} {layer.count(l)}" for l in LAYERS))


def main():
    # argv: "list <card.md>": print f"{n}\t{first line stripped, max 120 chars}" per block (n from 1).
    # "apply <tags_dir> <private_dir> [--root DIR]" (DIR default SKILLS). Else print usage, exit 2.
    import argparse
    
    parser = argparse.ArgumentParser(description="Split markdown cards into layers.")
    subparsers = parser.add_subparsers(dest="command", help="Commands")
    
    list_parser = subparsers.add_parser("list", help="List blocks in a card")
    list_parser.add_argument("card", help="Path to the card file")
    
    apply_parser = subparsers.add_parser("apply", help="Apply tags and split cards")
    apply_parser.add_argument("tags_dir", help="Directory containing .txt tag files")
    apply_parser.add_argument("private_dir", help="Directory for private cards")
    apply_parser.add_argument("--root", default=SKILLS, help="Root directory for cards (default: SKILLS)")
    
    args = parser.parse_args()
    
    if args.command == "list":
        with open(args.card, "r", encoding="utf-8") as f:
            content = f.read()
        _, blocks = split_card(content)
        for i, block in enumerate(blocks, 1):
            if block:
                first_line = block[0].strip()[:120]
                print(f"{i}\t{first_line}")
    elif args.command == "apply":
        apply(args.tags_dir, args.private_dir, args.root)
    else:
        parser.print_help()
        sys.exit(2)


if __name__ == "__main__":
    main()


