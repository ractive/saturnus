#!/usr/bin/env bash
# Start one audit session on the API credits of the wiki-audit profile.
set -euo pipefail
cd "$(dirname "$0")"
export ANTHROPIC_PROFILE=wiki-audit
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN
AGENT_ID=$(jq -r '.resources["./agents/wiki-auditor.md"].id' claude-lock.json)
ENV_ID=$(jq -r '.resources["./environments/wiki-audit.yaml"].id' claude-lock.json)
RUBRIC=$(jq -Rs . rubric.md)
ant beta:sessions create \
  --agent "$AGENT_ID" --environment-id "$ENV_ID" \
  --title "Wiki audit $(date +%F)" \
  --resource '{type: github_repository, url: "https://github.com/ractive/calculator-knowledgebase"}' \
  --budget '{type: limit, max_list_cost: {amount: "4000", currency: USD}}' \
  --initial-event "{type: user.define_outcome, description: \"Audit the wiki as the system prompt describes and write audit.md and findings.json.\", max_iterations: 3, rubric: {type: text, content: $RUBRIC}}" \
  --transform id -r --format-error yaml
