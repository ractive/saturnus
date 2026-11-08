#!/usr/bin/env bash
# Prints the id of the CI run (ci.yml) of a push to main at commit SHA
# whose `wasm` job succeeded and that still has its `web-pkg` artifact
# (web/pkg as web/build.sh made it, after the web tests passed), or
# nothing. pages.yml and desktop.yml take that package instead of
# building the same commit again; when this prints nothing (CI skipped
# the wasm job by its path filter, is still running, failed, or the
# artifact expired) they build it themselves (kb/docs/ci.md).
#   GH_TOKEN=... GH_REPO=owner/repo scripts/ci-web-pkg-run.sh SHA
# Only pushes to main count: a PR run builds a merge ref, not the commit,
# and its code is not main's. Any API error prints nothing.
set -uo pipefail
sha=${1:?usage: ci-web-pkg-run.sh SHA}
: "${GH_REPO:?GH_REPO (owner/repo) is not set}"

runs=$(gh api "repos/$GH_REPO/actions/workflows/ci.yml/runs?head_sha=$sha&event=push&branch=main&per_page=20" \
  --jq '.workflow_runs[] | select(.head_repository.full_name == "'"$GH_REPO"'") | .id' 2> /dev/null) || exit 0
for id in $runs; do
  wasm=$(gh api "repos/$GH_REPO/actions/runs/$id/jobs?per_page=100" \
    --jq '[.jobs[] | select(.name == "wasm" and .conclusion == "success")] | length' 2> /dev/null) || continue
  [ "${wasm:-0}" -gt 0 ] || continue
  kept=$(gh api "repos/$GH_REPO/actions/runs/$id/artifacts?name=web-pkg" \
    --jq '[.artifacts[] | select(.expired | not)] | length' 2> /dev/null) || continue
  [ "${kept:-0}" -gt 0 ] || continue
  echo "$id"
  exit 0
done
