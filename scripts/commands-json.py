#!/usr/bin/env python3
"""Generate web/commands.json, the data behind the command palette.

Folds the command reference of `crates/saturnus-cli/data/commands/` (the ROM catalogs with
their menus, our descriptions and stack effects, the examples generated on
the emulator, the manuals' placements and page numbers) into the one file
the page loads lazily when the palette or the Commands tab opens. The
page's lookup rules (`web/reference.js`) are those of `saturnus ref`
(`crates/saturnus-cli/src/reference.rs`); this file only carries the data.

    scripts/commands-json.py                # write web/commands.json
    scripts/commands-json.py --check [FILE] # the committed file is current

What the page gets per command: `name`, the long name that says what
the mnemonic stands for ("Residual" for RSD; only where it says more than
the mnemonic, so absent for most plain words), `description`, `stack`,
our `group`
(only where no ROM menu and no manual places it), `pages` (manual id to
PDF page) and `models`, one entry per model that has the command, with
the ROM's `menus`, the manual's `key` (`category`, `manual`, `page`,
`other` when it is another model's manual), the `examples` (`setup`,
`input`, `run`, `before`, `display`, `effect`, `error`) and the `skip`
reason where none was generated. `before` and `display` are the stack
before and after the command as the calculator displays it, level 1
first. The typed stacks of the examples stay out: the page shows the
calculator's own display text. The key legends that place a
command no manual mentions come from the skins at run time.

The key-driven models (the 42S; any `keys-<model>.json` in the folder)
come under `keyModels`, one entry per model, with no command line: per
model the manual, the keys under the menu boxes (`menuKeys`), how to
leave a menu (`leave`: its keys and the dots that show a menu is open),
the keys that type each character (`alpha`) and run a name (`xeq`), the
booted screen, and per function `long`, `description`, `stack`,
`param`, `keys` (steps `{key, label}` from the normal state) or `xeq`
(the name XEQ runs it by), `menu`, `page` (the manual's PDF page) and
the `examples` run on the emulator (`input` keys, the `before` and
`after` screens, upper line first). The format and its checks are
`saturnus-refgen keys` (crates/saturnus-refgen/src/keyed.rs); adding a
`keys-<model>.json` (and its examples) is all a model needs here.

The output is read by `scripts/commands-json.py --check` in CI, which
regenerates and compares, so the committed file never lags the data.
"""

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DATA = REPO / "crates" / "saturnus-cli" / "data" / "commands"
OUT = REPO / "web" / "commands.json"
MODELS = ["48sx", "48gx", "49g"]
EXAMPLE_FIELDS = ["setup", "input", "run", "before", "display", "effect", "error"]


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
        if ref.get("name"):
            entry["name"] = ref["name"]
        entry["description"] = ref["description"]
        entry["stack"] = ref["stack"]
        if ref.get("category"):
            entry["group"] = ref["category"]
        pages = manuals["pages"].get(name)
        if pages:
            entry["pages"] = pages

    return {
        "method": (
            "scripts/commands-json.py from crates/saturnus-cli/data/commands/: the ROMs' command "
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
        "keyModels": key_models(),
    }


def key_models():
    """The key-driven models' references, from every keys-<model>.json."""
    out = {}
    for path in sorted(DATA.glob("keys-*.json")):
        model = path.stem[len("keys-"):]
        src = load(path.name)
        ex_path = DATA / f"examples-{model}.json"
        ex = load(ex_path.name) if ex_path.exists() else {"examples": {}, "booted": []}
        commands = {}
        for c in src["commands"]:
            entry = {k: c[k] for k in ("long", "description", "stack", "param", "keys", "xeq", "menu", "page") if k in c}
            runs = ex["examples"].get(c["name"])
            if runs:
                entry["examples"] = [{k: x[k] for k in ("input", "before", "after")} for x in runs]
            if c["name"] in commands:
                raise SystemExit(f"{path.name}: {c['name']} is listed twice")
            commands[c["name"]] = entry
        km = {
            "manual": src["manual"],
            "menuKeys": src["menu_keys"],
            "booted": ex.get("booted", []),
            "commands": commands,
        }
        if "leave" in src:
            box = src["leave"]["menu_box"]
            km["leave"] = {"keys": src["leave"]["keys"], "menuBox": {"x": box["x"], "width": box["width"], "rows": box["rows"]}}
        if "alpha" in src:
            km["alpha"] = src["alpha"]
        if "xeq" in src:
            km["xeq"] = {"start": src["xeq"]["start"], "end": src["xeq"]["end"]}
        out[model] = km
    return out


def dump(data):
    return json.dumps(data, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"


def main():
    text = dump(build())
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        path = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != text:
            print(f"{path} is not what crates/saturnus-cli/data/commands/ gives; run scripts/commands-json.py", file=sys.stderr)
            sys.exit(1)
        n = len(json.loads(text)["commands"])
        print(f"{path}: current ({n} commands)")
        return
    OUT.write_text(text, encoding="utf-8")
    data = json.loads(text)
    n = len(data["commands"])
    keyed = ", ".join(f"{len(km['commands'])} {m} functions" for m, km in data["keyModels"].items())
    print(f"wrote {OUT} ({n} commands, {keyed}, {len(text.encode()) // 1024} KB)")


if __name__ == "__main__":
    main()
