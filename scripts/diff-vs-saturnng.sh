#!/usr/bin/env bash
# Differential screen test: replay the same key script on saturnus and on
# the saturnng oracle (a black box in Docker) and diff the final screens
# (131x64 pixels) and the lit annunciators.
#
# Usage: scripts/diff-vs-saturnng.sh [scenario...]
#   Scenarios live in scripts/scenarios/<name>/keys.txt (saturnus key
#   script format, see README.md). Default: all scenarios.
#   An optional scenarios/<name>/config (shell variables) sets:
#     ORACLE_CARDS=1   start the oracle with its default 128 KB RAM card
#                      in port 1 (default 0: empty slots)
#     SATURNUS_CARD1=1 give saturnus a fresh zeroed 128 KB RAM card in
#                      port 1 (CE1); SATURNUS_CARD2=1 the same in port 2
#                      (CE2). Defaults 0. The oracle's card file is called
#                      "port1", but the 48SX ROM finds it behind CE2 at
#                      #C0000 and reports it as port 2.
#
# Environment:
#   SATURNUS_ROM   48SX ROM for saturnus (default roms/sxrom-j)
#   EMU_DIR        oracle Dockerfile directory (default ~/devel/hptx/emulator)
#   IMAGE          oracle image name (default hp49g-emu; built if missing)
#   OUT_DIR        where both screens are written (default target/diff-vs-saturnng)
#   KEEP=1         keep the oracle containers running for inspection
#   ORACLE_RETRIES fresh oracle replays after a difference (default 1)
#
# The oracle runs with MODEL=48sx AUTOSTART=0 and, unless the scenario
# config says otherwise, CARDS=0: no first-boot automation and empty card
# slots, like saturnus. Between keys it waits
# until its LCD stops changing (5 equal samples 0.3 s apart), the
# wall-clock counterpart of saturnus' `wait-idle`.
#
# Exit status: 0 when every scenario matches, 1 on any screen or annunciator
# difference,
# 2 on a setup error.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SCEN_DIR=$ROOT/scripts/scenarios
SATURNUS_ROM=${SATURNUS_ROM:-$ROOT/roms/sxrom-j}
EMU_DIR=${EMU_DIR:-$HOME/devel/hptx/emulator}
IMAGE=${IMAGE:-hp49g-emu}
OUT_DIR=${OUT_DIR:-$ROOT/target/diff-vs-saturnng}
ORACLE_RETRIES=${ORACLE_RETRIES:-1}
export PATH=$HOME/.rd/bin:$PATH

die() { echo "error: $*" >&2; exit 2; }

command -v docker >/dev/null || die "docker not found (Rancher Desktop: ~/.rd/bin)"
command -v python3 >/dev/null || die "python3 not found"
[ -f "$SATURNUS_ROM" ] || die "ROM $SATURNUS_ROM missing; run: saturnus rom fetch --model 48sx"

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "building oracle image $IMAGE from $EMU_DIR"
  docker build -t "$IMAGE" "$EMU_DIR" || die "docker build failed"
fi

echo "building saturnus"
cargo build --release --quiet -p saturnus-cli --manifest-path "$ROOT/Cargo.toml" \
  || die "cargo build failed"
SATURNUS=$ROOT/target/release/saturnus

# saturnus key name -> saturnng TUI key (tmux key name). The TUI maps each
# letter to the key carrying that alpha label (A-F are the softkeys).
tui_key() {
  case "$1" in
    [a-f]) echo "$1" ;;
    [0-9]) echo "$1" ;;
    mth) echo g ;; prg) echo h ;; cst) echo i ;; var) echo j ;;
    up) echo k ;; nxt) echo l ;; quote) echo m ;; sto) echo n ;;
    eval) echo o ;; left) echo p ;; down) echo q ;; right) echo r ;;
    sin) echo s ;; cos) echo t ;; tan) echo u ;; sqrt) echo v ;;
    power) echo w ;; inv) echo x ;; neg) echo y ;; eex) echo z ;;
    enter) echo Enter ;; backspace) echo BSpace ;;
    alpha) echo ';' ;; leftshift) echo '[' ;; rightshift) echo ']' ;;
    on) echo '\' ;; point) echo . ;; plus) echo + ;; minus) echo - ;;
    multiply) echo '*' ;; divide) echo / ;; space) echo Space ;;
    *) return 1 ;;
  esac
}

