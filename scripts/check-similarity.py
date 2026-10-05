#!/usr/bin/env python3
"""Flag reference descriptions that share long word runs with HP's manuals.

Our descriptions in data/commands/reference.json are written from scratch;
this check makes sure none repeats a run of N or more consecutive words
(default 6) from the manuals' text layers. Words are compared lowercased,
letters and digits only. Exit status 1 when anything is flagged.

Usage: scripts/check-similarity.py [-n 6] PDF_OR_TEXT_DIR...

A PDF is read with pdftotext; a directory is read as one text file per page
(for manuals without a text layer, OCRed with tesseract). The manuals and
their texts stay in the literature library (~/devel/hp-literature), never in
this repository; when none of the given paths exists (CI) the check is
skipped with a message and exit status 0.
"""

import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REFERENCE = os.path.join(ROOT, "data", "commands", "reference.json")
WORD = re.compile(r"[a-z0-9]+")


def words(text):
    return WORD.findall(text.lower())


def manual_text(path):
    if os.path.isdir(path):
        parts = []
        for name in sorted(os.listdir(path)):
            if name.endswith(".txt"):
                with open(os.path.join(path, name), encoding="utf-8", errors="replace") as f:
                    parts.append(f.read())
        return "\n".join(parts)
    return subprocess.run(
        ["pdftotext", path, "-"], capture_output=True, text=True, check=True
    ).stdout


def main(argv):
    n = 6
    if len(argv) >= 2 and argv[0] == "-n":
        n = int(argv[1])
        argv = argv[2:]
    found = [p for p in argv if os.path.exists(p)]
    for p in argv:
        if p not in found:
            print(f"warning: {p} not found, not compared", file=sys.stderr)
    if not found:
        print(
            "similarity check skipped: no manual texts given or found "
            "(they live in the literature library, not in this repository)",
            file=sys.stderr,
        )
        return 0
    shingles = set()
    for path in found:
        w = words(manual_text(path))
        shingles.update(tuple(w[i : i + n]) for i in range(len(w) - n + 1))
    with open(REFERENCE, encoding="utf-8") as f:
        reference = json.load(f)["commands"]
    flagged = 0
    for name, entry in sorted(reference.items()):
        w = words(entry["description"])
        hits = {" ".join(w[i : i + n]) for i in range(len(w) - n + 1) if tuple(w[i : i + n]) in shingles}
        if hits:
            flagged += 1
            print(f"{name}: {' | '.join(sorted(hits))}")
    print(
        f"{len(reference)} descriptions checked against {len(shingles)} {n}-word runs: "
        f"{flagged} flagged",
        file=sys.stderr,
    )
    return 1 if flagged else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
