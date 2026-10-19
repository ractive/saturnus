#!/usr/bin/env python3
"""Check the comments on the reference's examples (kb/docs/example-comments.md).

    scripts/check-comments.py            # the comments in reference.json and keys-<model>.json
    scripts/check-comments.py FILE...    # the comments of sidecar files, before applying them

Fails (exit status 1) on a comment over 90 characters, ending with a full
stop, on more than one line, or with "this example", "here", "we",
"simply", "just", "note that", an exclamation mark or an emoji; on a
sidecar's command or index that does not exist.

Lists, without failing, every number in a comment found neither in the
example's input, setup or run nor in its result on every model that
shows it (the generated examples-<model>.json), for a person to read:
digits are compared with the decimal point removed and may be part of a
longer number, so 1.3 matches the input 1.3 and 15 the date 7.151991,
but 30 in "1h30m" is listed.
"""

import json
import sys

import example_comments as ec


def committed():
    """(group, name, i, comment) of every comment in the data."""
    for name, e in ec.load("reference.json")["commands"].items():
        for i, x in enumerate(e.get("examples", [])):
            if "comment" in x:
                yield "rpl", name, i, x["comment"]
    for m in ec.keyed_models():
        for c in ec.load(f"keys-{m}.json")["commands"]:
            for i, x in enumerate(c.get("examples", [])):
                if "comment" in x:
                    yield m, c["name"], i, x["comment"]


def main(argv):
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return 0
    errors = []
    items = []
    if argv:
        for path in argv:
            try:
                group, got = ec.read_sidecar(path)
            except (ValueError, OSError, json.JSONDecodeError) as e:
                errors.append(str(e))
                continue
            items.extend((path, group, name, i, c) for name, i, c in got)
    else:
        items = [(None, g, name, i, c) for g, name, i, c in committed()]
    reference = ec.load("reference.json")["commands"]
    rpl_gen = ec.rpl_examples()
    keyed_cmds = {}
    keyed_gen = {}
    listed = []
    for path, group, name, i, comment in items:
        where = f"{path + ': ' if path else ''}{group} {name} [{i}]"
        if group != "rpl" and group not in keyed_cmds:
            keyed_cmds[group] = ec.keyed_commands(group)
            keyed_gen[group] = ec.keyed_examples(group)
        specs = ec.specs_of(group, name, reference, keyed_cmds.get(group))
        if specs is None:
            errors.append(f"{where}: no command {name!r} in {group}")
            continue
        if not 0 <= i < len(specs):
            errors.append(f"{where}: no example {i} ({name} has {len(specs)})")
            continue
        if not comment and path:
            continue  # a sidecar's null removes a comment
        errors.extend(f"{where}: {p}: {comment!r}" for p in ec.problems(comment))
        if not isinstance(comment, str):
            continue
        gen = ec.generated_for_spec(group, name, i, specs, rpl_gen if group == "rpl" else keyed_gen[group])
        numbers = ec.unfound_numbers(group, comment, specs[i], gen)
        if numbers:
            listed.append(f"{where}: {', '.join(numbers)} not in the example: {comment!r}")
    for e in errors:
        print(e, file=sys.stderr)
    if listed:
        print(f"{len(listed)} comment(s) with numbers to read (not a failure):")
        for line in listed:
            print(f"  {line}")
    print(f"{len(items)} comment(s) checked, {len(errors)} problem(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
