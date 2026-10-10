---
type: iteration
title: "Iteration 36a: satx joins saturnus, with its history"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus
  - satx
  - consolidation
branch: iter-36a/import-satx
---

# Iteration 36a: satx joins saturnus, with its history

The first of four consolidation iterations (36a import, 36b renames, 36d site,
36c release). The owner's decisions are in [[decision-log]] under
"2026-10-10 (consolidation)". The transfer tool from ractive/hptx moves into
this repository, with its git history, as **satx**. Its old repository is then
deleted. There is no backwards compatibility. Once the consolidation is done,
the old name appears nowhere: not in the current files, and not in the
imported history.

## Where things stand (2026-10-10)

- The hptx repository: main is at `078ca41` (PR #28 merged). PR #29 is a draft
  (sans-I/O core, branch `iter-14/wasm-core`, 2 commits ahead and 3 behind,
  conflicting with #28). PR #30 (`feat/chars-help`) is open. It has 127
  commits, the tags v0.1.0 and v0.1.1 (v0.1.0 clashes with ours), and no
  issues.
- The crates.io crates `kermit-proto` and `xmodem-proto` point at that repo.
  `saturnus-host` and `saturnus-kermit` fetch `kermit-proto` from crates.io.
- Imported authors: 23 commits use the comparis.ch address. Committers: 27
  commits are by `noreply@github.com` (the merges).
- Elsewhere, the old name appears in: `Formula/hptx.rb` (ractive/homebrew-tap),
  `bucket/hptx.json` (ractive/scoop-bucket, public since 2026-10-10), the open
  PR microsoft/winget-pkgs#448271, and the Cloudsmith repository
  `ractive/hptx` (4 packages, 0 downloads).

## The rewrite of the imported history

All of it happens in a scratch clone (`git clone --no-local --no-tags`, with
the refs main, `iter-14/wasm-core` and `feat/chars-help`), using
`git filter-repo`. The checkout `~/devel/hptx` is never changed.

- **Paths:**
  - `crates/hptx-core` → `crates/satx-core`;
  - `crates/hptx-cli` → `crates/satx-cli`;
  - `kb/` → `kb/satx/`, and the iteration files named for the old name get
    satx names;
  - `emulator/` → `tools/saturnng-oracle/`;
  - `scripts/e2e-cli.sh` → `scripts/satx-e2e-cli.sh`;
  - `README.md` → `crates/satx-cli/README.md`;
  - Cargo.toml, Cargo.lock, CLAUDE.md, justfile, deny.toml, .hyalo.toml,
    .gitignore, LICENSE, AI_NOTICE, .github/ and .claude/ → `satx-import/`,
    folded in by hand and then deleted.
- **Removed from all history:** `docs/hptx-demo.gif`, because the name is in
  its pixels.
- **Contents and messages** (`--replace-text`, `--replace-message`), most
  specific first:
  1. `github.com/ractive/hptx` → `github.com/ractive/saturnus`
  2. `~/devel/hptx/emulator` → `tools/saturnng-oracle`
  3. `hptx-cli` → `satx-cli`, `hptx-core` → `satx-core`
  4. `hptx_core` → `satx_core`, `hptx_cli` → `satx_cli`
  5. `HPTX_` → `SATX_`
  6. `hptx` → `satx`, `Hptx` → `Satx`, `HPTX` → `SATX`

  Binary blobs (the `.hp` fixtures) are left as they are.
- **PR numbers, in messages only:** `regex:(?<![\w/&])#(\d+)==>satx PR \1`.
  This keeps "#26" from linking to a saturnus PR.
- **Mailmap, for author and committer:**
  - `James Bergamin <james@ractive.ch> <james.bergamin@comparis.ch>`
  - `James Bergamin <james@ractive.ch> <noreply@github.com>`
  - any other address that is not james@ractive.ch.

## Tasks

- [ ] Owner: nothing more lands in ractive/hptx. Commit and push, or drop, any uncommitted work in `~/devel/hptx-wasm` and `~/devel/hptx-chars`
- [ ] Owner: on GitHub, set the primary email to james@ractive.ch (Settings → Emails), so that merge commits made on GitHub carry it
- [ ] Scratch clone rewritten as above. Each of these prints nothing: `git log --all -i --grep hptx`, `git log --all -i -G hptx`, `git log --all --name-only --format= | grep -i hptx`, `git log --all --format=%B | grep -nP '(?<![\w/&])#\d+'`, `git log --all --format='%ae%n%ce' | sort -u | grep -vx james@ractive.ch`
- [ ] Gates and the ROM-gated e2e pass on the rewritten clone before the merge
- [ ] `git merge --allow-unrelated-histories --no-ff` into `iter-36a/import-satx`, with the message "Import satx with its history", and no tags. The rewritten #29 and #30 branches pushed as `satx/sans-io-core` and `satx/chars-help`
- [ ] Workspace members and dependencies: kermit-proto, xmodem-proto, satx-core and satx-cli join `members` and `default-members` with `version.workspace = true`. Workspace dependencies for the first three. satx-core's git dependencies on saturnus become path dependencies
- [ ] `saturnus-host` and `saturnus-kermit` take `kermit-proto` from the workspace (path). Nothing fetches it from crates.io any more
- [ ] Build profile: the dev profile keeps `opt-level = 3` for the core and for saturnus-drive (the in-process e2e needs it)
- [ ] The licence check and `cargo package` in `ci.yml`, and the justfile's `package` recipe, cover the four new crates. Each has a LICENSE copy
- [ ] `deny.toml`: the new crates' licences and the MPL-2.0 exception for `serialport`. Git sources stay banned
- [ ] CI: a `satx-e2e` job (the saturnng oracle containers on ports 4848 and 4850; [[docs/clean-room-rule]] allows the black-box oracle, and the ROMs are downloaded, never committed or shipped). The wasm job builds the two proto crates
- [ ] CI: a `commit-email` job on pull requests (`fetch-depth: 0`). Every commit in `origin/<base>..HEAD` has the author email james@ractive.ch. Only `dependabot[bot]` is exempt. CLAUDE.md states the rule
- [ ] CLAUDE.md: a satx section (oracle ports 4848–4852, `SATX_E2E_ADDR`, `SATURNUS_ROM_DIR`). justfile: the `satx-*` recipes, with the ROMs from `roms/`
- [ ] `scripts/diff-vs-saturnng.sh`: `EMU_DIR` defaults to `tools/saturnng-oracle`
- [ ] kb:
  - satx's open backlog items move to `kb/backlog/satx-*` (CI emulator flakes, flaky XModem e2e, trusted publishing);
  - satx iteration 6 (real hardware) becomes a backlog item;
  - satx iteration 7 (GUI) is marked superseded by saturnus tx;
  - the relative links in `kb/satx/` resolve inside it;
  - `hyalo lint` is clean
- [ ] Backlog: fold saturnus-kermit onto satx-core once satx-core has an emulated-time transport, and remove the duplicate charset in saturnus-objects
- [ ] `just gates`, then /create-pr, /review-pr, /merge-pr
- [ ] Owner: export the old repo's PRs and review comments (`gh pr list --state all --json …` and the comments) to a file outside every repository
- [ ] Owner, after the merge:
  - delete `Formula/hptx.rb` (homebrew-tap) and `bucket/hptx.json` (scoop-bucket), with commit messages that do not use the old name;
  - close winget-pkgs PR #448271 without a comment, and delete its branch in ractive/winget-pkgs;
  - delete the Cloudsmith repository `ractive/hptx`;
  - close #29 and #30;
  - delete ractive/hptx (`gh auth refresh -s delete_repo`, then `gh repo delete`);
  - revoke only the tokens used by that repository alone
- [ ] Owner: remove the local checkouts `~/devel/hptx`, `hptx-wasm` and `hptx-chars`

## Acceptance

- main builds and tests with the satx crates as workspace members, and no
  crate is fetched from git or for `kermit-proto` from crates.io.
- The commits that came in with the import contain neither the old name nor
  an address other than james@ractive.ch.
- ractive/hptx, its formula, its Scoop manifest and its Cloudsmith repository
  are gone.

Irreversible: deleting ractive/hptx and the Cloudsmith repository.
