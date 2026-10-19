#!/usr/bin/env python3
"""The HP 28S's menus and manual pages, from its Reference Manual.

The 28S has no menu table saturnus-refgen can read (its catalog's names
come from the ROM's CATALOG, read off the display), so its menus come
from the HP-28S Reference Manual: a dictionary organised by menu that
lists each menu's rows of labels (contents, p. 5-8, and the menu
headings). This script writes, for the commands of
crates/saturnus-cli/data/commands/28s.json:

- 28s.json: each command's `menus` (the legend of the key that opens the
  menu) and `menu_keys` in the keyboard's order;
- manuals.json: the manual and each command's PDF page (`hp28s-rm`);
- categories.json: each command's dictionary section and page (`28s`).

The other models' parts of those files are left as they are. The manual's
text is read from the knowledge base as scripts/manual-pages.py does
(--text-dir DIR, $CALCULATOR_KB_TEXT, else
~/devel/calculator-knowledgebase/raw/manuals/text, block N = PDF page N);
without it the script says so and changes nothing. Only page numbers and
the menus' label lists are kept, no manual text.

Usage: scripts/manual-28s.py [--text-dir DIR]
"""

import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "crates", "saturnus-cli", "data", "commands")
MANUAL = {
    "id": "hp28s-rm",
    "title": "HP-28S Reference Manual",
    "url": "https://literature.hpcalc.org/community/hp28s-rm-en.pdf",
    "models": ["28s"],
}
TEXT = "hp28s-rm-en.txt"

# Each menu by the legend of its key, with the labels that are commands
# in the manual's row order. Labels that run no command are left out:
# SST (CONTRL), TRAC (PRINT), SOLVR (SOLV), PPAR (PLOT, a variable), and
# MODE's CMD, UNDO, LAST, ML and RDX, toggles (MODE's LAST is not the
# LAST command, which is a key).
MENUS = [
    ("ARRAY", "→ARRY ARRY→ PUT GET PUTI GETI SIZE RDM TRN CON IDN RSD CROSS DOT DET ABS RNRM CNRM R→C C→R RE IM CONJ NEG"),
    ("BINARY", "DEC HEX OCT BIN STWS RCWS RL RR RLB RRB R→B B→R SL SR SLB SRB ASR AND OR XOR NOT"),
    ("COMPLX", "R→C C→R RE IM CONJ SIGN R→P P→R ABS NEG ARG"),
    ("STRING", "→STR STR→ CHR NUM →LCD LCD→ POS SUB SIZE DISP"),
    ("LIST", "→LIST LIST→ PUT GET PUTI GETI POS SUB SIZE"),
    ("REAL", "NEG FACT RAND RDZ MAXR MINR ABS SIGN MANT XPON IP FP FLOOR CEIL RND MAX MIN MOD %T"),
    ("STACK", "DUP OVER DUP2 DROP2 ROT LIST→ ROLLD PICK DUPN DROPN DEPTH →LIST"),
    ("STORE", "STO+ STO- STO* STO/ SNEG SINV SCONJ"),
    ("MEMORY", "MEM MENU ORDER PATH HOME CRDIR VARS CLUSR"),
    ("ALGBRA", "COLCT EXPAN SIZE FORM OBSUB EXSUB TAYLR ISOL QUAD SHOW OBGET EXGET"),
    ("STAT", "Σ+ Σ- NΣ CLΣ STOΣ RCLΣ TOT MEAN SDEV VAR MAXΣ MINΣ COLΣ CORR COV LR PREDV UTPC UTPF UTPN UTPT COMB PERM"),
    ("PRINT", "PR1 PRST PRVAR PRLCD CR PRSTC PRUSR PRMD"),
    ("CONTRL", "HALT ABORT KILL WAIT KEY BEEP CLLCD DISP CLMF ERRN ERRM"),
    ("BRANCH", "IF IFERR THEN ELSE END START FOR NEXT STEP IFT IFTE DO UNTIL WHILE REPEAT"),
    ("TEST", "SF CF FS? FC? FS?C FC?C AND OR XOR NOT SAME == STOF RCLF TYPE"),
    ("MODE", "STD FIX SCI ENG DEG RAD PRMD"),
    ("TRIG", "SIN ASIN COS ACOS TAN ATAN P→R R→P R→C C→R ARG →HMS HMS→ HMS+ HMS- D→R R→D"),
    ("LOGS", "LOG ALOG LN EXP LNP1 EXPM SINH ASINH COSH ACOSH TANH ATANH"),
    ("SOLV", "STEQ RCEQ ISOL QUAD SHOW ROOT"),
    ("PLOT", "STEQ RCEQ PMIN PMAX INDEP DRAW RES AXES CENTR *W *H STOΣ RCLΣ COLΣ SCLΣ DRWΣ CLLCD DGTIZ PIXEL DRAX CLMF PRLCD"),
]

