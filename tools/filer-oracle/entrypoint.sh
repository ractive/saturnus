#!/bin/sh
# Fills the served folder (/srv/lx, the palmtop's C:) with the test files
# and bridges TCP port 4853 to BSD Filer: every connection gets its own
# pseudo-terminal and its own BSD Filer process (serve-one), so a client
# that leaves a session half-open does not wedge the next one.
set -eu
mkdir -p /srv/lx/sub
# BSD Filer shows only names that are valid 8.3 and in lower case.
printf 'Hello from the filer oracle.\r\n' > /srv/lx/hello.txt
# 5000 bytes, every value 0-255 (DLE doubling and three 2 KB blocks).
perl -e 'print map { chr($_ % 256) } 0..4999' > /srv/lx/bytes.bin
printf 'in a subdirectory\r\n' > /srv/lx/sub/note.txt
: > /srv/lx/empty.dat
touch -d '2004-12-14 13:49:36' /srv/lx/hello.txt
echo "serving /srv/lx at $SPEED baud"
trap 'kill "$SOCAT" 2>/dev/null || true; exit 0' TERM INT
socat -d TCP-LISTEN:4853,reuseaddr,fork EXEC:/usr/local/bin/serve-one,pty,setsid,ctty,raw,echo=0 &
SOCAT=$!
# Say `bridged` once socat listens (port 4853 = 0x12F5, state 0A = LISTEN).
for _ in $(seq 1 100); do
  if awk '$2 ~ /:12F5$/ && $4 == "0A" { found = 1 } END { exit !found }' /proc/net/tcp; then
    break
  fi
  sleep 0.1
done
echo "bridged bsdfiler to tcp:4853"
wait "$SOCAT"
