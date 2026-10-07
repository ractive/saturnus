#!/usr/bin/env sh
# Assembles the static site of the web page into the directory given as the
# first argument (default: site/): every page file in web/ (HTML, CSS, JS,
# JSON, SVG), the components, the wasm package built by web/build.sh, and
# the .htaccess for Apache hosts. Tests, READMEs and build scripts stay out.
# Then checks that every relative module import and `new URL(...)` of the
# page names a file in the site, so a new module cannot be left out.
# Used by .github/workflows/pages.yml for GitHub Pages and ractive.ch.
#
# `web/site.sh --list` prints the page files (relative to web/, sorted,
# without the wasm package and the .htaccess) and copies nothing: the
# desktop app's build script embeds the same set, and the saturnus-tauri
# test `frontend` compares the two.
set -eu
here=$(cd "$(dirname "$0")" && pwd)

# The page files, relative to web/, one per line, sorted.
page_files() {
  (
    cd "$here"
    for f in ./*.html ./*.css ./*.js ./*.json ./*.svg components/*.js; do
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
case "$out_abs/" in "$here"/*|"$here/") echo "web/site.sh: the target must not be web/ or inside it" >&2; exit 1;; esac
[ -f "$here/pkg/saturnus_web_bg.wasm" ] || { echo "web/site.sh: run web/build.sh first (no web/pkg)" >&2; exit 1; }
rm -rf "$out"
mkdir -p "$out/pkg" "$out/components"
page_files | while IFS= read -r f; do
  cp "$here/$f" "$out/$f"
done
cp "$here/pkg/saturnus_web.js" "$here/pkg/saturnus_web_bg.wasm" "$out/pkg/"
cp "$here/site.htaccess" "$out/.htaccess"

# Every relative module reference of the page must resolve: `from "./x"`,
# a side-effect `import "./x"`, a dynamic `import("./x")` and
# `new URL("./x", ...)`, in double or single quotes.
missing=0
ref_pattern="(from|import *\\(?|new URL\\() *[\"']\\.{1,2}/[^\"']+[\"']"
for f in "$out_abs"/*.js "$out_abs"/components/*.js; do
  dir=$(dirname "$f")
  refs=$(grep -oE "$ref_pattern" "$f" | sed -E "s/.*[\"']([^\"']+)[\"']\$/\\1/") || true
  for ref in $refs; do
    if [ ! -f "$dir/$ref" ]; then
      echo "web/site.sh: ${f#"$out_abs"/} refers to $ref, which is not in the site" >&2
      missing=1
    fi
  done
done
[ "$missing" = 0 ] || exit 1
ls -lR "$out"