# The dictionary's sections by first PDF page.
SECTIONS = [
    (18, "ALGEBRA"), (36, "ALGEBRA (FORM)"), (55, "Arithmetic"), (65, "ARRAY"),
    (87, "BINARY"), (98, "Calculus"), (112, "COMPLEX"), (126, "Evaluation"),
    (129, "LIST"), (135, "LOGS"), (141, "MEMORY"), (147, "MODE"), (154, "PLOT"),
    (170, "PRINT"), (178, "Programs"), (185, "PROGRAM BRANCH"),
    (195, "PROGRAM CONTROL"), (203, "PROGRAM TEST"), (215, "REAL"), (226, "SOLVE"),
    (241, "STACK"), (247, "STAT"), (260, "STORE"), (265, "STRING"), (275, "TRIG"),
    (285, "UNITS"), (300, None),
]
# The section each menu's commands are described in.
MENU_SECTION = {
    "ARRAY": "ARRAY", "BINARY": "BINARY", "COMPLX": "COMPLEX", "STRING": "STRING",
    "LIST": "LIST", "REAL": "REAL", "STACK": "STACK", "STORE": "STORE",
    "MEMORY": "MEMORY", "ALGBRA": "ALGEBRA", "STAT": "STAT", "PRINT": "PRINT",
    "CONTRL": "PROGRAM CONTROL", "BRANCH": "PROGRAM BRANCH", "TEST": "PROGRAM TEST",
    "MODE": "MODE", "TRIG": "TRIG", "LOGS": "LOGS", "SOLV": "SOLVE", "PLOT": "PLOT",
}

# An entry's heading: the name, the long name, the kind.
HEADING = re.compile(
    r"^\s{0,30}(\S+(?: \S+)?)\s{3,}(\S.*?\S)\s{3,}(Command|Function|Analytic|Operation)\s*$"
)
# What the scan makes of a heading's name, where it is not the name.
OCR = {
    "X": "*", "|": "/", "\\": "√", "iM": "IM", "FiIX": "FIX", "SCi": "SCI", "P": "IP",
    "RCL2": "RCLΣ", "DRW2": "DRWΣ", "COL>": "COLΣ", "SCL>": "SCLΣ", "2+": "Σ+",
    "22-": "Σ-", "N2": "NΣ", "CL2": "CLΣ", "MAX2": "MAXΣ", "MIN2": "MINΣ",
    "COLZ2": "COLΣ", "STOZ": "STOΣ", "cov": "COV", "STOx": "STO*", "*xW": "*W",
    "d": "∂", "I}": "∫", "#*": "≠", "-HMS": "→HMS", "HMS-": "HMS→", "HMS -": "HMS-",
    "STO-": "STO-",
}
# Entries whose heading the scan lost or garbled, by PDF page: the
# program structures (described together), ^, ≤ and ≥, the constants e,
# i and π (one paragraph in ALGEBRA), and → (local variables, Programs).
PAGES = {
    "≤": [205], "≥": [205], "<": [204], "=": [23], "STO-": [261], "FS?": [207],
    "IF": [189], "IFERR": [189], "THEN": [189], "ELSE": [189], "END": [189, 194],
    "START": [191], "FOR": [191], "NEXT": [191], "STEP": [191],
    "DO": [194], "UNTIL": [194], "WHILE": [194], "REPEAT": [194],
    "^": [62], "e": [29], "i": [29], "π": [29], "→": [180],
}


