#!/usr/bin/env python3
"""Page index of HP's manuals: command name -> PDF page, for deep links.

Writes data/commands/manuals.json: the manuals (title, public URL, models)
and, per command of data/commands/<model>.json, the PDF page where each
manual describes it, so a link is `<url>#page=<n>`. Only page numbers are
stored; no manual text is kept.

Usage: scripts/manual-pages.py ID=PATH... [--ocr ID=DIR...]

ID is one of the manuals below and PATH its PDF, the public copy at the
URL below (page numbers must match the linked file). Without every manual (as in CI)
the script says so and leaves manuals.json unchanged. A manual without a
text layer needs `--ocr ID=DIR`: DIR holds one text file per page
(pNNNN.txt, from tesseract) instead.

Two ways to find a command:
- "headings": the command reference chapters of the Advanced User's
  references start each entry with a line holding just the name.
- "index": the operation index of a user's guide lists the name with a
  page label (e.g. 19-3); the labels are mapped to PDF pages by reading
  each page's own footer.
"""

import json
import os
import re
import subprocess
import sys

MANUALS = [
    {
        "id": "hp48sx-om",
        "title": "HP 48SX Owner's Manual",
        "url": "https://literature.hpcalc.org/community/hp48sx-om-en.pdf",
        "models": ["48sx"],
        "method": "index",
    },
    {
        "id": "hp48g-ug",
        "title": "HP 48G Series User's Guide",
        "url": "https://literature.hpcalc.org/community/hp48g-ug-en.pdf",
        "models": ["48gx"],
        "method": "index",
    },
    {
        "id": "hp48g-aur",
        "title": "HP 48G Series Advanced User's Reference Manual",
        "url": "https://literature.hpcalc.org/community/hp48g-aur-en.pdf",
        "models": ["48sx", "48gx"],
        "method": "headings",
    },
    {
        "id": "hp49g-aug",
        "title": "HP 49G Advanced User's Guide",
        "url": "https://literature.hpcalc.org/official/hp49g-aug-en.pdf",
        "models": ["49g"],
        "method": "index",
    },
]

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "data", "commands")
LABEL = re.compile(r"(?<![\w-])((?:[1-9][0-9]?|[A-J])-[0-9]{1,3})(?![\w-])")
INDEX_LINE = re.compile(
    r"^\s{0,14}(\S+)\s{1,}.*?(?<![\w-])((?:[1-9][0-9]?|[A-J])-[0-9]{1,3})\s*$"
)


def pdf_pages(path):
    out = subprocess.run(
        ["pdftotext", "-layout", path, "-"], capture_output=True, text=True, check=True
    ).stdout
    return out.split("\f")


def ocr_pages(directory):
    names = sorted(n for n in os.listdir(directory) if re.fullmatch(r"p\d{4}\.txt", n))
    pages = []
    for n in names:
        with open(os.path.join(directory, n), encoding="utf-8", errors="replace") as f:
            pages.append(f.read())
    return pages


def variants(name):
    """Spellings a text layer may use for a name with HP characters."""
    out = {name}
    for arrow in ("->", "—", "-"):
        for sigma in ("SIGMA", "Z", "E"):
            out.add(name.replace("→", arrow).replace("Σ", sigma))
    return out


def page_labels(pages):
    """Printed page label (e.g. 19-3) -> PDF page, from the running foot
    or head of each page: a short line that starts or ends with the label
    (the front matter's contents pages are skipped)."""
    labels = {}
    for i, text in enumerate(pages):
        if i < len(pages) // 25:
            continue
        lines = [l.strip() for l in text.splitlines() if l.strip()]
        for line in lines[:2] + lines[-3:]:
            words = line.split()
            if len(words) > 6:
                continue
            if words[0] == "Page" and len(words) > 1:
                words = words[1:]
            for w in (words[0], words[-1]):
                if LABEL.fullmatch(w):
                    labels.setdefault(w, i + 1)
    return labels


def by_index(pages, names):
    labels = page_labels(pages)
    found = {}
    # The operation index is at the back: read the last fifth only.
    start = len(pages) * 4 // 5
    for text in pages[start:]:
        for line in text.splitlines():
            m = INDEX_LINE.match(line)
            if not m:
                continue
            word, label = m.group(1), m.group(2)
            page = labels.get(label)
            if page is None:
                continue
            for name in names:
                if name not in found and word in variants(name):
                    # Keep the page only when it (or the next, where the
                    # label sits on a facing page) names the command.
                    for p in (page, page + 1):
                        if p <= len(pages) and mentions(pages[p - 1], name):
                            found[name] = p
                            break
    return found


def mentions(text, name):
    words = set(re.findall(r"\S+", text))
    stripped = {w.strip(".,;:()") for w in words}
    return any(v in words or v in stripped for v in variants(name))


def by_headings(pages, names):
    found = {}
    for i, text in enumerate(pages):
        lines = text.splitlines()
        for j, line in enumerate(lines):
            if len(line) - len(line.lstrip()) > 6:
                continue
            word = line.strip()
            # A heading is followed (within two lines) by a line naming
            # what the command is: "...  Function:", "...  Command:".
            after = " ".join(lines[j + 1 : j + 3])
            if not re.search(r"(Function|Command|Operation|Analytic)", after):
                continue
            for name in names:
                if name not in found and word in variants(name):
                    found[name] = i + 1
    # Else the running head: the first page whose top line names it.
    for i, text in enumerate(pages):
        lines = [l for l in text.splitlines() if l.strip()]
        if not lines:
            continue
        word = lines[0].strip()
        for name in names:
            if name not in found and word in variants(name):
                found[name] = i + 1
    return found


def main(argv):
    pdfs, ocr = {}, {}
    target = pdfs
    for a in argv:
        if a == "--ocr":
            target = ocr
            continue
        key, _, value = a.partition("=")
        target[key] = value
    names = set()
    for model in ("48sx", "48gx", "49g"):
        path = os.path.join(DATA, f"{model}.json")
        if os.path.exists(path):
            with open(path, encoding="utf-8") as f:
                names.update(c["name"] for c in json.load(f)["commands"])
    missing = next(
        (
            m["id"]
            for m in MANUALS
            if not os.path.exists(ocr.get(m["id"]) or pdfs.get(m["id"]) or "")
        ),
        None,
    )
    if missing:
        print(
            f"page index skipped: no {missing}=PATH (or --ocr {missing}=DIR) that exists; "
            "the manuals live in the literature library, not in this repository, and "
            "data/commands/manuals.json is left as it is",
            file=sys.stderr,
        )
        return
    pages_by = {}
    for m in MANUALS:
        mid = m["id"]
        if mid in ocr:
            pages = ocr_pages(ocr[mid])
        else:
            pages = pdf_pages(pdfs[mid])
        find = by_index if m["method"] == "index" else by_headings
        found = find(pages, sorted(names))
        print(f"{mid}: {len(found)} commands", file=sys.stderr)
        for name, page in found.items():
            pages_by.setdefault(name, {})[mid] = page
    out = {
        "manuals": [
            {k: m[k] for k in ("id", "title", "url", "models")} for m in MANUALS
        ],
        "pages": {n: dict(sorted(p.items())) for n, p in sorted(pages_by.items())},
    }
    with open(os.path.join(DATA, "manuals.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main(sys.argv[1:])
