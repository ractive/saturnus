#!/usr/bin/env python3
"""Generate web/flags.json, the data behind the flags panel.

Reads the system flag tables of the hardware wiki (~/devel/hp-literature,
or the directory given as the first argument) through `hyalo`: one page per
model family, each with a single table

    | Flags | Topic | Name | Clear | Set | Status | Source |

where Flags is `-1` or a range `-5..-10` (a multi-flag field whose encoding
is in the Clear column, Set `-`). The wiki stays outside this repository;
the JSON is committed and refreshed by running this script:

    scripts/flags-json.py [WIKI_DIR]
    scripts/flags-json.py --check [FILE]   # validate the committed JSON

The wiki's Source column (citations) is not shipped; each model's `basis`
sentence, written here, names the guides by title. Every system flag of a
model must be covered exactly once, statuses and topics come from the fixed
sets below, and no output string may hold wiki link syntax, a citation, a
local path or an e-mail address.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

# Local paths (home, user or devel directories, the wiki's raw/ tree,
# Windows drives) and e-mail addresses: none may reach the public page.
PRIVATE = re.compile(
    r"(^|[\s(\"'])~|/Users/|/home/|\bdevel/|(^|[\s(])raw/|\b[A-Za-z]:\\|"
    r"[\w.+-]+@[\w-]+\.[\w.]+"
)
# Wiki link syntax and citations stay in the wiki.
WIKI_SYNTAX = re.compile(r"\[\[|\]\]|\(src:|\bsources/")

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "web" / "flags.json"

# Display order of the panel's groups.
TOPICS = [
    "Math",
    "CAS",
    "Angle and coordinates",
    "Binary integers",
    "Number display",
    "Display",
    "Errors and exceptions",
    "Plotting",
    "I/O and printing",
    "Time and alarms",
    "Keyboard and entry",
    "System",
    "Other",
]
STATUSES = ["known", "unused", "unknown"]
COLUMNS = ["Flags", "Topic", "Name", "Clear", "Set", "Status", "Source"]

MODELS = [
    {
        "key": "48sx",
        "family": "HP 48S/SX",
        "page": "hardware/system-flags-48sx.md",
        "systemFlags": 64,
        "userFlags": 64,
        "basis": "Meanings from the system flag appendix of the HP 48SX Owner's Manual.",
    },
    {
        "key": "48gx",
        "family": "HP 48G/GX",
        "page": "hardware/system-flags-48gx.md",
        "systemFlags": 64,
        "userFlags": 64,
        "basis": "Meanings from the system flag appendix of the HP 48G Series User's Guide, "
        "checked against the HP 48G Series Advanced User's Reference Manual.",
    },
    {
        "key": "49g",
        "family": "HP 49G",
        "page": "hardware/system-flags-49g.md",
        "systemFlags": 128,
        "userFlags": 128,
        "basis": "The HP 49G User's Manual and Advanced User's Guide do not list the system "
        "flags (they refer to the HP 49G Pocket Guide, which is not among this project's "
        "sources). Only the flags those two guides mention are described; the others are "
        "shown without a meaning.",
    },
]

FLAGS_CELL = re.compile(r"^-(\d+)(?:\.\.-(\d+))?$")


def fail(msg):
    print(f"flags-json: {msg}", file=sys.stderr)
    sys.exit(1)


def hyalo_body(wiki, page):
    out = subprocess.run(
        ["hyalo", "read", "--file", page, "--format", "json"],
        cwd=wiki,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)["results"]["content"]


def parse_table(page, body):
    """The rows of the page's one flags table, as lists of cell strings."""
    lines = [ln.strip() for ln in body.splitlines()]
    tables, cur = [], []
    for ln in lines:
        if ln.startswith("|"):
            cur.append(ln)
        elif cur:
            tables.append(cur)
            cur = []
    if cur:
        tables.append(cur)
    flag_tables = [t for t in tables if cells(t[0]) == COLUMNS]
    if len(flag_tables) != 1:
        fail(f"{page}: expected exactly one table with columns {COLUMNS}, found {len(flag_tables)}")
    table = flag_tables[0]
    if not re.fullmatch(r"\|(\s*:?-{3,}:?\s*\|)+", table[1].replace(" ", "")):
        fail(f"{page}: table has no separator row")
    rows = [cells(r) for r in table[2:]]
    for r in rows:
        if len(r) != len(COLUMNS):
            fail(f"{page}: row has {len(r)} cells, expected {len(COLUMNS)}: {r}")
    return rows


def cells(line):
    return [c.strip() for c in line.strip().strip("|").split("|")]


