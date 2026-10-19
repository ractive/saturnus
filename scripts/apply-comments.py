#!/usr/bin/env python3
"""Write the comments of sidecar files into the reference's example specs.

    scripts/apply-comments.py FILE...

Each FILE is a sidecar (scripts/example_comments.py has the format,
crates/saturnus-cli/data/commands/comments/ keeps them):

    {"group": "rpl", "comments": {"HMS+": [{"i": 0, "comment": "1h30m plus 2h45m gives 4h15m"}]}}

`rpl` writes into reference.json (`commands.<name>.examples[i].comment`),
a key-driven model's id into keys-<model>.json (39g also into 40g, whose
file equals the 39G's). `i` is the example's index as
scripts/list-examples.py prints it. A comment of null or "" removes one.
Nothing is written when any command or index does not exist, or a comment
breaks the rules scripts/check-comments.py checks (its numbers are not
checked here). Running it again changes nothing.
"""

import json
import sys

import example_comments as ec


def write_reference(ref):
    text = json.dumps(ref, indent=2, ensure_ascii=False) + "\n"
    path = ec.DATA / "reference.json"
    if path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def write_keyed(model, updates):
    """Rewrite the lines of the commands in `updates` ({name: command}) in
    keys-<model>.json, one command per line as the file keeps them."""
    path = ec.DATA / f"keys-{model}.json"
    lines = path.read_text(encoding="utf-8").split("\n")
    inside = False
    done = set()
    for k, line in enumerate(lines):
        if line.startswith('  "commands": ['):
            inside = True
            continue
        if not inside or not line.lstrip().startswith('{"name"'):
            continue
        body = line.strip()
        comma = body.endswith(",")
        cmd = json.loads(body.rstrip(","))
        if cmd["name"] in updates:
            indent = line[: len(line) - len(line.lstrip())]
            lines[k] = indent + json.dumps(updates[cmd["name"]], ensure_ascii=False) + ("," if comma else "")
            done.add(cmd["name"])
    if done != set(updates):
        raise SystemExit(f"{path.name}: no line for {sorted(set(updates) - done)}")
    text = "\n".join(lines)
    json.loads(text)
    if path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return 0 if argv else 2
    sidecars = []
    errors = []
    for path in argv:
        try:
            sidecars.append(ec.read_sidecar(path))
        except (ValueError, OSError, json.JSONDecodeError) as e:
            errors.append(str(e))
    ref = ec.load("reference.json")
    keyed = {}
    changed_cmds = {}
    for (group, items), path in zip(sidecars, argv):
        if group != "rpl" and group not in keyed:
            keyed[group] = ec.load(f"keys-{group}.json")
        cmds = ref["commands"] if group == "rpl" else {c["name"]: c for c in keyed[group]["commands"]}
        for name, i, comment in items:
            where = f"{path}: {name} [{i}]"
            cmd = cmds.get(name)
            if cmd is None:
                errors.append(f"{where}: no command {name!r} in {group}")
                continue
            specs = cmd.get("examples", [])
            if not 0 <= i < len(specs):
                errors.append(f"{where}: no example {i} ({name} has {len(specs)})")
                continue
            if comment:
                errors.extend(f"{where}: {p}" for p in ec.problems(comment))
                specs[i]["comment"] = comment
            else:
                specs[i].pop("comment", None)
            if group != "rpl":
                changed_cmds.setdefault(group, {})[name] = cmd
    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        print(f"nothing written: {len(errors)} problem(s)", file=sys.stderr)
        return 1
    written = []
    if any(g == "rpl" for g, _ in sidecars) and write_reference(ref):
        written.append("reference.json")
    for group, updates in changed_cmds.items():
        for model in [group] + ec.MIRRORS.get(group, []):
            if write_keyed(model, updates):
                written.append(f"keys-{model}.json")
    n = sum(len(items) for _, items in sidecars)
    print(f"{n} comment(s) applied; " + (f"wrote {', '.join(written)}" if written else "nothing changed"))
    if written:
        print("then: scripts/commands-json.py (web/commands.json)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
