#!/usr/bin/env python3
"""List the reference's examples as plain text, for writing comments.

    scripts/list-examples.py GROUP [--from NAME] [--count N]

GROUP is `rpl` (reference.json: the 48SX, 48GX, 49G and 28S) or a
key-driven model's id (42s, 27s, 17bii, 17b, 19bii, 38g, 39g; the 40G's
are the 39G's). Lists the commands with examples in name order: per
command its name, long name, description and note, then per example its
index (the `i` of a sidecar, scripts/example_comments.py), what is
typed or pressed, and what the calculator showed: for `rpl` the stack
after on each model that ran it (level 1 last, deepest first; models
that agree on one line), for a key-driven model the display before and
after (lines joined by " / "). An existing comment follows as `# ...`.

`--from NAME` starts at that command (or the first after it in name
order), `--count N` lists N commands; the last line says where to go on.
"""

import argparse
import sys

import example_comments as ec


def stack_text(x):
    if x.get("error"):
        return f"Error: {x['error']}"
    lines = list(reversed(x.get("display", [])))
    return "  ".join(lines) if lines else "(empty)"


def screen(lines):
    shown = [l.strip() for l in lines if l.strip()]
    return " / ".join(shown) if shown else "(blank)"


def steps_text(steps):
    return " ".join(s.get("label") or ("⇧" if s["key"] in ("shift", "leftshift", "rightshift") else s["key"].upper()) for s in steps)


def head(name, long, description, note):
    out = name + (f" ({long})" if long else "") + f": {description}"
    if note:
        out += f"\n  note: {note}"
    return out


def rpl(names):
    ref = ec.load("reference.json")["commands"]
    gen = ec.rpl_examples()
    blocks = {}
    for name in names:
        e = ref[name]
        specs = e.get("examples", [])
        lines = [head(name, e.get("name"), e["description"], e.get("note"))]
        for i, s in enumerate(specs):
            parts = []
            if s.get("setup"):
                parts.append(f"setup: {s['setup']}")
            parts.append(f"input: {s['input']}" if s.get("input") else "input: (none)")
            if s.get("run"):
                parts.append(f"run: {s['run']}")
            lines.append(f"  [{i}] " + " | ".join(parts))
            shown = ec.generated_for_spec("rpl", name, i, specs, gen)
            if not shown:
                on = ", ".join(s["models"]) if s.get("models") else "no model"
                lines.append(f"      not generated (on {on})")
            # Models with the same display on one line.
            by_text = {}
            for m, x in shown.items():
                by_text.setdefault(stack_text(x), []).append(m)
            for text, models in by_text.items():
                lines.append(f"      {' '.join(models)}: {text}")
            if s.get("comment"):
                lines.append(f"      # {s['comment']}")
        blocks[name] = "\n".join(lines)
    return blocks


def keyed(model, names):
    cmds = ec.keyed_commands(model)
    gen = ec.keyed_examples(model)
    blocks = {}
    for name in names:
        c = cmds[name]
        specs = c.get("examples", [])
        lines = [head(name, c.get("long"), c["description"], c.get("note"))]
        if c.get("keys"):
            lines.append(f"  keys: {steps_text(c['keys'])}")
        elif c.get("xeq"):
            lines.append(f'  keys: XEQ "{c["xeq"]}"')
        if c.get("stack"):
            lines.append(f"  stack: {c['stack']}")
        for i, s in enumerate(specs):
            x = ec.generated_for_spec(model, name, i, specs, gen).get(model)
            labels = x.get("labels") if x else None
            typed = ec.keyed_input_text(labels or s.get("input", []))
            then = f" | then: {ec.keyed_input_text(s['then'])}" if s.get("then") else ""
            # In the function's menu already: only its last key is pressed.
            last = c.get("keys", [])[-1:]
            menu = f" | in the menu: {steps_text(last)}" if s.get("in_menu") and last else ""
            lines.append(f"  [{i}] input: {typed or '(none)'}{menu}{then}")
            if x:
                lines.append(f"      before: {screen(x.get('before', []))}")
                lines.append(f"      after:  {screen(x.get('after', []))}")
            else:
                lines.append("      not generated")
            if s.get("comment"):
                lines.append(f"      # {s['comment']}")
        blocks[name] = "\n".join(lines)
    return blocks


def main(argv):
    p = argparse.ArgumentParser(description="List the reference's examples for writing comments.")
    p.add_argument("group", help=f"one of: {', '.join(ec.groups())}")
    p.add_argument("--from", dest="start", metavar="NAME", help="start at this command")
    p.add_argument("--count", type=int, metavar="N", help="list N commands")
    a = p.parse_args(argv)
    if a.group not in ec.groups():
        p.error(f"group {a.group!r} is none of {', '.join(ec.groups())}")
    if a.group == "rpl":
        all_cmds = ec.load("reference.json")["commands"]
    else:
        all_cmds = ec.keyed_commands(a.group)
    names = sorted(n for n, c in all_cmds.items() if c.get("examples"))
    if a.start is not None:
        if a.start in names:
            names = names[names.index(a.start):]
        else:
            names = [n for n in names if n > a.start]
    rest = []
    if a.count is not None:
        names, rest = names[: a.count], names[a.count:]
    blocks = rpl(names) if a.group == "rpl" else keyed(a.group, names)
    out = "\n\n".join(blocks[n] for n in names)
    sys.stdout.write(out + "\n" if out else "")
    total = sum(1 for c in all_cmds.values() if c.get("examples"))
    if rest:
        print(f"\n-- {len(names)} of {total} commands; next: --from {rest[0]!r} --count {a.count}")
    else:
        print(f"\n-- end ({total} commands with examples in {a.group})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
