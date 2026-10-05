#!/usr/bin/env bash
# End-to-end test of the satx binary against a calculator in server mode,
# usually the emulator: SATX_E2E_ADDR=tcp://localhost:4848 scripts/e2e-cli.sh
#
# Re-expresses the satx-core e2e scenarios through the CLI: info, ls, a
# byte-exact binary put/get round trip of all 256 byte values, run, error
# paths, pict, backup, rm, `ls --json | jq`, `repl` from a pipe, and
# the offline commands on what the calculator sent: `object inspect` (walked size = file size),
# `grob to-png`, `object convert` both ways checked through the calculator.
# On a 49G also a list holding a symbolic matrix (iteration 3 regression).
# Never runs restore (it ends server mode). Leaves the calculator in HOME and
# ASCII mode.
#
# The first check is `satx info`: the model it reports must be the expected
# one, or the script stops before touching the calculator, so a wrong
# emulator on the port is not tested by mistake. SATX_E2E_MODEL=48sx|48gx|49g
# sets the expectation; without it the port decides (4848: 48sx, 4850 and
# 4852: 49g, the CI and local conventions); other ports need SATX_E2E_MODEL.
#
# XModem (put/get --protocol xmodem): on a 48G/GX or 49G, with
# SATX_E2E_CONTAINER naming the emulator's Docker container, the script types
# XRECV/XSEND and SERVER on the calculator with `docker exec CONTAINER
# calc-keys`; without it these steps print "skipped". On the 48S/SX the
# script checks that XModem is refused.
#
# SATX_BIN overrides the binary (default: build target/debug/satx).
set -euo pipefail

addr=${SATX_E2E_ADDR:?set SATX_E2E_ADDR, e.g. tcp://localhost:4848}
root=$(cd "$(dirname "$0")/.." && pwd)
if [[ -z ${SATX_BIN:-} ]]; then
    cargo build -q -p satx-cli --manifest-path "$root/Cargo.toml"
    SATX_BIN=$root/target/debug/satx
fi
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
container=${SATX_E2E_CONTAINER:-}

expected=${SATX_E2E_MODEL:-}
if [[ -z $expected ]]; then
    case ${addr##*:} in
        4848) expected=48sx ;;
        4850 | 4852) expected=49g ;;
        *)
            echo "set SATX_E2E_MODEL=48sx|48gx|49g for $addr" >&2
            exit 1
            ;;
    esac
fi
case $(tr '[:upper:]' '[:lower:]' <<<"$expected") in
    48sx) expected_model="HP 48S/SX" ;;
    48gx) expected_model="HP 48G/GX" ;;
    49g) expected_model="HP 49G" ;;
    *)
        echo "SATX_E2E_MODEL=$expected: use 48sx, 48gx or 49g" >&2
        exit 1
        ;;
esac

export SATX_PORT=$addr
work=$(mktemp -d)
passed=0
# Set once `info` confirmed the expected model; cleanup leaves any other
# calculator alone.
verified=0
# Set while an XModem step may have left the calculator out of server mode.
server_down=0
# Set when the script switched a 49G to RPN for XModem.
xm_switched=0
# Set while PICT holds the e2e drawing.
pict_drawn=0

