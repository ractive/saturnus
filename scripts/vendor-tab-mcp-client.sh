#!/usr/bin/env bash
# Copies tab-mcp's browser library (@ractive-ch/tab-mcp-client) into
# web/vendor/tab-mcp-client/, for the app's "Connect your AI tool"
# (iteration 55). The app has no bundler, so it takes the library's
# self-contained browser build (`cdn/`: the core, the chunks it imports
# with `import()` and the chunk those share, its one dependency inlined)
# and serves it from its own origin. Writes vendor.json: where the files
# came from, the version, and each file's SHA-256. Points
# web/ai-local.js's CONFIRM_MODULE at the confirmation chunk
# (confirm-<hash>.min.js since tab-mcp 0.16), whose name changes with
# the build.
#
# `just vendor-tab-mcp` takes the npm package at the version and integrity
# pinned in the justfile; `just vendor-tab-mcp-version 0.16.1` pins another
# version (with the registry's integrity) and takes it;
# `just vendor-tab-mcp-local` a local build.
#
#   scripts/vendor-tab-mcp-client.sh --from-local ~/devel/tab-mcp
#       builds packages/tab-mcp-client there (`bun run build`) and copies
#       dist/cdn/*.min.js; records the commit (and whether the package's
#       sources had changes not committed).
#   scripts/vendor-tab-mcp-client.sh --from-npm 0.1.0 [--integrity sha512-…]
#       `npm pack @ractive-ch/tab-mcp-client@0.1.0` into a new temporary
#       directory, checks the tarball's SHA-512 against the registry's
#       `dist.integrity` and, when given, against --integrity (pin it in
#       the justfile), then copies package/cdn/*.min.js.
#   --pin (with --from-npm, without --integrity): also writes the version
#       and the registry's integrity into the justfile's
#       tab_mcp_client_version and tab_mcp_client_integrity, once the
#       files are copied.
#   --dry-run: check and list, copy nothing.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
out="$here/web/vendor/tab-mcp-client"
pkg="@ractive-ch/tab-mcp-client"
mode=""
src=""
integrity=""
dry=0
pin=0
while [ $# -gt 0 ]; do
  case "$1" in
    --from-local) mode=local; src=${2:?--from-local needs the tab-mcp checkout}; shift 2;;
    --from-npm) mode=npm; src=${2:?--from-npm needs an exact version}; shift 2;;
    --integrity) integrity=${2:?--integrity needs sha512-…}; shift 2;;
    --pin) pin=1; shift;;
    --dry-run) dry=1; shift;;
    *) echo "vendor-tab-mcp-client: unknown argument $1" >&2; exit 2;;
  esac
done
[ -n "$mode" ] || { echo "usage: $0 --from-local DIR | --from-npm VERSION [--integrity sha512-… | --pin] [--dry-run]" >&2; exit 2; }
if [ "$pin" = 1 ] && { [ "$mode" != npm ] || [ -n "$integrity" ]; }; then
  echo "vendor-tab-mcp-client: --pin goes with --from-npm and without --integrity" >&2
  exit 2
fi

