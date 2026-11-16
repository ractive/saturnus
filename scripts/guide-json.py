#!/usr/bin/env python3
"""Generate web/guide.json, the text the AI tool read_guide gives.

The plugin's skill (plugins/saturnus/skills/saturnus/) is the one source:
SKILL.md without its frontmatter is the topic `skill`, and each file of
its references/ is the topic of its name (`rpl`, `models`, `workflows`).
The page loads web/guide.json when read_guide is first called
(web/ai-tools.js `guideLoader`); web/site.sh and the desktop app's build
ship it with the other page files, and the web app's service worker
precaches it. The JSON is committed and refreshed by running this script:

    scripts/guide-json.py                # write web/guide.json
    scripts/guide-json.py --check [FILE] # the committed file is current
"""

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SKILL = REPO / "plugins" / "saturnus" / "skills" / "saturnus"
OUT = REPO / "web" / "guide.json"

# The topics in the order read_guide names them (web/ai-tools.js GUIDE_TOPICS).
TOPICS = ["skill", "rpl", "models", "workflows"]


def without_frontmatter(text):
    """`text` without a leading YAML frontmatter block and the blank lines after it."""
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end < 0:
            sys.exit("SKILL.md: the frontmatter has no end")
        text = text[end + len("\n---\n"):]
    return text.lstrip("\n")


def build():
    topics = {"skill": without_frontmatter((SKILL / "SKILL.md").read_text(encoding="utf-8"))}
    refs = sorted(p.stem for p in (SKILL / "references").glob("*.md"))
    if sorted(TOPICS[1:]) != refs:
        sys.exit(f"references/ has {refs}, not the topics {TOPICS[1:]}: update TOPICS here and GUIDE_TOPICS in web/ai-tools.js")
    for topic in TOPICS[1:]:
        topics[topic] = (SKILL / "references" / f"{topic}.md").read_text(encoding="utf-8")
    return {"topics": topics}


def dump(data):
    return json.dumps(data, ensure_ascii=False, indent=1) + "\n"


def main():
    text = dump(build())
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        path = Path(sys.argv[2]) if len(sys.argv) > 2 else OUT
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != text:
            print(f"{path} is not what plugins/saturnus/skills/saturnus/ gives; run scripts/guide-json.py", file=sys.stderr)
            sys.exit(1)
        print(f"{path}: current ({len(TOPICS)} topics)")
        return
    OUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUT} ({len(TOPICS)} topics, {len(text.encode()) // 1024} KB)")


if __name__ == "__main__":
    main()
