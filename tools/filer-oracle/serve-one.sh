#!/bin/sh
# One BSD Filer server for one TCP connection: socat runs this with a fresh
# pseudo-terminal as its controlling terminal, so /dev/tty is the serial
# line. BSD Filer's own messages go to the log, not onto the line.
exec /usr/local/bin/bsdfiler -speed "${SPEED:-19200}" -root /srv/lx /dev/tty >>/tmp/bsdfiler.log 2>&1
