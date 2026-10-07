#!/usr/bin/env python3
"""Generate web/commands.json, the data behind the command palette.

Folds the command reference of `data/commands/` (the ROM catalogs with
their menus, our descriptions and stack effects, the examples generated on
the emulator, the manuals' placements and page numbers) into the one file
the page loads lazily when the palette or the Commands tab opens. The
page's lookup rules (`web/reference.js`) are those of `saturnus ref`
(`crates/saturnus-cli/src/reference.rs`); this file only carries the data.

    scripts/commands-json.py                # write web/commands.json
    scripts/commands-json.py --check [FILE] # the committed file is current

What the page gets per command: `description`, `stack`, our `group`
(only where no ROM menu and no manual places it), `pages` (manual id to
PDF page) and `models`, one entry per model that has the command, with
the ROM's `menus`, the manual's `key` (`category`, `manual`, `page`,
`other` when it is another model's manual), the `examples` (`setup`,
`input`, `run`, `display`, `effect`, `error`) and the `skip` reason where
none was generated. The typed stacks of the examples stay out: the page
shows the calculator's own display text. The key legends that place a
command no manual mentions come from the skins at run time.

The output is read by `scripts/commands-json.py --check` in CI, which
regenerates and compares, so the committed file never lags the data.
"""

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DATA = REPO / "data" / "commands"
OUT = REPO / "web" / "commands.json"
MODELS = ["48sx", "48gx", "49g"]
EXAMPLE_FIELDS = ["setup", "input", "run", "display", "effect", "error"]


def load(name):
    with open(DATA / name, encoding="utf-8") as f:
        return json.load(f)


def build():
    catalogs = {m: load(f"{m}.json") for m in MODELS}
    examples = {m: load(f"examples-{m}.json") for m in MODELS}
    reference = load("reference.json")["commands"]
    manuals = load("manuals.json")
    categories = load("categories.json")["commands"]

    commands = {}
    for m in MODELS:
        for c in catalogs[m]["commands"]:
            name = c["name"]
            entry = commands.setdefault(name, {"models": {}})
            per = {}
            if c.get("menus"):
                per["menus"] = list(c["menus"])
            placed = categories.get(name, {}).get(m)
            if placed and placed.get("category") and placed.get("manual"):
                key = {
                    "category": placed["category"],
                    "manual": placed["manual"],
                    "page": placed.get("page"),
                }
                if placed.get("other_model"):
                    key["other"] = True
                per["key"] = key
            ex = examples[m]["examples"].get(name)
            if ex:
                per["examples"] = [
                    {k: x[k] for k in EXAMPLE_FIELDS if k in x} for x in ex
                ]
            elif name in examples[m]["skipped"]:
                per["skip"] = examples[m]["skipped"][name]
            entry["models"][m] = per

    for name, entry in commands.items():
        ref = reference.get(name)
        if ref is None:
            raise SystemExit(f"{name} has no entry in reference.json")
        entry["description"] = ref["description"]
        entry["stack"] = ref["stack"]
        if ref.get("category"):
            entry["group"] = ref["category"]
        pages = manuals["pages"].get(name)
        if pages:
            entry["pages"] = pages

    return {
        "method": (
            "scripts/commands-json.py from data/commands/: the ROMs' command "
            "names and menus (saturnus-refgen), our descriptions and stack "
            "effects, the examples run on the emulator, the manuals' "
            "placements and page numbers (scripts/manual-categories.py, "
            "scripts/manual-pages.py)."
        ),
        "models": MODELS,
        "menuKeys": {m: [k["key"] for k in catalogs[m]["menu_keys"]] for m in MODELS},
        "manuals": {
            x["id"]: {"title": x["title"], "url": x["url"], "models": x["models"]}
            for x in manuals["manuals"]
        },
        "commands": dict(sorted(commands.items())),
    }


def dump(data):
    return json.dumps(data, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"


def main():
    text = dump(build())
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        path = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != text:
            print(f"{path} is not what data/commands/ gives; run scripts/commands-json.py", file=sys.stderr)
            sys.exit(1)
        n = len(json.loads(text)["commands"])
        print(f"{path}: current ({n} commands)")
        return
    OUT.write_text(text, encoding="utf-8")
    n = len(json.loads(text)["commands"])
    print(f"wrote {OUT} ({n} commands, {len(text.encode()) // 1024} KB)")


if __name__ == "__main__":
    main()
