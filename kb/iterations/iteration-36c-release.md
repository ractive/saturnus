---
type: iteration
title: "Iteration 36c: release 0.2.0, crates.io and the final check"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus
  - satx
  - release
  - consolidation
branch: iter-36c/release-0.2.0
---

# Iteration 36c: release 0.2.0, crates.io and the final check

The last consolidation iteration ([[decision-log]], "2026-10-10
(consolidation)"). It needs 36a, 36b and 36d to be merged first. One version,
0.2.0, for every crate and the desktop app. Two binaries, `saturnus` and
`satx`, come from one tag.

## crates.io

The deletion rules, from the code crates.io runs:
- A crate can be deleted if it is under 72 hours old, or if it has one owner
  and fewer than 1000 downloads for each started 30-day month.
- **In every case**, no other crate may depend on it. Yanked versions and
  dev-dependencies count.
- Only the browser can delete.
- A deleted name is blocked for everyone, the owner included, for 24 hours,
  and then free to anyone.

New crates are rate-limited: a burst of 5, then 1 every 10 minutes. A 429
response stops the job.

All seven crates are deleted, from the leaves inward:
`saturnus-cli`, `saturnus-drive`, `saturnus-host`, `saturnus-objects`,
`saturnus`, `xmodem-proto`, `kermit-proto`.

`saturnus-core` is a new name, so it goes out before the deletions. At T+24h,
eight crates are new. `saturnus` can only go out once its dependencies are up:
kermit-proto, saturnus-objects, saturnus-host and saturnus-drive. That makes it
fifth, inside the burst. xmodem-proto, satx-core and satx follow at 10-minute
intervals. **The owner accepts the risk that someone takes `saturnus`, or
another name, between T+24h and our publish.**

## release-workflows v0.3.0

Based on v0.2.4, and backwards compatible for hyalo, hoppy and ff-rdp. Two
callers in one run collide in four places today, and the first four items below
fix them:
- the `release` job's artifact download is filtered by `bin-name`;
- a new input `checksums-file`, default `SHA256SUMS`, read by the homebrew,
  scoop and aur jobs and used for the Scoop checksum URL;
- the dry-run bundle is named after `bin-name`;
- the tap and bucket pushes retry with `git pull --rebase`, up to 5 times;
- the crates-io loops (`release.yml` and `publish-crates.yml`) wait 600
  seconds on crates.io's "too many new crates" 429;
- `selftest.yml` runs two callers in one run;
- no Cloudsmith and no winget changes.

## Tasks

- [ ] release-workflows v0.3.0 as above: PR, review, tag
- [ ] `release.yml` here, on release-workflows v0.3.0 (it replaces PR #104's bump to v0.2.4), with two jobs:
  - `release-saturnus`: `bin-name` and `version-package` `saturnus`, `checksums-file: saturnus-SHA256SUMS`, the musl swap with `-p saturnus`, `linux-package-crate: saturnus`, and `publish-crates: saturnus-core,kermit-proto,saturnus-objects,saturnus-host,saturnus-drive,saturnus,xmodem-proto,satx-core,satx`;
  - `release-satx`: `bin-name` and `version-package` `satx`, `checksums-file: satx-SHA256SUMS`, satx's completions in the archives and the deb/rpm, `linux-package-crate: satx`, `publish-crates: ""`, and satx's targets (with `x86_64-apple-darwin`)
- [ ] `publish-crates.yml`: dispatch inputs `crates` (default: the full list) and `ref`
- [ ] Version 0.2.0 in `[workspace.package]` and every workspace dependency. CHANGELOG
- [ ] Owner: dry run (`gh workflow run release.yml`). The bundle holds both binaries' archives, two checksum files, and the .deb/.rpm named `saturnus` and `satx`. The desktop.yml build has `saturnus-app` packages. `/usr/bin/saturnus` belongs to exactly one package
- [ ] /create-pr, /review-pr, /merge-pr
- [ ] Owner: push tag `v0.2.0` without creating a GitHub release. Run `gh workflow run publish-crates.yml -f ref=v0.2.0 -f crates=saturnus-core`
- [ ] Owner: **irreversible.** Precondition: 36a, 36b and 36d merged, the dry run green, saturnus-core 0.2.0 on crates.io, the old repo's PRs exported. In the browser (`crates.io/crates/<name>/settings`), delete saturnus-cli, saturnus-drive, saturnus-host, saturnus-objects, saturnus, xmodem-proto and kermit-proto, in that order and within a few minutes. Write down T, the time of the last deletion
- [ ] Owner: at T+24h, run `gh workflow run publish-crates.yml -f ref=v0.2.0`, in this order: kermit-proto, saturnus-objects, saturnus-host, saturnus-drive, saturnus, then xmodem-proto, satx-core and satx. Then `gh release create v0.2.0 --generate-notes` (archives, Homebrew `saturnus` and `satx`, Scoop `saturnus` and `satx`; its crates job skips everything)
- [ ] The landing page gets the install lines (`brew install ractive/tap/saturnus`, `brew install ractive/tap/satx`, `cargo install saturnus`, `cargo install satx`, Scoop). Owner: deploy
- [ ] Check from a clean state: `cargo install saturnus` and `satx`, both brew installs, the desktop .deb installed next to the CLI .deb, docs.rs for 0.2.0
- [ ] Verification: in a fresh clone, with M as the import merge commit from 36a, each of these prints nothing:
  - `git grep -i hptx`;
  - `git log -i --grep hptx M^2`;
  - `git log -i -G hptx M^2`;
  - the paths in `M^2`;
  - `git log --format='%ae' M^2 | sort -u | grep -vx james@ractive.ch`
- [ ] Verification: every commit merged since 36a has the author email james@ractive.ch, dependabot aside
- [ ] Verification: every non-fork ractive repository, shallow-cloned, has no match in `git grep -il hptx` (except the archived ractive/hp-literature). `gh search code hptx --owner ractive` is empty
- [ ] Verification:
  - crates.io shows every crate with `repository = ractive/saturnus`;
  - `cargo search hptx` finds nothing;
  - Cloudsmith `ractive/hptx` and `github.com/ractive/hptx` return 404;
  - the tap and bucket have no match;
  - neither `/saturnus/`, `/saturnus/emulator/` nor its `about.json` mentions the old name;
  - `/saturnus/about.json` is gone;
  - nothing on the local disk (saturnus, calculator-knowledgebase, the owner's memory), and `~/devel/hptx*` no longer exist
- [ ] Last: the commands and their output go in this file. Delete the PR export. Turn the old name in these four iteration files and in the 2026-10-10 decision-log entry into the post-rename wording, so that `git grep -i hptx` stays empty

## Accepted exceptions

These keep the old name and are left alone (variant A):
- this repository's own history from before 36a;
- the histories of calculator-knowledgebase, homebrew-tap and scoop-bucket;
- the archived ractive/hp-literature;
- microsoft/winget-pkgs#448271 (closed; PRs there cannot be deleted);
- the commit lists of this repository's older PRs;
- third-party archives.

Irreversible: the seven crate deletions, and each publish.
