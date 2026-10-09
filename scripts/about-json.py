#!/usr/bin/env python3
"""Generate web/about.json, the source list of the About panel.

Reads the frontmatter of every source page of the hardware wiki
(~/devel/hp-literature, or the directory given as the first argument)
through `hyalo`, together with the wiki pages that cite each source, and
adds the inputs that are not wiki sources (the black-box oracle, the skin
references, hptx, the ROM policy). The wiki stays outside this
repository; the JSON is committed and refreshed by running this script:

    scripts/about-json.py [WIKI_DIR]
    scripts/about-json.py --check [FILE]   # validate the committed JSON

Only titles, authors, years, public URLs and tags are taken from the wiki;
every sentence in the output is written here, none is copied from a
source or from the wiki's prose. The page is public, so the output names
no local location: a source's wiki `raw` path becomes `"archived": true`
(the project's literature archive holds a copy) and anything else that is
not an http(s) URL is dropped. The script refuses to write, and `--check`
fails on, any string that looks like a local path or an e-mail address.
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


def private_strings(obj, path="$"):
    """(path, value) of every string in `obj` that looks private."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield from private_strings(v, f"{path}.{k}")
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            yield from private_strings(v, f"{path}[{i}]")
    elif isinstance(obj, str) and PRIVATE.search(obj):
        yield path, obj


def check(about):
    """Exit with a message if `about` holds anything private."""
    bad = list(private_strings(about))
    for path, value in bad:
        print(f"about.json: private-looking value at {path}: {value!r}", file=sys.stderr)
    if bad:
        sys.exit(1)

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "web" / "about.json"

# Wiki pages that cite everything (catalogue, log) or are other sources;
# they say nothing about what a source was used for.
NOT_USES = {"index.md", "log.md", "overview.md"}

# Wiki directories, as the reader should see them.
SECTIONS = {
    "hardware": "hardware",
    "protocols": "protocol",
    "emulators": "emulator note",
    "questions": "open question",
    "decisions": "decision",
    "synthesis": "synthesis",
}


