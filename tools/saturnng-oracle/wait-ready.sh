#!/bin/bash
# Wait until the oracle container NAME has bridged its calculator to TCP,
# which the entrypoint does only once the Kermit server answered.
# Usage: wait-ready.sh NAME [SECONDS]   (default 300: four starts of a
# starved calculator, the entrypoint's READY_ATTEMPTS, fit in it)
# Fails at once, with the container's log, when the container exits (the
# entrypoint gave up after READY_ATTEMPTS starts) or the time runs out.
set -eu
name=$1
limit=${2:-300}
while ((SECONDS < limit)); do
  if docker logs "$name" 2>&1 | grep -q '^bridged'; then
    docker logs "$name" 2>&1 | grep -E '^(calculator ready|warning: start)' || true
    exit 0
  fi
  if [ "$(docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null)" != true ]; then
    echo "error: container $name stopped before its calculator answered:" >&2
    docker logs "$name" 2>&1 | grep -v 'Write into' >&2 || true
    exit 1
  fi
  sleep 1
done
echo "error: container $name not ready after $limit s:" >&2
docker logs "$name" 2>&1 | grep -v 'Write into' >&2 || true
exit 1
