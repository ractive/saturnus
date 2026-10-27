#!/usr/bin/env bash
# Puts a short "Download the desktop app" block at the top of a release's
# notes, with links to that release's own Mac and Windows installers, so
# people find them among the ~45 files of a release.
#
#   scripts/release-downloads.sh [--dry-run] TAG
#
# desktop.yml's `notes` job runs it once the installers are attached; by
# hand it fills in an older release (kb/docs/releasing.md, "Desktop
# app"). It needs `gh` with write access to the release (GH_TOKEN, and
# GH_REPO outside a clone). The block starts with a marker comment, so a
# second run changes nothing. --dry-run prints the new notes instead of
# saving them. Its paragraphs are one line each: a release's notes keep
# single line breaks.
set -euo pipefail

marker='<!-- saturnus-desktop-downloads -->'

usage() {
  echo "usage: $0 [--dry-run] TAG" >&2
  exit 2
}

dry_run=false
if [ "${1:-}" = "--dry-run" ]; then
  dry_run=true
  shift
fi
[ "$#" -eq 1 ] || usage
tag=$1
version=${tag#v}

body=$(gh release view "$tag" --json body --jq .body)
if grep -qF -- "$marker" <<< "$body"; then
  echo "$tag: the notes already have the download block, nothing to do" >&2
  exit 0
fi

# "name url" per asset; url is the browser download address of this
# release's file, so the links stay right after newer releases.
assets=$(gh release view "$tag" --json assets --jq '.assets[] | .name + " " + .url')

# The download address of an asset, or nothing if the release lacks it.
asset_url() {
  awk -v name="$1" '$1 == name { print $2 }' <<< "$assets"
}

mac="saturnus_${version}_aarch64.dmg"
win="saturnus_${version}_x64-setup.exe"
win_arm="saturnus_${version}_arm64-setup.exe"
mac_url=$(asset_url "$mac")
win_url=$(asset_url "$win")
win_arm_url=$(asset_url "$win_arm")

# Every release has the Mac and Windows x64 installers once desktop.yml
# attached them; without them it is too early.
if [ -z "$mac_url" ] || [ -z "$win_url" ]; then
  echo "$tag lacks $mac or $win: attach the installers first" >&2
  exit 1
fi

block="$marker
## Download the desktop app

- **Mac** (Apple silicon): [$mac]($mac_url)
- **Windows** (Intel or AMD): [$win]($win_url)"
# Windows on ARM: only since 0.8.1.
if [ -n "$win_arm_url" ]; then
  block+="
- Windows on ARM: [$win_arm]($win_arm_url)"
fi
block+="

The Windows installers are not signed yet: if Windows says \"Windows protected your PC\", click \"More info\", then \"Run anyway\".

Linux, the .msi installers, the command-line tools and satx: see Assets at the end of this page.

---
"

new_body="$block
$body"

if [ "$dry_run" = true ]; then
  printf '%s\n' "$new_body"
  exit 0
fi

notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
printf '%s\n' "$new_body" > "$notes"
gh release edit "$tag" --notes-file "$notes" > /dev/null
echo "$tag: download block added to the notes" >&2
