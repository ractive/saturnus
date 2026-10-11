#!/usr/bin/env sh
# Git merge driver for kb/decision-log.md (.gitattributes, `just
# merge-driver` registers it): keep both sides' appended sections, like
# merge=union, and put back the blank line before a `## ` heading that the
# union loses. Git trims the lines both sides added identically (the blank
# line in front of each new section) out of the merged hunk, so a plain
# union glues the second section's heading to the first one's last line,
# which MD022 rejects.
#
# Usage (git passes the paths): scripts/merge-decision-log.sh %O %A %B
#   %O the merge base, %A ours (the result is written here), %B theirs.
set -eu

git merge-file --union "$2" "$1" "$3"

tmp="$2.blank-before-headings"
awk '
    /^(```|~~~)/ { fence = !fence }
    !fence && /^## / && NR > 1 && prev != "" { print "" }
    { print; prev = $0 }
' "$2" > "$tmp"
mv "$tmp" "$2"
