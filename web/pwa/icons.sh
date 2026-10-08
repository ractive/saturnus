#!/usr/bin/env sh
# Draws the installed page's icons (web/pwa/icons/*.png, committed) from
# web/logo.svg with headless Chrome: the logo on the panel colour, plain
# (rounded square), maskable (full bleed, the logo inside the safe circle)
# and Apple's touch icon (opaque, square; iOS rounds it). Run it again
# after the logo changes. SATURNUS_CHROME names the Chrome binary.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
chrome=${SATURNUS_CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
bg="#f1efe9"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
logo=$(sed -e '/<title>/d' -e 's/<svg [^>]*>//' -e 's#</svg>##' "$here/../logo.svg")

# icon NAME SIZE SCALE RADIUS: the logo at SCALE of the square, corners of RADIUS (logo units of 32).
icon() {
  cat > "$tmp/$1.svg" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="$2" height="$2">
<rect width="32" height="32" rx="$4" fill="$bg"/>
<g transform="translate(16 16) scale($3) translate(-16 -16)">$logo</g>
</svg>
EOF
  printf '<!doctype html><body style="margin:0;background:transparent"><img src="%s.svg" width="%s" height="%s" style="display:block">' "$1" "$2" "$2" > "$tmp/$1.html"
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
    --force-device-scale-factor=1 --window-size="$2,$2" --screenshot="$here/icons/$1.png" "file://$tmp/$1.html" > /dev/null 2>&1
}

mkdir -p "$here/icons"
icon icon-512 512 0.84 7
icon icon-192 192 0.84 7
icon maskable-512 512 0.62 0
icon maskable-192 192 0.62 0
icon apple-touch-icon 180 0.74 0
ls -l "$here/icons"
