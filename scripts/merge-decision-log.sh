#!/usr/bin/env sh
# Git merge driver for kb/decision-log.md (.gitattributes, `just
# merge-driver` registers it): keep both sides' appended sections, like
# merge=union, and put back the blank line before a `## ` heading that the
# union loses. Git trims the lines both sides added identically (the blank
# line in front of each new section) out of the merged hunk, so a plain
# union glues the second section's heading to the first one's last line,
# which MD022 rejects. As with merge=union, only appended sections merge
# safely: two sides editing the same lines keep both versions, unmarked.
#
# Usage (git passes the paths): scripts/merge-decision-log.sh %O %A %B
#   %O the merge base, %A ours (the result is written here), %B theirs.
set -eu

tmp="$2.blank-before-headings"
trap 'rm -f "$tmp"' EXIT

# --union resolves every conflict, so only an error (status above 127)
# fails the merge.
status=0
git merge-file --union "$2" "$1" "$3" || status=$?
if [ "$status" -gt 127 ]; then
    exit "$status"
fi

# A fence closes only on its own character, at least as long as it opened.
# Lines are compared without a CR, so CRLF blank lines count as blank.
awk '
    {
        line = $0
        cr = sub(/\r$/, "", line)
        s = line
        sub(/^ ? ? ?/, "", s)
        if (match(s, /^(`+|~+)/) && RLENGTH >= 3) {
            c = substr(s, 1, 1)
            n = RLENGTH
            if (!fence) {
                fence = 1; fence_char = c; fence_len = n
            } else if (c == fence_char && n >= fence_len && substr(s, n + 1) ~ /^[ \t]*$/) {
                fence = 0
            }
        } else if (!fence && line ~ /^## / && NR > 1 && prev != "") {
            print (cr ? "\r" : "")
        }
        print
        prev = line
    }
' "$2" > "$tmp"
mv "$tmp" "$2"
