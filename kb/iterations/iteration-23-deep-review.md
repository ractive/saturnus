---
type: iteration
title: "Iteration 23: Deep review of the whole codebase before v0.1.0"
date: 2026-10-07
status: in-progress
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

In progress. Findings fixed so far, with what was done:

### Batch 1: parsers of untrusted RAM and ROM (`saturnus-objects`)

1. High, `object.rs` array decode: `nest()` recursed once per array
   dimension, so a crafted header with 12 000 dimensions (60 K nibbles)
   overflowed the stack and aborted the process. Arrays now have at most
   `MAX_ARRAY_DIMS` = 2 dimensions (the calculator's vectors and
   matrices); more is a decode error before any allocation, which a
   top-level `decode` shows as an unknown object and a stack read reports.
   Recursion audit: every other recursive function follows object nesting,
   which the decoder bounds at `MAX_DEPTH` = 64 (lists, tagged, programs,
   algebraics through `element`/`pointer`, the decompiler's `expression`,
   `write_object`, `has_text`, `describe`, serde, `Drop`); the directory
   walk is bounded through `records`/`walk`; the menus at `MAX_NESTING`
   = 12 and fixed depths in `label` and `definition`; infix rendering was
   already iterative. Test `arrays_have_at_most_two_dimensions` (fails on
   the old code). Reproduction after the fix: the 12 000-dimension array
   decodes to an unknown object in a debug build with 2.5 MB resident
   (before: stack overflow, abort).
2. High, `object.rs` decode budget: it counted objects only and cached
   objects were cloned, so 4000 stack levels pointing at one 50 K-character
   string produced 381 MB of JSON with 1.2 GB resident in 0.7 s. Added
   `MAX_DECODED_NIBBLES` = 2^21 (four times the 49G's RAM): every object
   read from memory charges its size, and a cached object charges its
   objects and nibbles again before it is cloned, so the clones are
   bounded like the reads (no `Rc`: the `Object` API stays as it is).
   After review (PR 32): clones have their own budget,
   `MAX_CLONED_NIBBLES` = 2^24, because one shared budget refused stacks
   a calculator really builds (a large GROB DUP'd a few times); distinct
   reads stay at 2^21. The test now shows 100 DUP levels of a 40 K-nibble
   string decoding and 500 refused.
   `described` and `stack_described` also cap the `text` fields they add
   at `MAX_DESCRIBED_TEXT` = 4 M characters per call (a string nested 64
   deep is written 64 times). Both are errors, not truncation. Tests
   `repeated_large_objects_hit_the_nibble_budget` and
   `nested_texts_hit_the_described_budget` (both fail on the old code).
   Reproduction after the fix: the 4000-level stack is refused at level 21
   in under 1 ms with 3.7 MB resident; the largest stack within the budget
   (20 levels of the 50 K-character string) takes 4 ms, 10 MB resident,
   about 2 MB of JSON.
3. Medium, `names.rs` library scan: many headers sharing one large link
   table made a table per header (3000 headers: 5.4 GB, 3.1 s). Counted
   on the real images: 2 libraries on the 48SX (largest 385 commands), 42
   on the 48GX (507), 52 on the 49G 2.10 (1444, 5940 commands in all).
   A library of more than `MAX_LIBRARY_COMMANDS` = 4096 entries (command
   numbers are 3 nibbles) is not a library; more than `MAX_LIBRARIES` =
   256 libraries or `MAX_COMMANDS` = 32 768 commands in all gives an
   empty table (commands stay unnamed). Test
   `crafted_link_tables_are_capped` (fails on the old code). Reproduction
   after the fix: 3000 headers give an empty table in 3 ms, 2.9 MB
   resident.