def section(page):
    found = None
    for first, name in SECTIONS:
        if page >= first:
            found = name
    return found


def heading_name(raw):
    if raw in OCR:
        return OCR[raw]
    h = raw.replace("—", "-").replace(" ", "")
    if h in OCR:
        return OCR[h]
    h = re.sub(r"^-(?=[A-Z])", "→", h)
    h = re.sub(r"(?<=[A-Z])-(?=[A-Z])", "→", h)
    h = re.sub(r"(?<=[A-Z])-$", "→", h)
    return h


def entry_pages(pages, names):
    """Each name's dictionary pages, in page order."""
    found = {}
    for i in range(17, 300):
        for line in pages[i].splitlines():
            m = HEADING.match(line)
            if m:
                name = heading_name(m.group(1))
                found.setdefault(name, [])
                if i + 1 not in found[name]:
                    found[name].append(i + 1)
    out = {}
    for n in names:
        out[n] = PAGES.get(n) or found.get(n) or []
    return out


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
    path = os.path.join(text_dir, TEXT)
    if not os.path.exists(path):
        print(
            f"28S manual skipped: {path} not found; the manuals live in the knowledge base's "
            "raw/ (--text-dir, $CALCULATOR_KB_TEXT), not in this repository; the data files "
            "are left as they are",
            file=sys.stderr,
        )
        return
    with open(path, encoding="utf-8", errors="replace") as f:
        pages = f.read().split("\f")
    catalog_path = os.path.join(DATA, "28s.json")
    with open(catalog_path, encoding="utf-8") as f:
        catalog = json.load(f)
    names = [c["name"] for c in catalog["commands"]]
    for key, labels in MENUS:
        for n in labels.split():
            if n not in names:
                sys.exit(f"{n} (menu {key}) is not in 28s.json")
    for c in catalog["commands"]:
        c["menus"] = [k for k, labels in MENUS if c["name"] in labels.split()]
    catalog["menu_keys"] = [{"key": k} for k, _ in MENUS]
    found = entry_pages(pages, names)
    missing = [n for n, p in found.items() if not p]
    if missing:
        sys.exit(f"no dictionary page for {' '.join(missing)}")
    # The page of the section that describes the command in the first of
    # its menus, else its first page.
    primary = {}
    for c in catalog["commands"]:
        want = [MENU_SECTION[k] for k in c["menus"]]
        ps = found[c["name"]]
        primary[c["name"]] = next((p for p in ps if section(p) in want), ps[0])

    with open(os.path.join(DATA, "manuals.json"), encoding="utf-8") as f:
        manuals = json.load(f)
    manuals["manuals"] = [m for m in manuals["manuals"] if m["id"] != MANUAL["id"]] + [MANUAL]
    for n, p in manuals["pages"].items():
        p.pop(MANUAL["id"], None)
    for n, page in primary.items():
        manuals["pages"].setdefault(n, {})[MANUAL["id"]] = page
    manuals["pages"] = {
        n: dict(sorted(p.items())) for n, p in sorted(manuals["pages"].items()) if p
    }

    with open(os.path.join(DATA, "categories.json"), encoding="utf-8") as f:
        categories = json.load(f)
    commands = categories["commands"]
    for v in commands.values():
        v.pop("28s", None)
    for c in catalog["commands"]:
        n = c["name"]
        placed = {"category": section(primary[n]), "manual": MANUAL["id"], "page": primary[n]}
        if c["menus"]:
            placed["menus"] = c["menus"]
        commands.setdefault(n, {})["28s"] = dict(sorted(placed.items()))
    categories["commands"] = {
        n: {m: e for m, e in sorted(v.items())} for n, v in sorted(commands.items()) if v
    }

    for p, doc in [
        (catalog_path, catalog),
        (os.path.join(DATA, "manuals.json"), manuals),
        (os.path.join(DATA, "categories.json"), categories),
    ]:
        with open(p, "w", encoding="utf-8") as f:
            json.dump(doc, f, ensure_ascii=False, indent=2)
            f.write("\n")
    placed = sum(1 for c in catalog["commands"] if c["menus"])
    print(f"28s: {placed} of {len(names)} commands in a menu, {len(primary)} pages", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1:])
