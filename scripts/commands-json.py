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
    scripts/commands-json.py --out FILE     # write FILE instead

What the page gets per command: `name`, the long name that says what
the mnemonic stands for ("Residual" for RSD; only where it says more than
the mnemonic, so absent for most plain words), `description`, `stack`,
`note` (a remark shown after the description, such as a ROM bug; rare),
our `group`
(only where no ROM menu and no manual places it), `pages` (manual id to
PDF page) and `models`, one entry per model that has the command, with
the ROM's `menus`, the manual's `key` (`category`, `manual`, `page`,
`other` when it is another model's manual), the `examples` (`setup`,
`input`, `run`, `before`, `display`, `effect`, `error`, and the
`comment` of the spec it ran, joined here: scripts/example_comments.py
says how) and the `skip`
reason where none was generated, and the model's own `description`,
`stack`, `name` and `note` where the command differs there (reference.json's
`per_model`: the 28S's RND takes one argument, the 48SX's HMS+ has a
ROM bug). `before` and `display` are the stack
before and after the command as the calculator displays it, level 1
first. The typed stacks of the examples stay out: the page shows the
calculator's own display text. The key legends that place a
command no manual mentions come from the skins at run time.

The key-driven models (the 42S, 27S, 17BII, 17B, 19BII; any
`keys-<model>.json` in the folder)
come under `keyModels`, one entry per model, with no command line: per
model the manual, the keys under the menu boxes (`menuKeys`), how to
leave a menu (`leave`: its keys and the dots that show a menu is open;
without the dots, keys back to the top menu, pressed once), the keys
that type each character (`alpha`, only with `xeq`) and run a name (`xeq`), the
booted screen, and per function `long`, `description`, `note`, `stack`,
`param`, `keys` (steps `{key, label}` from the normal state) or `xeq`
(the name XEQ runs it by), `menu`, `inserts` (38G, 39G, 40G: what the
keys put on HOME's edit line), `page` (the manual's PDF page) and
the `examples` run on the emulator (`input` keys, the `then` keys after
the function's, `in_menu` when only the function's last key was pressed
in its menu, the `before` and `after` screens, upper line first, and
the spec's `comment`).
`home` (38G, 39G, 40G) are the keys pressed before every function. The format and its checks are
`saturnus-refgen keys` (crates/saturnus-refgen/src/keyed.rs); adding a
`keys-<model>.json` (and its examples) is all a model needs here.

The output is read by `scripts/commands-json.py --check` in CI, which
regenerates and compares, so the committed file never lags the data.
"""

import json
import sys
from pathlib import Path

from example_comments import DATA, match_keyed, match_rpl, spec_on

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "web" / "commands.json"
MODELS = ["48sx", "48gx", "49g", "28s"]
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
    joined = set()
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
                specs = reference.get(name, {}).get("examples", [])
                per["examples"] = [
                    with_comment({k: x[k] for k in EXAMPLE_FIELDS if k in x}, specs, i, joined, name)
                    for x, i in zip(ex, match_rpl(name, specs, ex, m))
                ]
            elif name in examples[m]["skipped"]:
                per["skip"] = examples[m]["skipped"][name]
            # The model's own text where the command differs there.
            own = reference.get(name, {}).get("per_model", {}).get(m)
            if own:
                for k in ("name", "description", "stack", "note"):
                    if own.get(k):
                        per[k] = own[k]
            entry["models"][m] = per

    lost(
        "reference.json",
        ((name, i) for name, e in reference.items() for i, x in enumerate(e.get("examples", []))
         if x.get("comment") and name in commands and any(spec_on(x, m) for m in commands[name]["models"])),
        joined,
    )

    for name, entry in commands.items():
        ref = reference.get(name)
        if ref is None:
            raise SystemExit(f"{name} has no entry in reference.json")
        if ref.get("name"):
            entry["name"] = ref["name"]
        entry["description"] = ref["description"]
        entry["stack"] = ref["stack"]
        if ref.get("note"):
            entry["note"] = ref["note"]
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


def with_comment(example, specs, i, joined, name):
    """`example` with the comment of spec `i` (None: no spec matched)."""
    if i is not None and specs[i].get("comment"):
        example["comment"] = specs[i]["comment"]
        joined.add((name, i))
    return example


def lost(where, commented, joined):
    """Fail when a comment matched no generated example: the examples are
    older than the specs (regenerate them with saturnus-refgen)."""
    missing = sorted(set(commented) - joined)
    if missing:
        names = ", ".join(f"{n} [{i}]" for n, i in missing)
        raise SystemExit(f"{where}: comments on examples that were never generated (regenerate the examples): {names}")


def key_models():
    """The key-driven models' references, from every keys-<model>.json."""
    out = {}
    for path in sorted(DATA.glob("keys-*.json")):
        model = path.stem[len("keys-"):]
        src = load(path.name)
        ex_path = DATA / f"examples-{model}.json"
        ex = load(ex_path.name) if ex_path.exists() else {"examples": {}, "booted": []}
        commands = {}
        joined = set()
        for c in src["commands"]:
            entry = {k: c[k] for k in ("long", "description", "note", "stack", "param", "keys", "xeq", "menu", "inserts", "page") if k in c}
            runs = ex["examples"].get(c["name"])
            if runs:
                specs = c.get("examples", [])
                entry["examples"] = [
                    with_comment({k: x[k] for k in ("input", "labels", "then", "in_menu", "before", "after") if k in x}, specs, i, joined, c["name"])
                    for x, i in zip(runs, match_keyed(specs, runs))
                ]
            if c["name"] in commands:
                raise SystemExit(f"{path.name}: {c['name']} is listed twice")
            commands[c["name"]] = entry
        lost(path.name, ((c["name"], i) for c in src["commands"] for i, x in enumerate(c.get("examples", [])) if x.get("comment")), joined)
        km = {
            "manual": src["manual"],
            "menuKeys": src["menu_keys"],
            "booted": ex.get("booted", []),
            "commands": commands,
        }
        if "leave" in src:
            km["leave"] = {"keys": src["leave"]["keys"]}
            box = src["leave"].get("menu_box")
            if box:
                km["leave"]["menuBox"] = {"x": box["x"], "width": box["width"], "rows": box["rows"]}
        # The characters' keys matter to the page only for running names.
        if "alpha" in src and "xeq" in src:
            km["alpha"] = src["alpha"]
        if "home" in src:
            km["home"] = src["home"]
        if "xeq" in src:
            km["xeq"] = {"start": src["xeq"]["start"], "end": src["xeq"]["end"]}
        out[model] = km
    return out


def dump(data):
    return json.dumps(data, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"


def main():
    text = dump(build())
    out = OUT
    if len(sys.argv) > 2 and sys.argv[1] == "--out":
        out = Path(sys.argv[2])
    elif len(sys.argv) > 1 and sys.argv[1] == "--check":
        path = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != text:
            print(f"{path} is not what crates/saturnus-cli/data/commands/ gives; run scripts/commands-json.py", file=sys.stderr)
            sys.exit(1)
        n = len(json.loads(text)["commands"])
        print(f"{path}: current ({n} commands)")
        return
    out.write_text(text, encoding="utf-8")
    data = json.loads(text)
    n = len(data["commands"])
    keyed = ", ".join(f"{len(km['commands'])} {m} functions" for m, km in data["keyModels"].items())
    print(f"wrote {out} ({n} commands, {keyed}, {len(text.encode()) // 1024} KB)")


if __name__ == "__main__":
    main()
