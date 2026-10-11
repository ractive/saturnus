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
#                       JS, JSON, SVG; not tx.html), the components, the
#                       shared code (web/shared/: its modules, components,
#                       stylesheets and icon sprite), the wasm package
#                       built by web/build.sh, and the installable page's
#                       files from web/pwa/: the manifest, the icons and
#                       the service worker, with index.html given the
#                       manifest link and Apple's tags (web/pwa/head.html).
#   transfer/           saturnus tx, the transfer page: web/tx.html as its
#                       index.html, web/tx/ (its modules and stylesheet,
#                       and its wasm package built by web/tx-build.sh),
#                       the shared code, the favicon, the emulator's wasm
#                       package (pkg/, for the emulated calculator), its
#                       own marker (.saturnus-transfer), manifest and head
#                       tags (web/tx/pwa/), the icons and a service worker
#                       of its own.
#
# Tests, READMEs and build scripts stay out. Each subsite's service worker
# is web/pwa/sw.js behind two constants written here: BUILD, a hash of
# every other file under that subsite (a new deploy is a new cache; the
# landing page and the other subsite are not part of it), and FILES, their
# list relative to the subsite (what it precaches; not the marker). Then
# checks that every relative module import and `new URL(...)` of each
# subsite and at the top names a file in the site, and so does every
# `href` and `src` of each subsite's index.html, so a new module cannot be
# left out.
# Used by .github/workflows/pages.yml for ractive.ch.
#
# `web/site.sh --list` prints the emulator's page files (relative to web/,
# sorted, without the wasm package, the .htaccess, web/pwa/ and
# web/landing/) and copies nothing: the desktop app's build script embeds
# the same set (no service worker, no manifest, no landing page), and the
# saturnus-tauri test `frontend` compares the two. `web/site.sh --list
# transfer` prints the transfer page's files the same way (for its desktop
# app, kb iteration 40).
set -eu
here=$(cd "$(dirname "$0")" && pwd)

