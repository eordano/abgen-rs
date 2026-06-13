#!/usr/bin/env python3
"""Relative-link checker for the markdown docs.

Scans every markdown file under docs/, plus README.md and the markdown notes
under dev/, for relative links of the form ](path) and ](path#anchor), and
verifies each target resolves to a file that exists. Absolute URLs (http://,
https://, mailto:) and pure in-page anchors (#section) are ignored.

Run from the repo root:  python3 dev/check_doc_links.py
Exits non-zero (and prints each broken link) if any link does not resolve.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# ](target) — a markdown link target.
LINK_RE = re.compile(r"\]\(([^)]+)\)")

SKIP_PREFIXES = ("http://", "https://", "mailto:", "#")


def md_files():
    for base in ("docs", "dev"):
        for dirpath, _dirs, files in os.walk(os.path.join(ROOT, base)):
            for name in files:
                if name.endswith(".md"):
                    yield os.path.join(dirpath, name)
    yield os.path.join(ROOT, "README.md")
    yield os.path.join(ROOT, "NOTICES.md")


def check():
    broken = []
    checked = 0
    for path in md_files():
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
        srcdir = os.path.dirname(path)
        for m in LINK_RE.finditer(text):
            target = m.group(1).strip()
            if target.startswith(SKIP_PREFIXES) or not target:
                continue
            # strip an anchor fragment
            filepart = target.split("#", 1)[0]
            if not filepart:
                continue  # was a pure #anchor
            resolved = os.path.normpath(os.path.join(srcdir, filepart))
            checked += 1
            if not os.path.exists(resolved):
                broken.append((os.path.relpath(path, ROOT), target))
    return checked, broken


def main():
    checked, broken = check()
    if broken:
        print(f"BROKEN LINKS ({len(broken)} of {checked} relative links):")
        for src, target in broken:
            print(f"  {src}: {target}")
        return 1
    print(f"OK: all {checked} relative links resolve")
    return 0


if __name__ == "__main__":
    sys.exit(main())
