#!/usr/bin/env bash
# End-to-end test of `satx lx` against a Filer server, usually BSD Filer
# in Docker (tools/filer-oracle, `just lx-oracle-up`):
#   FILER_E2E_ADDR=tcp://localhost:4853 scripts/satx-lx-e2e.sh
#
# drives, ls (root, a directory, a pattern, one file, a missing one),
# get of the seeded files byte for byte with their date, mkdir, put of
# every byte value and back, put refused on an existing name and its
# --dry-run, mv into the root, rm --dry-run and rm. Everything it makes
# has a name of this run (LXE plus five digits). BSD Filer has no rmdir
# (0x0E) and keeps the directories; FILER_E2E_RMDIR=1 also removes them,
# for a server that has it.
#
# SATX_BIN overrides the binary (default: build it).
set -euo pipefail

addr=${FILER_E2E_ADDR:?set FILER_E2E_ADDR, e.g. tcp://localhost:4853}
root=$(cd "$(dirname "$0")/.." && pwd)
if [[ -z ${SATX_BIN:-} ]]; then
    cargo build -q --locked -p satx --manifest-path "$root/Cargo.toml"
    SATX_BIN=${CARGO_TARGET_DIR:-$root/target}/debug/satx
fi
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
lx() { "$SATX_BIN" --port "$addr" lx "$@"; }
pass=0
ok() { pass=$((pass + 1)); echo "ok   $*"; }
fail() { echo "FAIL $*" >&2; exit 1; }

id=$(printf '%05d' $((RANDOM * 3 % 100000)))
dir="LXE$id"

# drives
lx drives --json | jq -e '.results | map(.drive) | index("C") != null' >/dev/null \
    || fail "drives lists C:"
ok drives

# ls
out=$(lx ls --json)
jq -e '.results[] | select(.name == "HELLO.TXT") | .size == 30 and .modified == "2004-12-14T13:49:36"' \
    <<<"$out" >/dev/null || fail "ls: HELLO.TXT, 30 bytes, 2004-12-14 13:49:36: $out"
jq -e '.results[] | select(.name == "SUB") | .directory' <<<"$out" >/dev/null || fail "ls: SUB"
jq -e '[.results[].name] | index(".") == null and index("..") == null' <<<"$out" >/dev/null \
    || fail "ls: no . or .."
[[ $(lx ls C:/SUB --json | jq -r '[.results[].name] | join(",")') == NOTE.TXT ]] || fail "ls C:/SUB"
[[ $(lx ls 'C:\*.TXT' --json | jq -r '[.results[].name] | join(",")') == HELLO.TXT ]] || fail "ls pattern"
[[ $(lx ls c:/hello.txt --json | jq '.total') == 1 ]] || fail "ls one file"
if lx ls C:/NOSUCH.TXT --json 2>err.json; then fail "ls of a missing file"; fi
jq -e '.error | test("no such file")' err.json >/dev/null || fail "ls missing: $(cat err.json)"
ok ls

# get
perl -e 'print map { chr($_ % 256) } 0..4999' >expect.bin
lx get C:/BYTES.BIN -o bytes.bin --json >/dev/null
cmp bytes.bin expect.bin || fail "get BYTES.BIN"
lx get C:/HELLO.TXT --json >/dev/null
[[ $(cat HELLO.TXT) == $'Hello from the filer oracle.\r' ]] || fail "get HELLO.TXT"
[[ $(date -r HELLO.TXT +%Y%m%d%H%M%S) == 20041214134936 ]] || fail "HELLO.TXT keeps its date"
if lx get C:/HELLO.TXT --json 2>/dev/null; then fail "get over an existing local file"; fi
lx get C:/HELLO.TXT --force --json >/dev/null
lx get C:/EMPTY.DAT --json >/dev/null
[[ -f EMPTY.DAT && ! -s EMPTY.DAT ]] || fail "get EMPTY.DAT"
[[ $(lx get C:/SUB/NOTE.TXT -o - --json) == $'in a subdirectory\r' ]] || fail "get to stdout"
ok get

# mkdir, put, get back
lx mkdir "C:/$dir" --json >/dev/null
if lx mkdir "C:/$dir" --json 2>/dev/null; then fail "mkdir of an existing directory"; fi
perl -e 'print map { chr($_ * 7 % 256) } 0..4499' >data.bin
lx put data.bin "C:/$dir" --json | jq -e ".results.path == \"C:\\\\$dir\\\\DATA.BIN\"" >/dev/null \
    || fail "put into a directory"
lx get "C:/$dir/DATA.BIN" -o back.bin --json >/dev/null
cmp back.bin data.bin || fail "put/get round trip"
if lx put data.bin "C:/$dir" --json 2>err.json; then fail "put over an existing name"; fi
jq -e '.error | test("exists")' err.json >/dev/null || fail "put refusal: $(cat err.json)"
lx put data.bin "C:/$dir/DATA.BIN" --dry-run --json | jq -e '.results.exists' >/dev/null \
    || fail "put --dry-run sees the file"
ok put

# mv, rm
lx mv "C:/$dir/DATA.BIN" "C:/$dir.BIN" --json >/dev/null
[[ $(lx ls "C:/$dir.BIN" --json | jq '.results[0].size') == 4500 ]] || fail "mv"
[[ $(lx ls "C:/$dir" --json | jq '.total') == 0 ]] || fail "mv left the directory empty"
if lx mv "C:/$dir.BIN" C:/HELLO.TXT --json 2>/dev/null; then fail "mv onto an existing name"; fi
lx rm "C:/$dir.BIN" --dry-run --json | jq -e '.results.dry_run and (.results.deleted | length) == 0' \
    >/dev/null || fail "rm --dry-run"
lx ls "C:/$dir.BIN" --json >/dev/null || fail "rm --dry-run deleted"
if lx rm "C:/$dir.BIN" C:/NOSUCH.TXT --json 2>/dev/null; then fail "rm with a missing name"; fi
lx ls "C:/$dir.BIN" --json >/dev/null || fail "rm with a missing name deleted"
lx rm "C:/$dir.BIN" --json >/dev/null
if lx ls "C:/$dir.BIN" --json 2>/dev/null; then fail "rm"; fi
ok "mv rm"

if [[ ${FILER_E2E_RMDIR:-0} == 1 ]]; then
    lx rmdir "C:/$dir" --json >/dev/null
    if lx ls "C:/$dir" --json 2>/dev/null | jq -e '.total > 0' >/dev/null; then fail "rmdir"; fi
    ok rmdir
fi

echo "satx lx e2e: $pass groups passed against $addr"
