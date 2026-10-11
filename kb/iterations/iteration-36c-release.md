---
type: iteration
title: "Iteration 36c: release 0.2.0, crates.io and the final check"
date: 2026-10-10
status: completed
tags:
  - iteration
  - saturnus
  - satx
  - release
  - consolidation
branch: iter-36c/release
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

The plan was to delete all seven old crates, from the leaves inward
(`saturnus-cli`, `saturnus-drive`, `saturnus-host`, `saturnus-objects`,
`saturnus`, `xmodem-proto`, `kermit-proto`). On 2026-10-11 the owner kept
them instead ([[decision-log]], "0.2.0: the old crates stay on
crates.io"); the staged publish below was not needed.

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

- [x] release-workflows v0.3.0 as above: PR, review, tag
- [x] `release.yml` here, on release-workflows v0.3.0 (it replaces PR #104's bump to v0.2.4), with two jobs:
  - `release-saturnus`: `bin-name` and `version-package` `saturnus`, `checksums-file: saturnus-SHA256SUMS`, the musl swap with `-p saturnus`, `linux-package-crate: saturnus`, and `publish-crates: saturnus-core,kermit-proto,saturnus-objects,saturnus-host,saturnus-drive,saturnus,xmodem-proto,satx-core,satx`;
  - `release-satx`: `bin-name` and `version-package` `satx`, `checksums-file: satx-SHA256SUMS`, satx's completions in the archives and the deb/rpm, `linux-package-crate: satx`, `publish-crates: ""`, and satx's targets (with `x86_64-apple-darwin`)
- [x] `publish-crates.yml`: dispatch inputs `crates` (default: the full list) and `ref`
- [x] Version 0.2.0 in `[workspace.package]` and every workspace dependency. CHANGELOG
- [x] kb/docs/releasing.md, section "crates.io": the nine published crates in the publish order above (kermit-proto, xmodem-proto, satx-core and satx included), and the two release jobs
- [x] Owner: dry run (`gh workflow run release.yml`). The bundle holds both binaries' archives, two checksum files, and the .deb/.rpm named `saturnus` and `satx`. The desktop.yml build has `saturnus-app` packages. `/usr/bin/saturnus` belongs to exactly one package
- [x] /create-pr, /review-pr, /merge-pr
- [x] Owner: push tag `v0.2.0` without creating a GitHub release. Run `gh workflow run publish-crates.yml -f ref=v0.2.0 -f crates=saturnus-core`
- [x] ~~Owner: delete the seven old crates~~. Dropped on 2026-10-11 ([[decision-log]], "0.2.0: the old crates stay on crates.io"): only kermit-proto and xmodem-proto 0.1.x named the old repository
- [x] `gh workflow run publish-crates.yml -f ref=v0.2.0` (run 38119399602): all eight crates at 0.2.0, no rate-limit wait. Then `gh release create v0.2.0` with `.github/release-notes/v0.2.0.md` (archives, Homebrew `saturnus` and `satx`, Scoop `saturnus` and `satx`; its crates job skips everything)
- [x] The landing page gets the install lines (`brew install ractive/tap/saturnus`, `brew install ractive/tap/satx`, `cargo install saturnus`, `cargo install satx`, Scoop): PR 116, deployed with pages.yml run 38121094692
- [x] Check from a clean state: `cargo install saturnus` and `satx` print 0.2.0; the formulas and manifests point at the 0.2.0 archives; the CLI's, satx's and the desktop app's .deb install together on Ubuntu 24.04 (`/usr/bin/saturnus` belongs to `saturnus` only, the menu entry runs `saturnus-app`); docs.rs built the seven library crates (`saturnus` and `satx` are binaries only)
- [x] Verification: in a fresh clone, with M as the import merge commit from 36a (`82e939c`), each of these prints nothing:
  - `git grep -i 'hp[t]x'`;
  - `git log -i -P --grep 'hp[t]x' M^2`;
  - `git log -i -G 'hp[t]x' M^2`;
  - the paths in `M^2`;
  - `git log --format='%ae' M^2 | sort -u | grep -vx james@ractive.ch`
- [x] Verification: every commit merged since 36a has the author email james@ractive.ch, dependabot aside (63 commits on 2026-10-11)
- [x] Verification: every non-fork ractive repository (58), shallow-cloned, has no match in `git grep -il 'hp[t]x'` (except the archived ractive/hp-literature), after release-workflows PR 6 and ractive.ch PR 11 on 2026-10-11. `gh search code` finds nothing
- [x] Verification:
  - [x] crates.io shows every crate's 0.2.0 with `repository = ractive/saturnus`;
  - [x] a crates.io search for the old name finds nothing;
  - [x] the old GitHub repository returns 404 (deleted 2026-10-11);
  - [x] the Cloudsmith repository of the old name returns 404 (deleted by the owner on 2026-10-11);
  - [x] the tap and bucket have no match;
  - [x] neither `/saturnus/` nor `/saturnus/emulator/` mentions the old name, and `/saturnus/about.json` is gone (404);
  - [x] nothing on the local disk (saturnus, calculator-knowledgebase, the owner's memory), and the old repository's checkouts no longer exist
- [x] Last: the commands and their results are in this file. Delete the PR export. The old name in these four iteration files and in the 2026-10-10 decision-log entry is reworded (written `<old>` as in [[iterations/iteration-36a-import]], matched by the pattern `hp[t]x`), so `git grep -i 'hp[t]x'` stays empty

## Accepted exceptions

These keep the old name and are left alone (variant A):
- this repository's own history from before 36a;
- the histories of calculator-knowledgebase, homebrew-tap and scoop-bucket;
- the archived ractive/hp-literature;
- microsoft/winget-pkgs#448271 (closed; PRs there cannot be deleted);
- the commit lists of this repository's older PRs;
- third-party archives;
- the 0.1.x versions of kermit-proto and xmodem-proto on crates.io, whose repository link names the old repository.

Irreversible: each publish.
