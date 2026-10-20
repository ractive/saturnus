# Wiki audit (Claude Managed Agent)

A read-only audit of [calculator-knowledgebase](https://github.com/ractive/calculator-knowledgebase)
before its content is republished as the HP calculator encyclopedia on
<https://ractive.ch/saturnus/>. It runs as a Claude Managed Agent on Anthropic's servers and is
billed to the owner's API org, not to a Claude Code plan. Backlog: `kb/backlog/encyclopedia.md`.

- `agents/wiki-auditor.md`: the agent (Opus 5.5, web tools off, `always_allow` because the
  sandbox has no network and the job is read-only); the body is the system prompt.
- `environments/wiki-audit.yaml`: cloud sandbox with `limited` networking and no hosts. The
  public repository is mounted read-only at session start, without a GitHub token.
- `rubric.md`: the outcome rubric the grader checks (max 3 iterations).
- `run.sh`: starts one session with a $40 budget and prints its id.
- `claude-lock.json`: ids of the agent and environment, written by `ant apply`.
- `audit-2026-10-20/`: the first report (`audit.md`, `findings.json`; wiki commit 9db1840,
  session `sesn_01QT1db3zz8yxRTDfo9UnsYp`, $6.16).

## Use

Needs the `ant` CLI (`brew install anthropics/tap/ant`) and the profile `wiki-audit`
(`ant auth login --profile wiki-audit`). Use it only through `ANTHROPIC_PROFILE=wiki-audit`;
keep `default` (Claude Code's own) the active profile (`ant profile activate default`), so
Claude Code stays on its own login. Run everything from this directory.

```sh
export ANTHROPIC_PROFILE=wiki-audit; unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN
ant apply --dry-run -v agents/wiki-auditor.md environments/wiki-audit.yaml  # after editing a file
ant apply agents/wiki-auditor.md environments/wiki-audit.yaml               # owner approves the plan
SID=$(./run.sh)
ant beta:sessions retrieve --session-id "$SID" --transform '{status,outcome_evaluations,usage}'
ant beta:files list --scope-id "$SID" --beta managed-agents-2026-04-01 --transform '{id,filename}'
ant beta:files download --file-id <id> --output audit-YYYY-MM-DD/audit.md
```

Budget amounts are in cents as integer strings (`"4000"` is $40). Add `--format-error yaml`
to see the message of a 400.
