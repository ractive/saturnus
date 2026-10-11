#!/usr/bin/env sh
# Assembles the static site of https://ractive.ch/saturnus/ into the
# directory given as the first argument (default: site/):
#
#   .htaccess           web/site.htaccess, for the Apache host (no redirects)
#   index.html, ...     the landing page, web/landing/ (HTML, CSS, img/)
#   favicon.svg         web/favicon.svg
#   sw.js               the kill switch, web/landing/sw.js: replaces the
#                       service worker the emulator registered at /saturnus/
#                       before it moved (to be removed on 2027-01-15,
#                       kb/backlog/remove-kill-switch.md)
#   emulator/           the emulator: every page file in web/ (HTML, CSS,
#                       JS, JSON, SVG), the components, the shared code
#                       (web/shared/: its modules, components and
#                       stylesheet), the wasm package
#                       built by web/build.sh, and the installable page's
#                       files from web/pwa/: the manifest, the icons and
#                       the service worker, with index.html given the
#                       manifest link and Apple's tags (web/pwa/head.html).
#
# Tests, READMEs and build scripts stay out. The emulator's service worker
# is web/pwa/sw.js behind two constants written here: BUILD, a hash of
# every other file under emulator/ (a new deploy is a new cache; the
# landing page is not part of it), and FILES, their list relative to
# emulator/ (what it precaches). Then checks that every relative module
# import and `new URL(...)` under emulator/ and at the top names a file in
# the site, so a new module cannot be left out.
# Used by .github/workflows/pages.yml for ractive.ch.
#
# `web/site.sh --list` prints the emulator's page files (relative to web/,
# sorted, without the wasm package, the .htaccess, web/pwa/ and
# web/landing/) and copies nothing: the desktop app's build script embeds
# the same set (no service worker, no manifest, no landing page), and the
# saturnus-tauri test `frontend` compares the two.
set -eu
here=$(cd "$(dirname "$0")" && pwd)

