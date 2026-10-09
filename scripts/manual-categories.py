#!/usr/bin/env python3
"""Categories of the commands, from the manuals' own statements of where a
command is found.

Writes crates/saturnus-cli/data/commands/categories.json: per command and model, the key or
menu the manual names for it (a short fact such as `MTH` or
`Arithmetic`) with the manual and the PDF page of the
statement, and the ROM's own menus from the catalogs (`menus`, written
by `saturnus-refgen menus`). No manual prose is kept.

Usage: scripts/manual-categories.py [--text-dir DIR]

The texts are those of the knowledge base: DIR, else $CALCULATOR_KB_TEXT,
else ~/devel/calculator-knowledgebase/raw/manuals/text (one file per
manual, `#` header lines, then pages separated by form feeds; see that
repository's sources/manifest.json). Without them (as in CI) the script
says so, exits 0 and leaves categories.json unchanged.

Which manual places commands for which model:
- 48SX: the owner's manual's operation index (appendix G): each row names
  the command and, on the line below, the keys that reach it, e.g.
  `C (MTH) PARTS ABS`.
- 48GX: the 48G Advanced User's Reference: each command entry has a
  "Keyboard Access:" line with the keys.
- 49G: the 49G Advanced User's Guide, chapter 14 (the computer algebra
  commands only): each entry has an "Access:" line naming the CAS
  category and the submenu, e.g. `Arithmetic, ... POLYNOMIAL`; the
  category is kept, the submenu (often misread) is not.

The keys are scanned text: the hardware keys (in parentheses) read well,
the menu labels mostly do not. So a category from the 48 manuals is the
first menu key named (`MTH`, `PRG`, `MEMORY`, ...), or `Keyboard` when the
key is the command itself (`SIN`); OCR variants of a key name are mapped
to the spelling the manuals use most often. A 48SX command that the
owner's manual does not place takes the 48G AUR's statement, marked with
that manual (the keyboards share their menu keys). The 49G takes no
statement from the 48 manuals: its keyboard differs (no TIME or MTH BASE
keys), and half of the AUR's statements did not match the 49G ROM's
menus. The CLI falls back to our own category in reference.json after
that, and shows none when there is none.
"""

import difflib
import json
import os
import re
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "crates", "saturnus-cli", "data", "commands")
MODELS = ("48sx", "48gx", "49g")
TEXTS = {
    "hp48sx-om": "hp48sx-om-en.txt",
    "hp48g-aur": "hp48g-aur-en.txt",
    "hp49g-aug": "hp49g-aug-en.txt",
}
# The manual that places a model's commands, then the fallback.
ORDER = {
    "48sx": ["hp48sx-om", "hp48g-aur"],
    "48gx": ["hp48g-aur"],
    "49g": ["hp49g-aug"],
}
KEYBOARD = "Keyboard"