sha256() { if command -v sha256sum > /dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if [ "$mode" = local ]; then
  client="$src/packages/tab-mcp-client"
  [ -f "$client/package.json" ] || { echo "vendor-tab-mcp-client: no $client/package.json" >&2; exit 1; }
  (cd "$client" && bun run build > "$tmp/build.log") || { cat "$tmp/build.log" >&2; exit 1; }
  cdn="$client/dist/cdn"
  version=$(node -p "require('$client/dist/package.json').version")
  commit=$(git -C "$src" rev-parse HEAD)
  dirty=false
  [ -z "$(git -C "$src" status --porcelain -- packages/tab-mcp-client)" ] || dirty=true
  origin="local build of $src/packages/tab-mcp-client at $commit"
  source_json="{\"kind\": \"local\", \"commit\": \"$commit\", \"uncommittedChanges\": $dirty}"
else
  case "$src" in *[!0-9.a-z-]*|"") echo "vendor-tab-mcp-client: an exact version, such as 0.1.0" >&2; exit 2;; esac
  expected=$(npm view "$pkg@$src" dist.integrity)
  [ -n "$expected" ] || { echo "vendor-tab-mcp-client: the registry has no $pkg@$src" >&2; exit 1; }
  case "$expected" in sha512-*) ;; *) echo "vendor-tab-mcp-client: the registry's integrity $expected is not a sha512-…" >&2; exit 1;; esac
  if [ -n "$integrity" ] && [ "$integrity" != "$expected" ]; then
    echo "vendor-tab-mcp-client: the registry's integrity $expected is not the pinned $integrity" >&2
    exit 1
  fi
  mkdir "$tmp/pack" "$tmp/x"
  (cd "$tmp/pack" && npm pack "$pkg@$src" --silent > /dev/null)
  tarball=$(ls "$tmp/pack"/*.tgz)
  actual="sha512-$(openssl dgst -sha512 -binary "$tarball" | openssl base64 -A)"
  [ "$actual" = "$expected" ] || { echo "vendor-tab-mcp-client: the tarball's integrity $actual is not the registry's $expected" >&2; exit 1; }
  tar -xzf "$tarball" -C "$tmp/x"
  cdn="$tmp/x/package/cdn"
  version=$src
  origin="npm $pkg@$src ($actual)"
  source_json="{\"kind\": \"npm\", \"package\": \"$pkg\", \"integrity\": \"$actual\"}"
fi

files=$(cd "$cdn" && ls ./*.min.js | sed 's|^\./||' | LC_ALL=C sort)
[ -n "$files" ] || { echo "vendor-tab-mcp-client: no *.min.js in $cdn" >&2; exit 1; }
echo "$files" | grep -qx 'tab-mcp-client.min.js' || { echo "vendor-tab-mcp-client: no tab-mcp-client.min.js in $cdn" >&2; exit 1; }
# The confirmation that the desktop app imports by name (web/ai-local.js).
confirm=$(echo "$files" | grep -E '^confirm-[0-9a-z]+\.min\.js$' || true)
[ "$(echo "$confirm" | grep -c .)" = 1 ] || { echo "vendor-tab-mcp-client: not exactly one confirm-*.min.js in $cdn (tab-mcp 0.16 on): $confirm" >&2; exit 1; }
echo "$origin, version $version:"
echo "$files" | sed 's/^/  /'
[ "$dry" = 0 ] || exit 0

rm -rf "$out"
mkdir -p "$out"
entries=""
for f in $files; do
  # The source maps stay out (not shipped): no comment pointing at them.
  sed '/^\/\/# sourceMappingURL=/d' "$cdn/$f" > "$out/$f"
  entries="$entries${entries:+,
}    \"$f\": \"$(sha256 "$out/$f")\""
done
cat > "$out/vendor.json" <<EOF
{
  "package": "$pkg",
  "version": "$version",
  "license": "MIT (tab-mcp; @cfworker/json-schema, MIT, inlined)",
  "source": $source_json,
  "entry": "tab-mcp-client.min.js",
  "files": {
$entries
  }
}
EOF
echo "written to web/vendor/tab-mcp-client/"

# CONFIRM_MODULE: the one line that names the chunk (web/test/vendor.test.mjs checks it).
local_js="$here/web/ai-local.js"
grep -qE '^export const CONFIRM_MODULE = "\./vendor/tab-mcp-client/[^"]+";$' "$local_js" || { echo "vendor-tab-mcp-client: no CONFIRM_MODULE line in $local_js" >&2; exit 1; }
sed -E "s|^export const CONFIRM_MODULE = \"\./vendor/tab-mcp-client/[^\"]+\";$|export const CONFIRM_MODULE = \"./vendor/tab-mcp-client/$confirm\";|" "$local_js" > "$tmp/ai-local.js"
cat "$tmp/ai-local.js" > "$local_js"
echo "CONFIRM_MODULE: $confirm"

if [ "$pin" = 1 ]; then
  just="$here/justfile"
  grep -q '^tab_mcp_client_version := "' "$just" && grep -q '^tab_mcp_client_integrity := "' "$just" || { echo "vendor-tab-mcp-client: no tab_mcp_client_version/integrity in $just" >&2; exit 1; }
  sed -E -e "s|^tab_mcp_client_version := \"[^\"]*\"$|tab_mcp_client_version := \"$version\"|" \
    -e "s|^tab_mcp_client_integrity := \"[^\"]*\"$|tab_mcp_client_integrity := \"$actual\"|" "$just" > "$tmp/justfile"
  cat "$tmp/justfile" > "$just"
  echo "pinned in the justfile: $version, $actual"
fi