# The emulator's page files, relative to web/, one per line, sorted.
page_files() {
  (
    cd "$here"
    for f in ./*.html ./*.css ./*.js ./*.json ./*.svg components/*.js shared/*.js shared/*.css shared/components/*.js; do
      [ -f "$f" ] && printf '%s\n' "${f#./}"
    done
  ) | LC_ALL=C sort
}

if [ "${1:-}" = "--list" ]; then
  page_files
  exit 0
fi

out=${1:-site}
out_abs=$(mkdir -p "$out" && cd "$out" && pwd)
# The target is deleted and rebuilt, so it must be neither web/ (or inside
# it) nor an ancestor of web/ (the checkout, $HOME), and a non-empty
# target must carry the marker an earlier run left.
marker=.saturnus-site
case "$out_abs/" in "$here"/*|"$here/") echo "web/site.sh: the target must not be web/ or inside it" >&2; exit 1;; esac
case "$here/" in "$out_abs"/*) echo "web/site.sh: the target must not contain web/" >&2; exit 1;; esac
if [ -n "$(ls -A "$out_abs")" ] && [ ! -f "$out_abs/$marker" ]; then
  echo "web/site.sh: $out is not empty and was not made by web/site.sh (no $marker); not deleting it" >&2
  exit 1
fi
[ -f "$here/pkg/saturnus_web_bg.wasm" ] || { echo "web/site.sh: run web/build.sh first (no web/pkg)" >&2; exit 1; }
rm -rf "$out"
emu="$out/emulator"
mkdir -p "$emu/pkg" "$emu/components" "$emu/shared/components" "$out/img"
: > "$out/$marker"

# The top: the .htaccess (it applies to emulator/ too), the landing page,
# the favicon and the kill switch.
cp "$here/site.htaccess" "$out/.htaccess"
for f in "$here"/landing/*.html "$here"/landing/*.css; do
  cp "$f" "$out/$(basename "$f")"
done
cp "$here"/landing/img/* "$out/img/"
cp "$here/favicon.svg" "$out/favicon.svg"
cp "$here/landing/sw.js" "$out/sw.js"

# The emulator.
page_files | while IFS= read -r f; do
  cp "$here/$f" "$emu/$f"
done
cp "$here/pkg/saturnus_web.js" "$here/pkg/saturnus_web_bg.wasm" "$emu/pkg/"
# The .htaccess's Content-Security-Policy, minus frame-ancestors, as a meta
# tag in the emulator's index.html (for a host that sends no headers). Only
# the site's copy: the desktop app has its own policy (tauri.conf.json).
csp=$(sed -n "s/^Header always set Content-Security-Policy \"\(.*\)\"\$/\1/p" "$here/site.htaccess" | sed "s/; *frame-ancestors [^;]*//")
[ -n "$csp" ] || { echo "web/site.sh: no Content-Security-Policy in site.htaccess" >&2; exit 1; }
sed "s|^<meta charset=\"utf-8\">\$|&\\
<meta http-equiv=\"Content-Security-Policy\" content=\"$csp\">|" "$here/index.html" > "$emu/index.html"
grep -q 'http-equiv="Content-Security-Policy"' "$emu/index.html" || { echo "web/site.sh: could not add the CSP to index.html" >&2; exit 1; }

# The installable page: the manifest, the icons and the head tags (after
# the CSP), then the service worker over every other file under emulator/.
mkdir -p "$emu/icons"
cp "$here/pwa/manifest.webmanifest" "$emu/"
cp "$here"/pwa/icons/*.png "$emu/icons/"
sed "/^<meta http-equiv=\"Content-Security-Policy\"/r $here/pwa/head.html" "$emu/index.html" > "$emu/index.html.tmp"
mv "$emu/index.html.tmp" "$emu/index.html"
grep -q '<link rel="manifest"' "$emu/index.html" || { echo "web/site.sh: could not add the manifest to index.html" >&2; exit 1; }
sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}
precached=$(cd "$emu" && find . -type f | sed 's|^\./||' | LC_ALL=C sort)
# Each file's name and content hash, hashed: a change, a new or a removed file is a new build.
build=$(cd "$emu" && printf '%s\n' "$precached" | while IFS= read -r f; do sha256 "$f"; done | sha256 | cut -c1-16)
{
  printf '// Written by web/site.sh.\nconst BUILD = "%s";\nconst FILES = [\n' "$build"
  printf '%s\n' "$precached" | sed 's|.*|  "&",|'
  printf '];\n\n'
  cat "$here/pwa/sw.js"
} > "$emu/sw.js"
echo "web/site.sh: build $build, $(printf '%s\n' "$precached" | wc -l | tr -d ' ') files precached"

# Every relative module reference of a page must resolve: `from "./x"`,
# a side-effect `import "./x"`, a dynamic `import("./x")` and
# `new URL("./x", ...)`, in double or single quotes. Once per subsite: the
# emulator's modules, components and shared code, and the top's scripts.
missing=0
ref_pattern="(from|import *\\(?|new URL\\() *[\"']\\.{1,2}/[^\"']+[\"']"
check_refs() {
  for f in "$@"; do
    [ -f "$f" ] || continue
    dir=$(dirname "$f")
    refs=$(grep -oE "$ref_pattern" "$f" | sed -E "s/.*[\"']([^\"']+)[\"']\$/\\1/") || true
    for ref in $refs; do
      if [ ! -f "$dir/$ref" ]; then
        echo "web/site.sh: ${f#"$out_abs"/} refers to $ref, which is not in the site" >&2
        missing=1
      fi
    done
  done
}
check_refs "$out_abs"/emulator/*.js "$out_abs"/emulator/components/*.js "$out_abs"/emulator/shared/*.js "$out_abs"/emulator/shared/components/*.js
check_refs "$out_abs"/*.js
[ "$missing" = 0 ] || exit 1
ls -lR "$out"