satx() { "$SATX_BIN" "$@"; }
# Object bytes without the 8-byte HPHP4x-x header.
body() { tail -c +9 "$1"; }
size() { wc -c <"$1" | tr -d ' '; }
# Assert `object inspect` walks FILE to its exact size: no padding, no gap.
walks_whole_file() {
    local info
    info=$(satx object inspect "$1" --json)
    jq -e --argjson n "$(size "$1")" \
        '.results[0] | .size_bytes + 8 == $n and .padding_bytes == 0' <<<"$info" >/dev/null \
        || fail "object inspect $1 ($(size "$1") bytes): $info"
    jq -r '.results[0].type' <<<"$info"
}
step() { printf '%-44s' "$1"; }
ok() { passed=$((passed + 1)); echo "ok${1:+  $1}"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
skip() { echo "skipped: $*"; }

# Press keys on the emulated calculator (`;` is ALPHA, `\` is ON).
keys() { docker exec "$container" calc-keys "$@" >/dev/null; }
# Type a lowercase word in alpha-lock, then ENTER. ON first: it clears a
# command line and an alpha mode left over (the 49G can come out of the
# Kermit server with alpha still on, and ;; would then switch it off).
type_word() {
    local -a letters
    local i
    for ((i = 0; i < ${#1}; i++)); do letters+=("${1:i:1}"); done
    keys "\\"
    sleep 0.5
    keys ';' ';' "${letters[@]}" Enter
}
# Cancel whatever runs (ON) and start the Kermit server again.
restart_server() {
    sleep 2
    keys "\\"
    sleep 1
    type_word server
    sleep 2
    server_down=0
}
# Run `satx ARGS --format text` in the background; once it prints the
# instructions (server ended), type WORD on the calculator. Fails unless
# satx succeeds. The calculator is out of server mode afterwards.
xmodem_run() {
    local word=$1
    shift
    server_down=1
    satx "$@" --format text >"$work/xm.out" 2>"$work/xm.err" &
    local pid=$! i
    for ((i = 0; i < 300; i++)); do
        grep -q "On the calculator, type" "$work/xm.err" && break
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    grep -q "On the calculator, type" "$work/xm.err" \
        || { wait "$pid" || true; fail "no instructions from satx: $(cat "$work/xm.err")"; }
    sleep 1
    type_word "$word"
    wait "$pid" || fail "satx $*: $(cat "$work/xm.err")"
}

cleanup() {
    local status=$?
    if ((!verified)); then
        rm -rf "$work"
        exit "$status"
    fi
    if ((server_down)) && [[ -n $container ]]; then
        echo "cleanup: restarting SERVER on the calculator" >&2
        restart_server || true
    fi
    if ((xm_switched)); then satx run -95 SF >/dev/null 2>&1 || true; fi
    if ((pict_drawn)); then satx run ERASE >/dev/null 2>&1 || true; fi
    # Best effort: remove what this script may have created.
    local names
    names=$(satx --dir HOME ls --jq '.results[].name' 2>/dev/null || true)
    for n in SATXCLI SATXPT SATXSM SATXCD SATXCE SATXGR SATXG2 SATXCV SATXCW SATXCX SATXXM SATXR; do
        if grep -qx "$n" <<<"$names"; then
            echo "cleanup: removing $n" >&2
            satx rm "$n" >/dev/null 2>&1 || true
        fi
    done
    satx settings --mode ascii >/dev/null 2>&1 || true
    rm -rf "$work"
    exit "$status"
}
trap cleanup EXIT

echo "satx e2e against $addr, expecting the $expected_model"

step "info: the expected model"
info=$(satx info --json)
model=$(jq -r '.results.model' <<<"$info")
[[ $model == "$expected_model" ]] \
    || fail "$addr answers as the $model, expected the $expected_model (SATX_E2E_MODEL=${SATX_E2E_MODEL:-unset}); is another emulator on this port? Nothing was changed."
verified=1
info=$(satx --dir HOME info --json)
[[ $(jq -r '.results.path | join("/")' <<<"$info") == HOME ]] || fail "info: path is not HOME: $info"
jq -e '.results.free_bytes > 0' <<<"$info" >/dev/null || fail "info: free memory: $info"
jq -e '.results.iopar.baud == 9600' <<<"$info" >/dev/null || fail "info: IOPAR: $info"
ok "$model"

step "ls"
listing=$(satx ls --json)
jq -e '.results | map(.name) | index("IOPAR")' <<<"$listing" >/dev/null || fail "ls: no IOPAR: $listing"
ok "$(jq -r '.total' <<<"$listing") variables"

step "ls --json | jq '.total' matches the listing"
total=$(satx ls --json | jq '.total')
[[ $total =~ ^[0-9]+$ && $total == $(jq '.results | length' <<<"$listing") ]] || fail "total: $total"
[[ $(satx ls --jq '.total') == "$total" ]] || fail "--jq .total"
ok "$total"

step "get IOPAR (binary header)"
satx get IOPAR -o "$work/iopar.hp" --json >/dev/null
header=$(head -c 8 "$work/iopar.hp")
[[ $header == HPHP4[89]-? ]] || fail "IOPAR header: $header"
ok "$header"

step "put + get all 256 byte values, byte-exact"
# A String object (prolog 02A2C, length 5 + 2*256 nibbles = 0x00205),
# packed low nibble first: 2C 2A 50 20 00, then the bytes 00..FF.
{
    head -c 8 "$work/iopar.hp"
    printf '\x2c\x2a\x50\x20\x00'
    # shellcheck disable=SC2059 # the octal escape is the format on purpose
    for i in $(seq 0 255); do printf "\\$(printf %03o "$i")"; done
} >"$work/all.hp"
[[ $(wc -c <"$work/all.hp" | tr -d ' ') == 269 ]] || fail "test file size"
stored=$(satx put "$work/all.hp" --as SATXCLI --json | jq -r '.results.name')
[[ $stored == SATXCLI ]] || fail "stored as $stored"
satx get SATXCLI -o "$work/back.hp" --json >/dev/null
cmp "$work/all.hp" "$work/back.hp" || fail "round trip differs"
[[ $(walks_whole_file "$work/back.hp") == String ]] || fail "inspect type"
ok "269 bytes, inspect: String"

step "put refuses an existing name"
if out=$(satx put "$work/all.hp" --as SATXCLI --json 2>&1); then
    fail "put over an existing name succeeded: $out"
fi
jq -e '.hint | test("--overwrite")' <<<"$out" >/dev/null || fail "no --overwrite hint: $out"
ok

step "put --overwrite --dry-run"
dry=$(satx put "$work/all.hp" --as SATXCLI --overwrite --dry-run --json)
jq -e '.results.dry_run and .results.replaces.type == "String"' <<<"$dry" >/dev/null || fail "$dry"
grep -q SATXPT <<<"$(satx put "$work/all.hp" --as SATXCLI --overwrite --dry-run --format text)" \
    || fail "dry-run text does not name SATXPT"
ok

step "put --overwrite replaces via SATXPT"
# Different content (IOPAR's list) so the replacement is visible.
replaced=$(satx put "$work/iopar.hp" --as SATXCLI --overwrite --json)
jq -e '.results.name == "SATXCLI" and .results.replaced.type == "String"' <<<"$replaced" \
    >/dev/null || fail "$replaced"
names=$(satx ls --jq '.results[].name')
[[ $(grep -cx SATXCLI <<<"$names") == 1 ]] || fail "not exactly one SATXCLI: $names"
if grep -qx SATXPT <<<"$names"; then fail "SATXPT left over"; fi
satx get SATXCLI -o "$work/replaced.hp" --json >/dev/null
cmp <(body "$work/iopar.hp") <(body "$work/replaced.hp") || fail "replaced content differs"
ok

step "get of a missing name fails with a hint"
if out=$(satx get SATXNOSUCH -o "$work/x" --json 2>&1); then
    fail "get of a missing variable succeeded"
fi
jq -e '.error and .hint' <<<"$out" >/dev/null || fail "no error/hint: $out"
ok "$(jq -r '.error' <<<"$out")"

step "run '6 7 *'"
out=$(satx run '6 7 *' --format text)
satx run DROP >/dev/null
grep -q '42' <<<"$out" || fail "run: $out"
ok "$(tr '\n' ' ' <<<"$out")"

step "run with a calculator error"
if out=$(satx run "'SATXNOSUCH' RCL" --json 2>&1); then
    fail "run of an error succeeded"
fi
satx run DROP >/dev/null
jq -e '.error | test("Undefined Name")' <<<"$out" >/dev/null || fail "$out"
ok

step "run of a too long command"
if out=$(satx run "$(printf '1%.0s' $(seq 80))" --json 2>&1); then
    fail "too long command succeeded"
fi
jq -e '.error | test("too long")' <<<"$out" >/dev/null || fail "$out"
ok

step "mkdir, mv, ls PATH, --dir, rm"
satx mkdir SATXCD --json >/dev/null
satx mv SATXCD SATXCE --json >/dev/null
[[ $(satx ls HOME/SATXCE --jq '.total') == 0 ]] || fail "ls HOME/SATXCE not empty"
satx --dir HOME ls --jq '.results[] | select(.name == "SATXCE") | .directory' | grep -qx true \
    || fail "SATXCE is not a directory"
satx rm SATXCE --json >/dev/null
ok

step "pict (PICT as PNG, 131x64)"
# A fresh PICT is 0x0; ERASE makes it 131x64. Draw one pixel, fetch it.
pict_drawn=1
satx run 'ERASE { # 10d # 10d } PIXON' --json >/dev/null
pict=$(satx pict -o "$work/pict.png" --json)
jq -e '.results.width == 131 and .results.height == 64' <<<"$pict" >/dev/null || fail "$pict"
[[ $(head -c 8 "$work/pict.png" | od -An -tx1 | tr -d ' \n') == 89504e470d0a1a0a ]] \
    || fail "not a PNG"
# IHDR width and height, big-endian, right after the signature and chunk header.
[[ $(head -c 24 "$work/pict.png" | tail -c 8 | od -An -tx1 | tr -d ' \n') == 0000008300000040 ]] \
    || fail "PNG is not 131x64"
if satx ls --jq '.results[].name' | grep -qx SATXTMP; then fail "SATXTMP left over"; fi
satx run ERASE --json >/dev/null
pict_drawn=0
ok "$(wc -c <"$work/pict.png" | tr -d ' ') bytes"

step "backup (HPHP4 header)"
satx backup -o "$work/home.hp" --json >/dev/null
[[ $(head -c 5 "$work/home.hp") == HPHP4 ]] || fail "backup header"
satx restore "$work/home.hp" --dry-run --json | jq -e '.results.dry_run' >/dev/null \
    || fail "restore --dry-run"
ok "$(wc -c <"$work/home.hp" | tr -d ' ') bytes"

if [[ $model == "HP 49G" ]]; then
    step "49G: list with a symbolic matrix"
    satx run "{ [[ 'X' 1 ]] 2 } 'SATXSM' STO" --json >/dev/null
    type=$(satx get SATXSM -o "$work/sm1.hp" --jq '.results.type')
    [[ $type == List ]] || fail "type $type"
    satx get SATXSM -o "$work/sm2.hp" --json >/dev/null
    cmp "$work/sm1.hp" "$work/sm2.hp" || fail "two gets differ"
    size=$(size "$work/sm1.hp")
    ((size > 8)) || fail "empty object"
    [[ $(walks_whole_file "$work/sm1.hp") == List ]] || fail "inspect type"
    # A symbolic matrix has no text form in satx: refused, naming the type.
    if out=$(satx object convert "$work/sm1.hp" --to ascii -o "$work/sm1.txt" --json 2>&1); then
        fail "convert of a symbolic matrix succeeded"
    fi
    jq -e '.error | test("Symbolic Matrix")' <<<"$out" >/dev/null || fail "$out"
    satx rm SATXSM --json >/dev/null
    ok "$size bytes, inspect: walked size = file size"
fi

if [[ $model == "HP 48S/SX" ]]; then
    step "xmodem: refused on the 48S/SX"
    if out=$(satx put "$work/all.hp" --as SATXXM --protocol xmodem --json 2>&1); then
        fail "put --protocol xmodem succeeded on the 48S/SX: $out"
    fi
    jq -e '(.error | test("no XModem")) and (.hint | test("Kermit"))' <<<"$out" >/dev/null \
        || fail "$out"
    satx ls --json >/dev/null || fail "server not running after the refusal"
    ok
elif [[ -z $container ]]; then
    step "xmodem put/get"
    skip "set SATX_E2E_CONTAINER to the emulator's container to type on the calculator"
else
    step "xmodem: put --dry-run keeps the server"
    dry=$(satx put "$work/all.hp" --as SATXXM --protocol xmodem --dry-run --json)
    jq -e --arg keys "'SATXXM' XRECV" \
        '.results.dry_run and .results.keys == $keys and .results.bytes == 269' \
        <<<"$dry" >/dev/null || fail "$dry"
    satx ls --json >/dev/null || fail "server not running after --dry-run"
    if satx ls --jq '.results[].name' | grep -qx SATXXM; then fail "dry run stored SATXXM"; fi
    ok "$(jq -r '.results.model' <<<"$dry")"

    if [[ $model == "HP 49G" ]]; then
        # Typed commands need RPN mode.
        if [[ $(satx run -95 'FS?' --jq '.results.stack[0]') == 1* ]]; then
            satx run -95 CF --json >/dev/null
            xm_switched=1
        fi
        satx run DROP --json >/dev/null
    fi

    step "xmodem: put (XRECV), Kermit get byte-exact"
    # As tests/e2e.rs: the name goes on the stack over Kermit, XRECV is typed.
    satx run "'SATXXM'" --json >/dev/null
    xmodem_run xrecv put "$work/all.hp" --as SATXXM --protocol xmodem --start-timeout 60
    grep -q "Type SERVER" "$work/xm.out" || fail "no SERVER note: $(cat "$work/xm.out")"
    restart_server
    satx get SATXXM -o "$work/xm-kermit.hp" --json >/dev/null
    cmp "$work/all.hp" "$work/xm-kermit.hp" || fail "XRECV stored something else"
    ok "$(head -1 "$work/xm.out" | sed 's/.*(//; s/)//')"

    step "xmodem: get (XSEND) byte-exact"
    satx run "'SATXXM'" --json >/dev/null
    xmodem_run xsend get SATXXM -o "$work/xm-back.hp" --protocol xmodem --start-timeout 60
    restart_server
    cmp "$work/all.hp" "$work/xm-back.hp" || fail "XSEND gave something else"
    ok "$(head -1 "$work/xm.out" | sed 's/.*(//; s/)//')"

    step "xmodem: put refuses an existing name"
    if out=$(satx put "$work/all.hp" --as SATXXM --protocol xmodem --json 2>&1); then
        fail "put --protocol xmodem over an existing name succeeded: $out"
    fi
    jq -e '.hint | test("satx rm SATXXM")' <<<"$out" >/dev/null || fail "$out"
    satx rm SATXXM --json >/dev/null
    if [[ $model == "HP 49G" ]]; then
        # SERVER typed in RPN mode on the 49G leaves a tagged `SERVER` and
        # NOVAL on the stack (not in algebraic mode, not on the 48GX).
        satx run CLEAR --json >/dev/null
    fi
    if ((xm_switched)); then
        satx run -95 SF --json >/dev/null
        xm_switched=0
    fi
    ok
fi

step "grob to-png of a stored LCD\\-> GROB"
satx run "LCD\\-> 'SATXGR' STO" --json >/dev/null
satx get SATXGR -o "$work/gr.hp" --json >/dev/null
[[ $(walks_whole_file "$work/gr.hp") == Graphic ]] || fail "inspect type"
png=$(satx grob to-png "$work/gr.hp" --json)
jq -e '.results.width == 131 and .results.height == 64' <<<"$png" >/dev/null || fail "$png"
[[ $(head -c 8 "$work/gr.png" | od -An -tx1 | tr -d ' \n') == 89504e470d0a1a0a ]] || fail "not a PNG"
ok "131x64"

step "object convert GROB via the calculator"
satx object convert "$work/gr.hp" --to ascii -o "$work/gr.txt" --json >/dev/null
satx put "$work/gr.txt" --as SATXG2 --json | jq -e '.results.mode == "ascii"' >/dev/null || fail "put ascii"
satx get SATXG2 -o "$work/g2.hp" --json >/dev/null
cmp <(body "$work/gr.hp") <(body "$work/g2.hp") || fail "GROB differs after the ASCII round trip"
satx object convert "$work/gr.txt" --to binary -o "$work/gr2.hp" --json >/dev/null
cmp <(body "$work/gr.hp") <(body "$work/gr2.hp") || fail "GROB text compiles differently"
satx grob to-png "$work/gr.txt" -o "$work/gr-txt.png" --json >/dev/null
cmp "$work/gr.png" "$work/gr-txt.png" || fail "PNG from text differs"
satx rm SATXGR SATXG2 --json >/dev/null
ok

step "object convert list via the calculator"
# Reals, a complex, a binary integer, an empty list and a string holding a
# quote, a backslash, a LF and character 141 (→).
satx run '{ -1.23E-15 (1.5,-2.) # FFh 123456.5 { } }' --json >/dev/null
satx run '"q" 34 CHR + 92 CHR + 10 CHR + 141 CHR + +' "'SATXCV' STO" --json >/dev/null
satx get SATXCV -o "$work/cv.hp" --json >/dev/null
[[ $(walks_whole_file "$work/cv.hp") == List ]] || fail "inspect type"
# Binary -> satx text -> calculator compiles it -> same object.
satx object convert "$work/cv.hp" --to ascii -o "$work/cv.txt" --json >/dev/null
satx put "$work/cv.txt" --as SATXCW --json >/dev/null
satx get SATXCW -o "$work/cw.hp" --json >/dev/null
cmp <(body "$work/cv.hp") <(body "$work/cw.hp") || fail "satx text compiles to another object on the calculator"
# The calculator's own text -> satx compiles it -> same object.
family=48
[[ $model == "HP 49G" ]] && family=49
satx get SATXCV --ascii -o "$work/cv-calc.txt" --json >/dev/null
satx object convert "$work/cv-calc.txt" --to binary --model "$family" -o "$work/cv2.hp" --json >/dev/null
cmp <(body "$work/cv.hp") <(body "$work/cv2.hp") || fail "calculator text compiles differently in satx"
# satx's binary file (its own header) is accepted by the calculator.
satx put "$work/cv2.hp" --as SATXCX --json >/dev/null
satx get SATXCX -o "$work/cx.hp" --json >/dev/null
cmp <(body "$work/cv.hp") <(body "$work/cx.hp") || fail "satx binary file stored differently"
satx rm SATXCV SATXCW SATXCX --json >/dev/null
ok "$(tr -d '\r' <"$work/cv.txt" | tail -n +2 | tr '\n' ' ' | cut -c1-60)"

step "rm --dry-run, rm"
satx rm SATXCLI --dry-run --json | jq -e '.results[0].deleted == false' >/dev/null || fail "dry run"
satx rm SATXCLI --json >/dev/null
if satx ls --jq '.results[].name' | grep -qx SATXCLI; then fail "SATXCLI still there"; fi
ok

step "repl: piped RPL and colon commands"
# One link for the whole session: store, recall, list, delete.
out=$(printf "42 'SATXR' STO\nSATXR\n:ls\n:rm SATXR\n" | satx repl 2>"$work/repl.err") \
    || fail "repl exit $?: $(cat "$work/repl.err")"
satx run DROP --json >/dev/null
[[ ! -s $work/repl.err ]] || fail "repl stderr: $(cat "$work/repl.err")"
[[ $(grep -cx '1: 42' <<<"$out") == 1 ]] || fail "not one '1: 42': $out"
grep -Eq '^  SATXR +[0-9.]+ ' <<<"$out" || fail ":ls has no SATXR: $out"
grep -q '^Deleted SATXR ' <<<"$out" || fail ":rm: $out"
if satx ls --json | jq -e '.results | map(.name) | index("SATXR")' >/dev/null; then
    fail "SATXR left after :rm"
fi
ok

step "repl: errors print and the session goes on"
out=$(printf "'SATXNOSUCH' RCL\nDROP\n:nosuch\n6 7 *\nDROP\n" | satx repl 2>"$work/repl.err") \
    || fail "repl exit $?: $(cat "$work/repl.err")"
grep -q 'Undefined Name' "$work/repl.err" || fail "no calculator error: $(cat "$work/repl.err")"
grep -q 'unknown command :nosuch' "$work/repl.err" || fail "no hint: $(cat "$work/repl.err")"
grep -qx '1: 42' <<<"$out" || fail "no result after the errors: $out"
if satx repl --json </dev/null 2>/dev/null; then fail "repl --json accepted"; fi
if satx --port tcp://127.0.0.1:1 repl </dev/null 2>/dev/null; then
    fail "repl on a dead link exited 0"
fi
ok

step "settings --mode ascii"
[[ $(satx settings --mode ascii --jq '.results.transfer_mode') == ascii ]] || fail "mode"
ok

echo "e2e-cli: $passed checks passed ($model, $addr)"
