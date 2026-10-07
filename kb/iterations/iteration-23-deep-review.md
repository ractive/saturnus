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

### Batch 2: correctness, protocol and docs

1. `rust-version` was 1.85 but let-chains need 1.88: the workspace now
   says 1.88 (`cargo +1.88.0 check --workspace --exclude saturnus-tauri
   --locked` passes); new CI job `msrv` checks the five published crates
   with 1.88.0 (`kb/docs/ci.md`).
2. `saturnus run` writes the RAM cards first, then state, screen and
   annunciators, attempting every one and reporting the failures
   together; state and cards go through the hardened atomic write
   (item 10). Test `tests/control.rs::a_failing_output_still_writes_the_card`.
3. `GET /v1/object?address=` (the runner's bounds: 422 outside the
   address space) and `saturnus ctl object ADDR`; `web/protocol.md` and
   README; cases in `server::tests::oversized_bodies_and_heads_are_refused`.
4. `just rom-tests DIR` runs every ROM-gated test (the workspace, then
   `saturnus-tauri` with `tests/runner.rs`, then the `--ignored`
   regeneration); `kb/docs/releasing.md` step 4 uses it.
5. The Worker checks `key` and `letter` as strings (`missing string
   field`, as the runner) and `pause` without `paused` pauses (only
   `paused: false` runs). Node test in `web/test/worker-typing.test.mjs`.
   `romSettings`/`forgetRom` differences left: the page always sends
   valid fields.
6. Run/Pause catches like Reset; `exitFullscreen` and the Alt+M chain
   report into the status line; a failed About load is retried on the
   next open.
7. `kb/docs/clean-room-rule.md` states the stricter rule (the emulators'
   source is off limits to anyone implementing; only a designated
   reviewer opens it; facts from documentation, change logs and black-box
   runs, cited in the wiki); README Legal and `about-json.py` say the
   same and name Emu42, jsEmu48, x50ng and ui4x; `web/about.json`
   regenerated (it also picked up wiki pages added since the last run).
8. Docs: README Tests (`--exclude saturnus-tauri`, the typing, runner and
   `rom-tests` suites), Web UI (status line, paste and palette, 42S skin,
   skin sources, stored keys); `web/README.md` (runner path,
   `memory_changes` polling, `saturnus.layer*` keys); CHANGELOG and
   release notes (typing, palette); `architecture.md` (status, machine/,
   saturnus-objects, saturnus-refgen, no `run_until_idle`);
   `test-policy.md` (the gated suites and their commands).

### Batch 3: security

9. Instead of a per-run file with an echoed server id (an echo any local
   user can ask for while the run is up and replay after a crash), `ctl`
   asks `GET /v1/hello?nonce=N` without the token and requires
   HMAC-SHA-256 under the token of N and the server's bound port; any
   other answer ends `ctl` before the token is sent. No file to keep or
   clean up; a proof relayed from a real server on another port does not
   match. Tests `token::tests::hmac_and_proofs` (RFC 4231 vector),
   `server::tests::hello_proves_the_token_without_it`,
   `tests/control.rs::ctl_does_not_send_the_token_to_an_impostor` (wrong
   proof, 404, 401: no `Authorization` reaches the fake). Threat and
   residual (an impostor can still fail `ctl`; `curl` scripts do not
   check) in `kb/docs/control-api-security.md` and README.
10. `write_atomic` creates its temporary with `create_new` (no link
    followed, no file reused) under a random 64-bit suffix, retrying on a
    clash; the Tauri settings writer uses it too (`write_atomic_with`,
    mode 0600). Test `runner::tests::atomic_writes_do_not_follow_planted_links`.
11. `web/site.sh` refuses a target that contains `web/` and a non-empty
    target without the `.saturnus-site` marker an earlier run leaves.
    Checked in a scratch copy: `.`, the checkout's absolute path, `..`,
    `web`, `web/x` and an unmarked directory refused; a marked one
    rebuilt.
12. CSP: `web/site.htaccess` sets it (with `frame-ancestors 'none'`,
    inside `<IfModule mod_headers.c>`); `web/site.sh` copies it without
    frame-ancestors as a meta tag into the site's `index.html` only, not
    into `web/index.html`, because the desktop app loads that file under
    its own CSP (`tauri.conf.json`, which needs `ipc:`) and a second,
    stricter policy there would block its IPC. Headless Chrome booted a
    48SX from the assembled site with the header and the meta tag, and
    with the meta tag alone: the screen drawn, no violation.
13. Forget ROMs (browser) also deletes the saved 49G state
    (`ROM_HOLDING_STATES`); the panel hint, the message, README,
    `web/README.md` and the About text say so. Node test
    `web/test/forget.test.mjs`.
14. Connection flood: residual risk documented in
    `kb/docs/control-api-security.md` (no rate limit can tell the other
    user from the token holder before the token is read); no code change.

### Owner requests

15. The grid view is removed: the "Drawn calculator" box, the
    `saturnus.view` preference (removed from localStorage on load), the
    grid rendering and CSS, `backend.layout()`, the palette's view action
    and the docs' mentions. Kept, because the control API uses them: the
    protocol's `layout` command, the wasm binding and `saturnus-host`'s
    layout module (`model` result, key names); `protocol.md` says so.
    Decision log entry.
16. The panel's Pause/Run button is removed; pausing is the palette's
    action. Headless Chrome: "Pause the calculator" shows "paused" in the
    status line and "Run the calculator" resumes. The `pause` command is
    unchanged.
17. The skin follows the selected model with or without a ROM; without one
    an empty state over the LCD names the model (the 42S: dump your own
    ROM) with "Choose ROM…" (opens that model's picker or dialog); a drawn
    or computer-keyboard key pulses it. Switching away from a running
    model without a ROM for the new one pauses it, switching back resumes
    it; a remembered ROM boots as before. Rules in `web/norom.js`, Node
    test `web/test/norom.test.mjs` (fake backend). Headless Chrome on the
    served site: 48SX, 49G, 42S without ROMs each drew its skin and its
    message, Enter pulsed it, then the 48SX ROM booted (screen drawn),
    49G paused it, 48SX resumed it. The app uses the same component.
