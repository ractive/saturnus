#!/bin/bash
# Wait until the Filer oracle container NAME listens on its TCP port.
# Usage: wait-ready.sh NAME [SECONDS]   (default 60)
set -eu
name=$1
limit=${2:-60}
while ((SECONDS < limit)); do
  if docker logs "$name" 2>&1 | grep -q '^bridged'; then
    exit 0
  fi
  if [ "$(docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null)" != true ]; then
    echo "error: container $name stopped:" >&2
    docker logs "$name" >&2 || true
    exit 1
  fi
  sleep 0.5
done
echo "error: container $name not ready after $limit s:" >&2
docker logs "$name" >&2 || true
exit 1
