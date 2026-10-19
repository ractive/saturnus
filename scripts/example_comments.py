"""The reference's examples and their comments, shared by
scripts/commands-json.py, list-examples.py, apply-comments.py and
check-comments.py.

A comment (`comment`, optional) belongs to an example spec: in
`reference.json` (`commands.<name>.examples[i]`, the 48SX, 48GX, 49G and
28S) and in `keys-<model>.json` (`commands[k].examples[i]`, the key-driven
models; the 40G's file equals the 39G's). The generated
`examples-<model>.json` never hold one: the comment is joined to the
generated example here, by matching the two.

- RPL models: a model runs the specs that are `on` it, in order; a
  generated example is matched to the first unused spec with the same
  `setup`, `input` and `run` (the command's name when the spec has none).
- Key-driven models: every spec runs, in order; a generated example is
  matched by its index, checked by its `input` and `then` (else the first
  unused spec with the same ones).

`SATURNUS_COMMANDS_DATA` overrides the data folder (the tests use a copy).

Writers give comments in a sidecar file (kept in `comments/` of the data
folder):

    {"group": "rpl" | "<model>",
     "comments": {"<command>": [{"i": <example index>, "comment": "..."}]}}

`i` is the index into the spec list (`reference.json` or
`keys-<model>.json`), as `scripts/list-examples.py` prints it. A comment
of null or "" removes one.
"""

import json
import os
import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DATA = Path(os.environ.get("SATURNUS_COMMANDS_DATA") or REPO / "crates" / "saturnus-cli" / "data" / "commands")
RPL_MODELS = ["48sx", "48gx", "49g", "28s"]
# The 40G's reference is the 39G's: writers use 39g, applied to both.
MIRRORS = {"39g": ["40g"]}
MAX_LEN = 90


def load(name):
    with open(DATA / name, encoding="utf-8") as f:
        return json.load(f)


def keyed_models():
    """Every key-driven model with a keys-<model>.json."""
    return sorted(p.stem[len("keys-"):] for p in DATA.glob("keys-*.json"))


def groups():
    """The sidecar groups: rpl and the key-driven models but the mirrors."""
    mirrored = {m for ms in MIRRORS.values() for m in ms}
    return ["rpl"] + [m for m in keyed_models() if m not in mirrored]


def spec_on(spec, model):
    models = spec.get("models")
    return models is None or model in models


def rpl_key(name, spec):
    return (spec.get("setup"), spec.get("input", ""), spec.get("run") or name)


def match_rpl(name, specs, generated, model):
    """The spec index of each generated example of `name` on `model`
    (None where none matches: the examples are older than the specs)."""
    free = [i for i, s in enumerate(specs) if spec_on(s, model)]
    out = []
    for x in generated:
        key = (x.get("setup"), x.get("input", ""), x.get("run"))
        hit = next((i for i in free if rpl_key(name, specs[i]) == key), None)
        if hit is not None:
            free.remove(hit)
        out.append(hit)
    return out


def keyed_key(spec):
    return (list(spec.get("input", [])), list(spec.get("then", [])))


def match_keyed(specs, generated):
    """The spec index of each generated example of a key-driven function."""
    free = list(range(len(specs)))
    out = []
    for j, x in enumerate(generated):
        key = keyed_key(x)
        if j in free and keyed_key(specs[j]) == key:
            hit = j
        else:
            hit = next((i for i in free if keyed_key(specs[i]) == key), None)
        if hit is not None:
            free.remove(hit)
        out.append(hit)
    return out


def rpl_examples():
    """Per RPL model, its generated examples by command."""
    return {m: load(f"examples-{m}.json")["examples"] for m in RPL_MODELS}


def keyed_examples(model):
    path = DATA / f"examples-{model}.json"
    return load(path.name)["examples"] if path.exists() else {}


def generated_for_spec(group, name, index, specs=None, generated=None):
    """Per model, the generated example of spec `index` of `name`:
    {model: example}. `specs`/`generated` may be passed to save loads."""
    out = {}
    if group == "rpl":
        specs = specs if specs is not None else load("reference.json")["commands"][name].get("examples", [])
        generated = generated if generated is not None else rpl_examples()
        for m in RPL_MODELS:
            runs = generated[m].get(name, [])
            for x, i in zip(runs, match_rpl(name, specs, runs, m)):
                if i == index:
                    out[m] = x
    else:
        runs = (generated if generated is not None else keyed_examples(group)).get(name, [])
        for x, i in zip(runs, match_keyed(specs, runs)):
            if i == index:
                out[group] = x
    return out


def keyed_commands(model):
    return {c["name"]: c for c in load(f"keys-{model}.json")["commands"]}