# Wait until the oracle LCD shows something and has not changed for
# 5 samples 0.3 s apart (at most ~45 s).
wait_stable() {
  local c=$1 prev="" cur n=0
  for _ in $(seq 1 150); do
    sleep 0.3
    cur=$(docker exec "$c" calc-screen)
    if [ "$cur" = "$prev" ] && grep -q '█' <<<"$cur"; then
      n=$((n + 1)); [ $n -ge 5 ] && return 0
    else
      n=0
    fi
    prev=$cur
  done
  echo "warning: oracle screen did not settle" >&2
}

# Raw TUI pane -> 64 lines of 131 '#'/'.' (the `saturnus --screen x.txt`
# form). The pane has a box: row 0 is the top border, rows 1-64 hold the
# LCD with pixel x in column x+1; column 132 is padding and must be blank.
normalise_pane() {
  python3 -c '
import sys
rows = sys.stdin.read().split("\n")
if len(rows) < 66 or not rows[0].startswith("┌") or not rows[65].startswith("└"):
    sys.exit("unexpected TUI pane layout")
for r in rows[1:65]:
    if len(r) != 134 or r[0] != "│" or r[-1] != "│" or r[132] != " ":
        sys.exit("unexpected LCD row: %r" % r)
    cells = r[1:132]
    if set(cells) - {" ", "█"}:
        sys.exit("unexpected characters in LCD row: %r" % r)
    print(cells.replace("█", "#").replace(" ", "."))
'
}

# Top border of the raw pane -> lit annunciator names, like
# `saturnus run --annunciators`. The border holds six 3-cell slots
# "[ x | x | x | x | x | x ]" in label-strip order: left shift, right
# shift, alpha (all three observed), alert, busy (observed), transmit
# (by position).
pane_annunciators() {
  python3 -c '
import sys
top = sys.stdin.read().split("\n")[0]
start, end = top.find("["), top.find("]")
if start < 0 or end < start:
    sys.exit("no annunciator slots in the TUI border")
slots = top[start + 1:end].split("|")
names = ["leftshift", "rightshift", "alpha", "alert", "busy", "transmit"]
if len(slots) != len(names):
    sys.exit("unexpected annunciator slots: %r" % top[start:end + 1])
lit = [n for n, s in zip(names, slots) if s.strip()]
print(" ".join(lit) if lit else "-")
'
}

# Replay keys.txt on the oracle container $1.
oracle_replay() {
  local c=$1 script=$2 line cmd arg rest key n=0
  while IFS= read -r line || [ -n "$line" ]; do
    n=$((n + 1))
    line=${line%%#*}
    read -r cmd arg rest <<<"$line" || true
    [ -z "${cmd:-}" ] && continue
    cmd=$(tr '[:upper:]' '[:lower:]' <<<"$cmd")
    case "$cmd" in
      wait-idle) wait_stable "$c" ;;
      wait) sleep "$(awk -v ms="${arg%ms}" 'BEGIN { print ms / 1000 }')" ;;
      down|up) die "$script:$n: '$cmd' cannot be replayed on the oracle (tmux sends press+release)" ;;
      *)
        if [ "$cmd" = press ]; then key=$arg; else key=$cmd; fi
        key=$(tr '[:upper:]' '[:lower:]' <<<"$key")
        tk=$(tui_key "$key") || die "$script:$n: no TUI key for '$key'"
        docker exec "$c" calc-keys "$tk"
        wait_stable "$c"
        ;;
    esac
  done <"$script"
}

# First differing row/column of two screen files, 1-based.
first_mismatch() {
  python3 - "$1" "$2" <<'PY'
import sys
a = open(sys.argv[1]).read().split("\n")
b = open(sys.argv[2]).read().split("\n")
for y in range(max(len(a), len(b))):
    ra = a[y] if y < len(a) else ""
    rb = b[y] if y < len(b) else ""
    if ra != rb:
        x = next((i for i in range(max(len(ra), len(rb)))
                  if (ra[i:i+1] or None) != (rb[i:i+1] or None)), 0)
        print(f"first mismatch at row {y + 1}, column {x + 1}")
        break
PY
}