def hyalo(wiki, *args):
    out = subprocess.run(
        ["hyalo", *args, "--format", "json", "--limit", "0"],
        cwd=wiki,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)["results"]


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        target = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        check(json.loads(target.read_text(encoding="utf-8")))
        print(f"{target}: no private values")
        return
    wiki = Path(sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/devel/hp-literature"))
    titles = {r["file"]: r.get("title") or r["file"] for r in hyalo(wiki, "find", "--fields", "title")}
    pages = hyalo(
        wiki,
        "find",
        "--property",
        "type=source",
        "--fields",
        "title,properties,tags,links,backlinks",
    )
    sources = []
    for p in pages:
        props = p.get("properties", {})
        raw = str(props.get("raw", ""))
        urls = [
            link["target"]
            for link in p.get("links", [])
            if link.get("kind") == "external" and str(link.get("target", "")).startswith(("https://", "http://"))
        ]
        if raw.startswith(("https://", "http://")):
            url, archived = raw, False
        else:
            # A raw/ path is a copy in the literature archive; any other
            # location (a local note) is not published at all.
            url, archived = (urls[0] if urls else None), raw.startswith("raw/")
        used = []
        for b in p.get("backlinks", []):
            src = b["source"]
            if src in NOT_USES or src.startswith("sources/"):
                continue
            kind = SECTIONS.get(src.split("/", 1)[0], "page")
            entry = {"page": src.removesuffix(".md"), "title": titles.get(src, src), "kind": kind}
            if entry not in used:
                used.append(entry)
        used.sort(key=lambda e: (e["kind"], e["title"]))
        authors = props.get("authors") or []
        if isinstance(authors, str):
            authors = [authors]
        sources.append(
            {
                "page": p["file"].removesuffix(".md"),
                "title": p.get("title") or p["file"],
                "authors": authors,
                "year": props.get("year"),
                "url": url,
                "archived": archived,
                "status": props.get("status"),
                "tags": p.get("tags", []),
                "usedFor": used,
            }
        )
    sources.sort(key=lambda s: (s["title"].lower(), s["page"]))

    about = {
        "generator": "scripts/about-json.py",
        "wiki": "hp-literature (the project's hardware wiki, kept outside this repository)",
        "statement": [
            "saturnus emulates the HP 48SX, 48GX, 49G, 38G, 39G, 40G and 42S.",
            "It was written from HP's documentation, the hardware literature listed below, "
            "other emulators' documentation and change logs, and black-box runs of the "
            "calculators' ROMs. Each fact is recorded with its source in the project's "
            "hardware wiki.",
            "It contains no code from Emu48, Emu42, jsEmu48, x48, x48ng, x50ng, ui4x, "
            "saturnng or HP EMU.",
            "The calculator drawings are saturnus's own, measured from photographs of the "
            "calculators and from the keyboard figures in HP's manuals (see Calculator drawings).",
            "No ROM is included. HP has allowed its ROM files to be downloaded from "
            "hpcalc.org since 2000.",
            "MIT licence. Parts of the code were generated by AI under human supervision "
            "(see AI_NOTICE).",
            "Not affiliated with HP. HP, HP48 and HP49 are trademarks of HP Inc.",
        ],
        "oracles": [
            {
                "name": "saturnng",
                "url": "https://codeberg.org/gwh/saturnng",
                "licence": "GPL",
                "use": "The same ROM and keys are run in saturnng and in saturnus, and the "
                "screens are compared.",
            },
        ],
        "skins": [
            {
                "model": "HP 48SX",
                "reference": "Photographs of the author's 48SX; the keyboard figure of "
                "the HP 48SX Owner's Manual, volume 1, for the labels.",
            },
            {
                "model": "HP 48GX",
                "reference": "The 48SX geometry (one case); the keyboard figure of the HP 48G "
                "Series User's Guide for the labels.",
            },
            {
                "model": "HP 38G",
                "reference": "Photographs of the author's 38G; the HP 38G User's Guide "
                "for the labels.",
            },
            {
                "model": "HP 49G",
                "reference": "Photographs of the author's 49G; the keyboard figure of "
                "the HP 49G User's Manual for the labels.",
            },
            {
                "model": "HP 42S",
                "reference": "Photographs of the author's 42S, for the geometry, the "
                "labels and the colours.",
            },
            {
                "model": "HP 39G and 40G",
                "reference": "The keyboard figure of the HP 39G/40G Graphing Calculator "
                "User's Guide.",
            },
        ],
        "tools": [
            {
                "name": "hptx",
                "url": "https://github.com/ractive/hptx",
                "use": "The author's Kermit and XModem transfer tool; its protocol code moves "
                "objects in and out of saturnus, and its emulator image runs saturnng for "
                "the comparison.",
            },
        ],
        "roms": [
            "saturnus includes no ROM. The ROMs are HP's software, hosted by hpcalc.org "
            "with HP's permission for use with emulators; they are not part of saturnus, and "
            "saturnus neither hosts nor passes them on.",
            "The web page links each model to its download on hpcalc.org: download it there, "
            "unzip it and drop the file on the page. The app downloads a model's ROM from "
            "hpcalc.org after asking, and the command \"saturnus rom fetch\" does the same; "
            "both check each file's size and SHA-256.",
            "The web page keeps the ROM you choose in this browser. \"Remove ROMs…\" deletes "
            "it, together with the saved 49G state, which contains the 49G's ROM. The app "
            "remembers where the ROM file is and reads it from there.",
            "HP never published the 42S ROM; the 42S runs from a copy read out of your own "
            "calculator.",
        ],
        "sources": sources,
    }
    check(about)
    OUT.write_text(json.dumps(about, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{OUT.relative_to(REPO)}: {len(sources)} wiki sources")


if __name__ == "__main__":
    main()