# The shared code, relative to web/, one per line (run in web/).
shared_files() {
  for f in shared/*.js shared/*.css shared/*.svg shared/components/*.js; do
    [ -f "$f" ] && printf '%s\n' "$f"
  done
}

# The emulator's page files, relative to web/, one per line, sorted.
page_files() {
  (
    cd "$here"
    for f in ./*.html ./*.css ./*.js ./*.json ./*.svg components/*.js; do
      [ "$f" = ./tx.html ] && continue
      [ -f "$f" ] && printf '%s\n' "${f#./}"
    done
    shared_files
  ) | LC_ALL=C sort
}

# The transfer page's files, relative to web/, one per line, sorted:
# tx.html, the favicon, its modules and stylesheet, the shared code.
transfer_files() {
  (
    cd "$here"
    printf '%s\n' tx.html favicon.svg
    for f in tx/*.js tx/*.css; do
      [ -f "$f" ] && printf '%s\n' "$f"
    done
    shared_files
  ) | LC_ALL=C sort
}

if [ "${1:-}" = "--list" ]; then
  if [ "${2:-}" = transfer ]; then transfer_files; else page_files; fi
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
[ -f "$here/tx/pkg/saturnus_tx_web_bg.wasm" ] || { echo "web/site.sh: run web/tx-build.sh first (no web/tx/pkg)" >&2; exit 1; }
rm -rf "$out"
emu="$out/emulator"
tx="$out/transfer"
mkdir -p "$emu/pkg" "$tx/pkg" "$tx/tx/pkg" "$out/img"
: > "$out/$marker"

# The top: the .htaccess (it applies to the subsites too), the landing
# page, the favicon and the kill switch.
cp "$here/site.htaccess" "$out/.htaccess"
for f in "$here"/landing/*.html "$here"/landing/*.css; do
  cp "$f" "$out/$(basename "$f")"
done
cp "$here"/landing/img/* "$out/img/"
cp "$here/favicon.svg" "$out/favicon.svg"
cp "$here/landing/sw.js" "$out/sw.js"

# Copy the files named on stdin (relative to web/) into directory $1.
copy_into() {
  while IFS= read -r f; do
    mkdir -p "$1/$(dirname "$f")"
    cp "$here/$f" "$1/$f"
  done
}

# The .htaccess's Content-Security-Policy, minus frame-ancestors, as a meta
# tag in each subsite's index.html (for a host that sends no headers). Only
# the site's copies: the desktop app has its own policy (tauri.conf.json).
csp=$(sed -n "s/^Header always set Content-Security-Policy \"\(.*\)\"\$/\1/p" "$here/site.htaccess" | sed "s/; *frame-ancestors [^;]*//")
[ -n "$csp" ] || { echo "web/site.sh: no Content-Security-Policy in site.htaccess" >&2; exit 1; }

# Page $1 as $2/index.html, with the CSP meta and then the head tags in $3
# (the manifest link, the theme colours, Apple's tags).
install_index() {
  sed "s|^<meta charset=\"utf-8\">\$|&\\
<meta http-equiv=\"Content-Security-Policy\" content=\"$csp\">|" "$1" > "$2/index.html"
  grep -q 'http-equiv="Content-Security-Policy"' "$2/index.html" || { echo "web/site.sh: could not add the CSP to $2/index.html" >&2; exit 1; }
  sed "/^<meta http-equiv=\"Content-Security-Policy\"/r $3" "$2/index.html" > "$2/index.html.tmp"
  mv "$2/index.html.tmp" "$2/index.html"
  grep -q '<link rel="manifest"' "$2/index.html" || { echo "web/site.sh: could not add the manifest to $2/index.html" >&2; exit 1; }
}

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

# The service worker of subsite $1: web/pwa/sw.js behind BUILD (a hash
# over every file under $1 but its dotfiles) and FILES (those files).
install_sw() {
  precached=$(cd "$1" && find . -type f ! -name '.*' | sed 's|^\./||' | LC_ALL=C sort)
  # Each file's name and content hash, hashed: a change, a new or a removed file is a new build.
  build=$(cd "$1" && printf '%s\n' "$precached" | while IFS= read -r f; do sha256 "$f"; done | sha256 | cut -c1-16)
  {
    printf '// Written by web/site.sh.\nconst BUILD = "%s";\nconst FILES = [\n' "$build"
    printf '%s\n' "$precached" | sed 's|.*|  "&",|'
    printf '];\n\n'
    cat "$here/pwa/sw.js"
  } > "$1/sw.js"
  echo "web/site.sh: $(basename "$1")/ build $build, $(printf '%s\n' "$precached" | wc -l | tr -d ' ') files precached"
}

# The emulator.
page_files | copy_into "$emu"
cp "$here/pkg/saturnus_web.js" "$here/pkg/saturnus_web_bg.wasm" "$emu/pkg/"
install_index "$here/index.html" "$emu" "$here/pwa/head.html"
mkdir -p "$emu/icons"
cp "$here/pwa/manifest.webmanifest" "$emu/"
cp "$here"/pwa/icons/*.png "$emu/icons/"
install_sw "$emu"

# The transfer page: its files, its wasm package and the emulator's (the
# emulated calculator's Worker, tx/emulator-worker.js, loads ../pkg).
transfer_files | grep -v '^tx\.html$' | copy_into "$tx"
cp "$here/tx/pkg/saturnus_tx_web.js" "$here/tx/pkg/saturnus_tx_web_bg.wasm" "$tx/tx/pkg/"
cp "$here/pkg/saturnus_web.js" "$here/pkg/saturnus_web_bg.wasm" "$tx/pkg/"
: > "$tx/.saturnus-transfer"
install_index "$here/tx.html" "$tx" "$here/tx/pwa/head.html"
mkdir -p "$tx/icons"
cp "$here/tx/pwa/manifest.webmanifest" "$tx/"
cp "$here"/pwa/icons/*.png "$tx/icons/"
install_sw "$tx"

# Every relative module reference of a page must resolve: `from "./x"`,
# a side-effect `import "./x"`, a dynamic `import("./x")` and
# `new URL("./x", ...)`, in double or single quotes. Once per subsite and
# for the top's scripts.
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
check_refs "$out_abs"/transfer/tx/*.js "$out_abs"/transfer/shared/*.js "$out_abs"/transfer/shared/components/*.js
check_refs "$out_abs"/*.js
# And each subsite page's own `href` and `src` attributes (its
# stylesheets, classic scripts, icons, manifest), but not other sites,
# absolute paths, anchors or data: URLs; a folder ("./") must have its
# index.html.
check_html() {
  html_refs=$(grep -oE '(href|src)="[^"]*"' "$1/index.html" | sed -E 's/^[a-z]+="([^"]*)"$/\1/' | sort -u) || true
  for ref in $html_refs; do
    case "$ref" in *:*|/*|"#"*) continue;; esac
    ref=${ref%%[?#]*}
    case "$ref" in */) target="$1/${ref}index.html";; *) target="$1/$ref";; esac
    if [ ! -f "$target" ]; then
      echo "web/site.sh: $(basename "$1")/index.html refers to $ref, which is not in the site" >&2
      missing=1
    fi
  done
}
check_html "$emu"
check_html "$tx"
[ "$missing" = 0 ] || exit 1
ls -lR "$out"