# Start a fresh oracle, replay the scenario, and write saturnng.{pane,txt,ann}.
oracle_run() {
  local name=$1 script=$2 out=$3 c
  c=saturnus-diff-$name-$$-$RANDOM
  CONTAINERS+=("$c")
  docker run --rm -d -e MODEL=48sx -e AUTOSTART=0 -e CARDS="$ORACLE_CARDS" --name "$c" "$IMAGE" >/dev/null \
    || die "cannot start the oracle container"
  for _ in $(seq 1 60); do
    docker logs "$c" 2>&1 | grep -q bridged && break
    sleep 1
  done
  docker logs "$c" 2>&1 | grep -q bridged || die "oracle container $c did not come up"
  oracle_replay "$c" "$script"
  docker exec "$c" calc-screen -a >"$out/saturnng.pane" \
    || die "cannot read the oracle screen for $name"
  normalise_pane <"$out/saturnng.pane" >"$out/saturnng.txt" \
    || die "unexpected oracle screen layout for $name (raw pane in $out/saturnng.pane)"
  pane_annunciators <"$out/saturnng.pane" >"$out/saturnng.ann" \
    || die "unexpected oracle annunciators for $name"
  if [ "${KEEP:-0}" != 1 ]; then docker rm -f "$c" >/dev/null 2>&1 || true; fi
}

CONTAINERS=()
cleanup() {
  [ "${KEEP:-0}" = 1 ] && return
  for c in "${CONTAINERS[@]+"${CONTAINERS[@]}"}"; do
    docker rm -f "$c" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

if [ $# -gt 0 ]; then
  SCENARIOS=("$@")
else
  SCENARIOS=()
  for d in "$SCEN_DIR"/*/; do SCENARIOS+=("$(basename "$d")"); done
fi

failed=0
for name in "${SCENARIOS[@]}"; do
  script=$SCEN_DIR/$name/keys.txt
  [ -f "$script" ] || die "no scenario $name ($script)"
  out=$OUT_DIR/$name
  rm -rf "$out"
  mkdir -p "$out"

  ORACLE_CARDS=0
  SATURNUS_CARD1=0
  SATURNUS_CARD2=0
  if [ -f "$SCEN_DIR/$name/config" ]; then
    # shellcheck source=/dev/null
    . "$SCEN_DIR/$name/config"
  fi
  card_args=()   # a missing card file becomes a fresh zeroed 128 KB card
  if [ "$SATURNUS_CARD1" = 1 ]; then card_args+=(--card1 "$out/card1.img"); fi
  if [ "$SATURNUS_CARD2" = 1 ]; then card_args+=(--card2 "$out/card2.img"); fi

  "$SATURNUS" run --model 48sx --rom "$SATURNUS_ROM" --keys "$script" \
    "${card_args[@]+"${card_args[@]}"}" \
    --screen "$out/saturnus.txt" --annunciators "$out/saturnus.ann" \
    || die "saturnus failed on $name"

  # The oracle's TUI occasionally delivers a key twice (seen once: NO
  # pressed twice at the boot prompt). On a difference, replay on a fresh
  # oracle up to ORACLE_RETRIES times; saturnus is deterministic and is not
  # rerun. Every differing oracle run is kept and reported.
  attempt=0
  while :; do
    attempt=$((attempt + 1))
    oracle_run "$name" "$script" "$out"
    if cmp -s "$out/saturnus.txt" "$out/saturnng.txt" \
       && cmp -s "$out/saturnus.ann" "$out/saturnng.ann"; then
      break
    fi
    [ "$attempt" -gt "$ORACLE_RETRIES" ] && break
    for f in txt ann pane; do mv "$out/saturnng.$f" "$out/saturnng-run$attempt.$f"; done
    echo "$name: oracle run $attempt differs ($(first_mismatch "$out/saturnus.txt" "$out/saturnng-run$attempt.txt"));" \
      "replaying on a fresh oracle (kept as $out/saturnng-run$attempt.txt)"
  done
  [ "$attempt" -gt 1 ] && retry_note=" after $attempt oracle runs" || retry_note=""

  if ! cmp -s "$out/saturnus.ann" "$out/saturnng.ann"; then
    echo "$name: annunciators DIFFERENT$retry_note: saturnng '$(cat "$out/saturnng.ann")', saturnus '$(cat "$out/saturnus.ann")'"
    failed=1
  fi
  if cmp -s "$out/saturnus.txt" "$out/saturnng.txt"; then
    echo "$name: screen match$retry_note (annunciators: $(cat "$out/saturnus.ann"))"
  else
    echo "$name: DIFFERENT$retry_note ($(first_mismatch "$out/saturnus.txt" "$out/saturnng.txt"))"
    diff "$out/saturnng.txt" "$out/saturnus.txt" | head -40 || true
    echo "  screens: $out/saturnng.txt (oracle), $out/saturnus.txt"
    failed=1
  fi
done
exit "$failed"