def specs_of(group, name, reference=None, keyed=None):
    """The example specs of `name` in `group`, or None when no such command."""
    if group == "rpl":
        ref = reference if reference is not None else load("reference.json")["commands"]
        entry = ref.get(name)
        return None if entry is None else entry.get("examples", [])
    cmds = keyed if keyed is not None else keyed_commands(group)
    c = cmds.get(name)
    return None if c is None else c.get("examples", [])


# ---------------------------------------------------------------- checks

BANNED = [
    (re.compile(r"\bthis example\b", re.I), '"this example"'),
    (re.compile(r"\bhere\b", re.I), '"here"'),
    (re.compile(r"\bwe\b", re.I), '"we"'),
    (re.compile(r"\bsimply\b", re.I), '"simply"'),
    (re.compile(r"\bjust\b", re.I), '"just"'),
    (re.compile(r"\bnote that\b", re.I), '"note that"'),
    (re.compile(r"!"), "an exclamation mark"),
    (re.compile("[\U0001F000-\U0001FAFF☀-➿️‍]"), "an emoji"),
]
NUMBER = re.compile(r"\d+(?:\.\d+)?")


def problems(comment):
    """Why `comment` breaks the rules (empty when it does not)."""
    out = []
    if not isinstance(comment, str) or not comment.strip():
        return ["empty"]
    if len(comment) > MAX_LEN:
        out.append(f"{len(comment)} characters, over {MAX_LEN}")
    if comment != comment.strip():
        out.append("spaces at an end")
    if comment.rstrip().endswith(".") and not comment.rstrip().endswith("..."):
        out.append("ends with a full stop")
    if "\n" in comment:
        out.append("more than one line")
    for rx, what in BANNED:
        if rx.search(comment):
            out.append(f"contains {what}")
    return out


def digits(text):
    """The numbers in `text` as digit strings, decimal points removed."""
    return [n.replace(".", "") for n in NUMBER.findall(text)]


def keyed_input_text(keys):
    """Key names (or labels) with runs of digit keys joined into numbers:
    1 0 0 → 100, 1 point 5 → 1.5."""
    out, run = [], ""
    for k in keys:
        if (len(k) == 1 and k.isdigit()) or k in (".", "point"):
            run += k if k.isdigit() else "."
            continue
        if run:
            out.append(run)
            run = ""
        out.append(k)
    if run:
        out.append(run)
    return " ".join(out)


def unfound_numbers(group, comment, spec, generated):
    """The numbers of `comment` found neither in the spec's input, setup or
    run nor in the generated result on every model that shows it."""
    if group == "rpl":
        source = " ".join(filter(None, [spec.get("setup"), spec.get("input"), spec.get("run")]))
        results = [" ".join(x.get("display", []) + ([x["error"]] if x.get("error") else [])) for x in generated.values()]
    else:
        source = " ".join([keyed_input_text(spec.get("input", [])), keyed_input_text(spec.get("then", []))])
        results = [" ".join([keyed_input_text(x.get("labels", []))] + x.get("after", [])) for x in generated.values()]
    src = digits(source)
    res = [digits(r) for r in results]
    out = []
    for n in digits(comment):
        if any(n in s for s in src):
            continue
        if res and all(any(n in s for s in r) for r in res):
            continue
        out.append(n)
    return out


# ---------------------------------------------------------------- sidecars

def read_sidecar(path):
    """A sidecar file, checked for shape: (group, [(command, i, comment)]).
    Raises ValueError with what is wrong."""
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    if not isinstance(doc, dict) or set(doc) - {"group", "comments"}:
        raise ValueError(f"{path}: needs exactly \"group\" and \"comments\"")
    group = doc.get("group")
    if group not in groups():
        raise ValueError(f"{path}: group {group!r} is none of {', '.join(groups())}")
    comments = doc.get("comments")
    if not isinstance(comments, dict):
        raise ValueError(f"{path}: \"comments\" must map command names to lists")
    out = []
    for name, items in comments.items():
        if not isinstance(items, list):
            raise ValueError(f"{path}: {name}: needs a list of {{\"i\", \"comment\"}}")
        for item in items:
            if not isinstance(item, dict) or set(item) != {"i", "comment"} or not isinstance(item["i"], int) or isinstance(item["i"], bool):
                raise ValueError(f"{path}: {name}: each item is {{\"i\": <index>, \"comment\": \"...\"}}, not {item!r}")
            if item["comment"] is not None and not isinstance(item["comment"], str):
                raise ValueError(f"{path}: {name} [{item['i']}]: the comment must be text or null")
            out.append((name, item["i"], item["comment"]))
    return group, out