def pages_of(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read().split("\f")[1:]


def variants(name):
    """Spellings a scanned text may use for a name with HP characters."""
    out = {name}
    for arrow in ("->", "—", "-"):
        for sigma in ("SIGMA", "Z", "E"):
            out.add(name.replace("→", arrow).replace("Σ", sigma))
    return out


def key_tokens(text):
    """The key names in `text` in reading order: parenthesized ones
    (uppercase letters and `/` kept), and bare uppercase words, which only
    count when they are a known key name exactly (the scan sometimes loses
    the parentheses)."""
    out = []
    for m in re.finditer(r"\(([^()]{2,12})\)|\b([A-Z][A-Z/]{1,9})\b", text):
        if m.group(1) is not None:
            token = re.sub(r"[^A-Za-z/]", "", m.group(1)).upper()
            if len(token) >= 2:
                out.append(token)
        else:
            out.append("=" + m.group(2))
    return out


# Keys that move around the keyboard or the menu, never a category.
NAVIGATION = {"NXT", "PREV", "UP", "DOWN", "LEFT", "RIGHT", "ENTER", "SPC", "DEL", "ON",
              "ALPHA", "EVAL", "CST", "VAR", "REVIEW", "HOME", "CANCEL", "OK"}


class Keys:
    """Maps scanned key names to the spelling the manuals use most: the
    parenthesized names that occur at least `minimum` times."""

    def __init__(self, tokens, minimum=4):
        counts = Counter(t for t in tokens if not t.startswith("="))
        # Most frequent first; a rarer spelling close to a more frequent
        # one is a misreading of it (TME for TIME, UNTS for UNITS).
        self.vocab = []
        self.alias = {}
        for t, n in counts.most_common():
            if n < minimum or len(t) < 3:
                continue
            close = [
                v
                for v in difflib.get_close_matches(t, self.vocab, n=3, cutoff=0.6)
                if n * 4 <= counts[v]
            ]
            if close:
                self.alias[t] = close[0]
            else:
                self.vocab.append(t)

    def canon(self, token):
        if token.startswith("="):
            word = token[1:]
            return word if word in self.vocab else None
        if token in self.vocab:
            return token
        if token in self.alias:
            return self.alias[token]
        close = difflib.get_close_matches(token, self.vocab, n=1, cutoff=0.75)
        return close[0] if close else None


def key_category(name, tokens, keys, commands):
    """The category the key list states for `name`: the first key that is
    a menu key, `Keyboard` when the first key is the command itself."""
    for token in tokens:
        if not token.startswith("=") and token in {v.upper() for v in variants(name)}:
            # The key is the command itself (SIN, ASIN on its shift),
            # unless the key opens a menu of that name (TIME).
            return token if token in MENU_KEYS_THAT_ARE_COMMANDS else KEYBOARD
        key = keys.canon(token)
        if key is None or key in NAVIGATION:
            continue
        if key in variants(name) or (key in commands and key == name.upper()):
            return key if key in MENU_KEYS_THAT_ARE_COMMANDS else KEYBOARD
        if key in commands and key not in MENU_KEYS_THAT_ARE_COMMANDS:
            # Another command's key (a shifted function on it).
            return KEYBOARD
        return key
    return None


# Keys whose label is a menu even though a command has the same name.
MENU_KEYS_THAT_ARE_COMMANDS = {"TIME", "UNITS", "PLOT", "SOLVE", "STAT", "MODES", "MATRIX"}


def sx_rows(pages):
    """48SX owner's manual, operation index: rows of (name, page, key
    tokens). A row is a description line (the name in the first 13
    columns, blank where the index shows a key label the scan lost),
    continued by more description lines and then the key lines, which
    start with the operation type (C, O, U, F, A). Rows without a name
    are kept so their keys do not stick to the row before."""
    rows = []
    current = None
    after_keys = False
    for i, text in enumerate(pages):
        if "Operation Index" not in text[-300:] or i < len(pages) // 2:
            continue
        for line in text.splitlines():
            if not line.strip() or "Operation Index" in line or "Description" in line:
                continue
            if re.match(r"\s{8,}[CAOUF0](\s|$)", line) and current is not None:
                if after_keys:
                    # A second key line: the keys of a row whose
                    # description the scan put elsewhere.
                    current = ["", i + 1, []]
                    rows.append(current)
                current[2].extend(key_tokens(line))
                after_keys = True
                continue
            head = line[:13].strip()
            if head or after_keys or current is None:
                current = [head, i + 1, []]
                rows.append(current)
            after_keys = False
    # A one-character name is a key label the scan misread (the index
    # shows labels such as the angle sign as pictures).
    return [r for r in rows if r[0] and (len(r[0]) > 1 or r[0].isalnum())]


def place_rows(rows, names, commands, keys):
    found = {}
    for word, page, tokens in rows:
        for name in names:
            if name not in found and word in variants(name):
                cat = key_category(name, tokens, keys, commands)
                if cat:
                    found[name] = (cat, page)
    return found


def aur_rows(pages):
    """48G AUR: each entry's "Keyboard Access:" line, as (name, page,
    key tokens); entries that must be typed in have no keys."""
    entries = []  # (name, page, access text)
    for i, text in enumerate(pages):
        lines = text.splitlines()
        for j, line in enumerate(lines):
            if len(line) - len(line.lstrip()) > 6:
                continue
            word = line.strip()
            after = " ".join(lines[j + 1 : j + 3])
            if not word or not re.search(r"(Function|Command|Operation|Analytic)", after):
                continue
            # The access line follows within this page or the next.
            rest = "\n".join(lines[j + 1 :]) + "\n" + (pages[i + 1] if i + 1 < len(pages) else "")
            m = re.search(r"Keyboard Access:(.*?)(Affected by|Remarks:|$)", rest, re.S)
            if m:
                access = m.group(1)
                typed = re.search(r"None\.\s*Must be typed", access)
                entries.append([word, i + 1, [] if typed else key_tokens(access)])
    return entries


def aug(pages, names, commands):
    """49G AUG chapter 14: each entry's "Access:" line."""
    start = next((i for i, p in enumerate(pages) if "Alphabetical command list" in p), None)
    if start is None:
        return {}
    heads = []  # (name, page index, line index)
    for i in range(start, len(pages)):
        lines = pages[i].splitlines()
        for j, line in enumerate(lines):
            word = line.strip()
            nxt = next((l.strip() for l in lines[j + 1 : j + 4] if l.strip()), "")
            if word and nxt.startswith("Type:"):
                heads.append((word, i, j))
    lead = Counter()
    accesses = []
    for k, (word, i, j) in enumerate(heads):
        stop = heads[k + 1] if k + 1 < len(heads) else (None, len(pages), 0)
        block = []
        for pi in range(i, min(stop[1] + 1, len(pages))):
            lines = pages[pi].splitlines()
            lo = j + 1 if pi == i else 0
            hi = stop[2] if pi == stop[1] else len(lines)
            block.extend((pi, l) for l in lines[lo:hi])
        for pi, l in block:
            # Only an "Access:" line with its value on the same line: on
            # pages where the scan split labels and values into columns
            # the value cannot be told apart from a neighbour's.
            m = re.match(r"\s*Access:\s*([A-Z][a-z]+(?: [&a-z]+)?(?: [A-Z][a-z]+)?),\s*(.*)$", l)
            if m:
                lead[m.group(1)] += 1
                accesses.append((word, pi + 1, m.group(1), m.group(2)))
                break
    found = {}
    for word, page, first, _ in accesses:
        # The CAS category; the submenu after the keys is too often
        # misread to keep.
        if lead[first] < 2:
            continue
        cat = first
        for name in names:
            if name not in found and word in variants(name):
                found[name] = (cat, page)
    return found


def main(argv):
    text_dir = os.environ.get("CALCULATOR_KB_TEXT") or os.path.expanduser(
        "~/devel/calculator-knowledgebase/raw/manuals/text"
    )
    it = iter(argv)
    for a in it:
        if a == "--text-dir":
            text_dir = next(it, "")
        else:
            sys.exit(f"unknown argument {a!r}\n{__doc__}")
    paths = {m: os.path.join(text_dir, f) for m, f in TEXTS.items()}
    missing = [p for p in paths.values() if not os.path.exists(p)]
    if missing:
        print(
            f"categories skipped: {', '.join(missing)} not found; the manuals live in the "
            "knowledge base's raw/ (--text-dir, $CALCULATOR_KB_TEXT), not in this repository; "
            "crates/saturnus-cli/data/commands/categories.json is left as it is",
            file=sys.stderr,
        )
        return
    catalogs = {}
    rom_menus = {}
    for model in MODELS:
        with open(os.path.join(DATA, f"{model}.json"), encoding="utf-8") as f:
            commands = json.load(f)["commands"]
        catalogs[model] = [c["name"] for c in commands]
        rom_menus[model] = {c["name"]: c["menus"] for c in commands if c.get("menus")}
    every = sorted({n for names in catalogs.values() for n in names})
    commands = {n.upper() for n in every}
    sx = sx_rows(pages_of(paths["hp48sx-om"]))
    gx = aur_rows(pages_of(paths["hp48g-aur"]))
    keys = Keys([t for r in sx + gx for t in r[2]])
    found = {
        "hp48sx-om": place_rows(sx, every, commands, keys),
        "hp48g-aur": place_rows(gx, every, commands, keys),
        "hp49g-aug": aug(pages_of(paths["hp49g-aug"]), every, commands),
    }
    for manual, f in found.items():
        print(f"{manual}: places {len(f)} commands", file=sys.stderr)
    out = {}
    for model in MODELS:
        for name in catalogs[model]:
            for manual in ORDER[model]:
                if name in found[manual]:
                    cat, page = found[manual][name]
                    placed = {"category": cat, "manual": manual, "page": page}
                    if manual != ORDER[model][0]:
                        # Another model's manual: its keys may differ.
                        placed["other_model"] = True
                    out.setdefault(name, {})[model] = placed
                    break
    # The ROM's own menus (saturnus-refgen menus, in the catalogs) next to
    # the manuals' statements.
    for model in MODELS:
        for name, menus in rom_menus[model].items():
            out.setdefault(name, {}).setdefault(model, {})["menus"] = menus
    doc = {
        "method": "scripts/manual-categories.py: the key or menu each manual names for a "
        "command (48SX owner's manual operation index, 48G AUR Keyboard Access lines, 49G "
        "AUG Access lines), page = the PDF page of that statement; menus = the ROM's own "
        "menus that offer it (saturnus-refgen menus, copied from the catalogs)",
        # Every level sorted, as saturnus-refgen writes it too.
        "commands": {
            n: {m: dict(sorted(e.items())) for m, e in sorted(v.items())}
            for n, v in sorted(out.items())
        },
    }
    with open(os.path.join(DATA, "categories.json"), "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main(sys.argv[1:])