def entry_from_row(page, row):
    flags, topic, name, clear, set_, status, _source = row
    m = FLAGS_CELL.match(flags)
    if not m:
        fail(f"{page}: bad Flags cell {flags!r}")
    first = -int(m.group(1))
    last = -int(m.group(2)) if m.group(2) else first
    if last > first:
        fail(f"{page}: range {flags} runs the wrong way")
    if topic not in TOPICS:
        fail(f"{page}: {flags}: topic {topic!r} not in {TOPICS}")
    if status not in STATUSES:
        fail(f"{page}: {flags}: status {status!r} not in {STATUSES}")
    if not name or name == "-":
        fail(f"{page}: {flags}: empty name")
    clear = None if clear == "-" else clear
    set_ = None if set_ == "-" else set_
    entry = {"first": first, "last": last, "topic": topic, "name": name}
    if first != last:
        if set_ is not None:
            fail(f"{page}: {flags}: a range keeps its encoding in Clear; Set must be '-'")
        if clear is not None:
            entry["field"] = clear
        elif status == "known":
            fail(f"{page}: {flags}: a {status} range needs its encoding in Clear")
    else:
        if status == "known" and (clear is None or set_ is None):
            fail(f"{page}: {flags}: a {status} flag needs both Clear and Set")
        if clear is not None:
            entry["clear"] = clear
        if set_ is not None:
            entry["set"] = set_
    entry["status"] = status
    return entry


def strings(obj, path="$"):
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield from strings(v, f"{path}.{k}")
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            yield from strings(v, f"{path}[{i}]")
    elif isinstance(obj, str):
        yield path, obj


def validate(doc):
    """Exit with messages if `doc` is not a valid flags.json."""
    errors = []
    for path, value in strings(doc):
        if PRIVATE.search(value):
            errors.append(f"private-looking value at {path}: {value!r}")
        if WIKI_SYNTAX.search(value):
            errors.append(f"wiki link or citation at {path}: {value!r}")
    topics = doc.get("topics")
    if topics != TOPICS:
        errors.append(f"topics are {topics!r}, expected {TOPICS!r}")
    models = doc.get("models", {})
    if sorted(models) != sorted(m["key"] for m in MODELS):
        errors.append(f"models are {sorted(models)}")
    for key, model in models.items():
        n = model.get("systemFlags")
        if not isinstance(n, int) or not isinstance(model.get("userFlags"), int):
            errors.append(f"{key}: systemFlags and userFlags must be integers")
            continue
        for k in ("family", "basis"):
            if not isinstance(model.get(k), str) or not model[k]:
                errors.append(f"{key}: missing {k}")
        seen = {}
        prev = 0
        for e in model.get("system", []):
            first, last = e.get("first"), e.get("last")
            if not (isinstance(first, int) and isinstance(last, int) and 0 > first >= last):
                errors.append(f"{key}: bad range {first}..{last}")
                continue
            if first >= prev:
                errors.append(f"{key}: entries not sorted from -1 downward at {first}")
            prev = first
            if e.get("topic") not in TOPICS:
                errors.append(f"{key}: {first}: topic {e.get('topic')!r}")
            if e.get("status") not in STATUSES:
                errors.append(f"{key}: {first}: status {e.get('status')!r}")
            for f in range(first, last - 1, -1):
                seen[f] = seen.get(f, 0) + 1
        for f in range(-1, -n - 1, -1):
            if seen.get(f, 0) != 1:
                errors.append(f"{key}: flag {f} covered {seen.get(f, 0)} times")
        extra = [f for f in seen if f < -n]
        if extra:
            errors.append(f"{key}: flags beyond -{n}: {sorted(extra, reverse=True)}")
    for e in errors:
        print(f"flags.json: {e}", file=sys.stderr)
    if errors:
        sys.exit(1)


def counts(model):
    c = {s: 0 for s in STATUSES}
    for e in model["system"]:
        c[e["status"]] += e["first"] - e["last"] + 1
    return c


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        target = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        validate(json.loads(target.read_text(encoding="utf-8")))
        print(f"{target}: valid, no private values")
        return
    wiki = Path(sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/devel/hp-literature"))
    models = {}
    for m in MODELS:
        rows = parse_table(m["page"], hyalo_body(wiki, m["page"]))
        system = sorted((entry_from_row(m["page"], r) for r in rows), key=lambda e: -e["first"])
        models[m["key"]] = {
            "family": m["family"],
            "basis": m["basis"],
            "systemFlags": m["systemFlags"],
            "userFlags": m["userFlags"],
            "system": system,
        }
    doc = {"generator": "scripts/flags-json.py", "topics": TOPICS, "models": models}
    validate(doc)
    OUT.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    for key, model in models.items():
        c = counts(model)
        summary = ", ".join(f"{v} {k}" for k, v in c.items() if v)
        print(f"{OUT.relative_to(REPO)}: {key}: {len(model['system'])} entries ({summary})")


if __name__ == "__main__":
    main()
