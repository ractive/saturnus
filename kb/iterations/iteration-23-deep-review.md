---
type: iteration
title: "Iteration 23: Deep review of the whole codebase before v0.1.0"
date: 2026-10-07
status: planned
tags:
  - iteration
  - saturnus
branch: iter-23/deep-review
---

# Iteration 23: Deep review of the whole codebase before v0.1.0

Owner (2026-10-07): "When do you think it would be a good time to do a
proper deep review of the whole codebase again?" Decided: after
iteration 18 has deleted `saturnus-mcp` (so the review does not read
code about to disappear) and before the `v0.1.0` tag (the irreversible
step: published crate APIs and the protocol). A second review of the
same kind after the next wave (12b, 14, 22), which adds the first paths
that write calculator memory from the page.

## Design

Several reviewers in fresh contexts, each reading the whole tree (not a
diff) along one theme, plus one pass that merges, de-duplicates and
ranks; every finding verified against the code (reproduced where it is a
behaviour) before it is reported; the fixes go into this iteration's PR
with the usual three-reviewer pass on the PR itself.

Themes:
1. The core against the wiki: every hardware behaviour implemented
   should trace to a cited wiki page; anything implemented from memory
   or from a reading the wiki has since corrected (TIMER2 reads, the
   display hold) is a finding.
2. One protocol, three hosts: `web/protocol.md` against the Worker, the
   Tauri runner and the HTTP API; commands or shapes present in one host
   and not another, or documented differently; the typed-object shapes
   (`text`, `source`, `name`) end to end.
3. Security: the control API (auth, Host and Origin, bounds, withdrawal),
   the serial bridge, the Tauri boundary (no paths from the page, the
   capability file, the settings and ROM files), the page's storage
   promises, the generators' file handling; threat model written down
   and checked against the code.
4. Error handling and the project rules: `unwrap`/`expect` outside
   tests, `anyhow` context, panics reachable from guest-controlled data
   (RAM, ROM, state files), bounded work everywhere the guest controls
   sizes.
5. Dead code and duplication after the refactors (`saturnus-host`, the
   decompiler, the menus, the removed crawler, the retired MCP crate's
   remains).
6. Tests against `kb/docs/test-policy.md`: what is covered only by
   ROM-gated tests, what is not covered at all, flaky wall-clock
   assertions, the ignored 30-minute regeneration test's place in the
   release checklist.
7. The kb and the wiki against the code: plans whose Outcome no longer
   matches, decisions superseded without a note, README sections drifted
   from behaviour, `web/README.md` and `kb/docs/*`.
8. Supply chain and release: `deny.toml` (the exceptions and ignores
   still justified?), workflow pinning and permissions, the release
   checklist, the package audit repeated on the final tree.

## Tasks

- [ ] Run the eight themed reviews in parallel; each writes verified
  findings as JSON (file, line, severity, summary, failure scenario,
  suggestion, reproduced) to the scratchpad.
- [ ] Merge, de-duplicate and rank; present the table to the owner.
- [ ] Fix every high and medium finding, and every low one that is
  cheap; answer the rest with a reason in the PR; the PR through the
  usual review.
- [ ] Record the review's method and the findings count in the decision
  log; move anything deliberately deferred into `kb/backlog/`.

## Acceptance criteria

- [ ] No open high or medium finding before the `v0.1.0` tag.
- [ ] `just gates`, the ROM-gated suites and the full regeneration pass
  on the final tree.

## Outcome

(to be written)
