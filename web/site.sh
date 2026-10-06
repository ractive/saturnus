#!/usr/bin/env sh
# Assembles the static site of the web page into the directory given as the
# first argument (default: site/): every page file in web/ (HTML, CSS, JS,
# JSON, SVG), the components, the wasm package built by web/build.sh, and
# the .htaccess for Apache hosts. Tests, READMEs and build scripts stay out.
# Used by .github/workflows/pages.yml for GitHub Pages and ractive.ch.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-site}
out_abs=$(mkdir -p "$out" && cd "$out" && pwd)
case "$out_abs/" in "$here"/*|"$here/") echo "web/site.sh: the target must not be web/ or inside it" >&2; exit 1;; esac
[ -f "$here/pkg/saturnus_web_bg.wasm" ] || { echo "web/site.sh: run web/build.sh first (no web/pkg)" >&2; exit 1; }
rm -rf "$out"
mkdir -p "$out/pkg" "$out/components"
for f in "$here"/*.html "$here"/*.css "$here"/*.js "$here"/*.json "$here"/*.svg; do
  [ -e "$f" ] && cp "$f" "$out/"
done
cp "$here"/components/*.js "$out/components/"
cp "$here/pkg/saturnus_web.js" "$here/pkg/saturnus_web_bg.wasm" "$out/pkg/"
cp "$here/site.htaccess" "$out/.htaccess"
ls -lR "$out"
