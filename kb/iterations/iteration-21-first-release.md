---
type: iteration
title: "Iteration 21: First public release (crates, CLI binaries, desktop app, web page)"
date: 2026-10-06
status: planned
tags:
  - iteration
  - infrastructure
branch: iter-21/first-release
---

# Iteration 21: First public release (crates, CLI binaries, desktop app, web page)

Read first: `kb/docs/releasing.md`, `kb/docs/ci.md`,
`.github/workflows/{release,desktop,pages}.yml`, `deny.toml`, every
crate's `Cargo.toml`, `kb/iterations/iteration-16-ci-and-supply-chain.md`
(Outcome), `kb/iterations/iteration-18-retire-mcp.md`.

## Context (owner, 2026-10-06)

"If possible you can also drive the publication of all the crates and
saturnus app(s)." The owner was away when this was written; what can be
done without them is done, what needs something only they can provide is
prepared up to that point and listed.

State on 2026-10-06 (checked):
- crates.io: the names `saturnus`, `saturnus-objects`, `saturnus-drive`,
  `saturnus-web`, `saturnus-cli` and `saturnus-tauri` are free. There is
  no crates.io token on the development machine and no `CARGO_TOKEN`
  secret in the repository, so nothing can be published to crates.io
  until the owner adds one.
- Repository secrets: only `WINGET_TOKEN`. `HOMEBREW_TAP_TOKEN` and
  `SCOOP_BUCKET_TOKEN` are missing, and the shared release workflow runs
  its Homebrew and Scoop jobs on every real release.
- GitHub Pages is not enabled for the repository.
- No tag and no release exist. Every crate is at 0.1.0.
- Crate graph: `saturnus` (core, no dependencies) <- `saturnus-objects`
  <- `saturnus-web` (wasm-bindgen) <- `saturnus-drive` <- `saturnus-cli`,
  `saturnus-tauri`. `saturnus-mcp` (git dependency on hptx-core, to be
  retired in iteration 18) and `saturnus-refgen` (depends on it) cannot
  go to crates.io and are not meant to.

## Decisions taken for the owner (reversible until the first publish)

- **What is published where.** crates.io: `saturnus` (core),
  `saturnus-objects`, `saturnus-drive`, `saturnus-cli` (so `cargo install
  saturnus-cli` gives the `saturnus` binary). Not on crates.io:
  `saturnus-mcp`, `saturnus-refgen` (`publish = false`), `saturnus-tauri`
  (shipped as installers) and, if the graph allows, `saturnus-web`
  (shipped as the web page). GitHub release: the CLI archives from
  `release.yml`, the desktop installers from `desktop.yml`. GitHub Pages:
  the web page.
- **Crate graph before the names are taken for good.** `saturnus-drive`
  and the CLI depend on `saturnus-web`, a wasm-bindgen crate, only for
  the host-neutral key queue and command handling. Publishing the CLI
  that way would force `saturnus-web` and wasm-bindgen onto every `cargo
  install`. The host-neutral part moves to a plain crate (into
  `saturnus-drive`, or a small crate below both) and `saturnus-web`
  becomes a thin binding layer on top; no behaviour change.
- **Version.** 0.1.0 for all published crates and the app; one version
  for the workspace (`workspace.package.version`), the tag `v0.1.0`; the
  desktop app's version follows the same number.
- **Unsigned installers** for the first release, clearly labelled, with
  the steps to open them on macOS and Windows in the release notes;
  signing is the owner's (certificates).

## Tasks

- [ ] Crate graph cleanup as above; `publish = false` on the crates that
  stay private; `cargo tree -p saturnus-cli` shows no wasm-bindgen.
- [ ] Publishing metadata for each published crate (description, readme,
  keywords, categories, documentation, repository, licence, `include`
  lists that keep ROM-derived goldens out); a README per crate; docs.rs
  builds (`cargo doc` clean with `-D warnings` on the published crates);
  the embedded command reference still packages with the CLI.
- [ ] A publish rehearsal for the whole chain without touching
  crates.io: `cargo publish --dry-run` in dependency order where cargo
  allows it, or packaging the workspace against a local registry; in
  `just gates` and CI as far as it is cheap.
- [ ] `release.yml`: `publish-crates` in dependency order (the shared
  workflow's input and its `CARGO_TOKEN` secret); a dry run through
  `workflow_dispatch` passes. If the shared workflow cannot publish a
  chain of crates, say what it lacks; the fix belongs in
  `ractive/release-workflows`, not in a workaround here.
- [ ] `desktop.yml` dispatched once without a release tag: installers for
  macOS, Windows and Linux built as artifacts; each downloaded and its
  contents checked as far as possible on this Mac (the macOS app starts).
- [ ] `pages.yml`: enable Pages with "GitHub Actions" as the source (the
  owner asked for the apps to be published; this is the setting that
  publishes the web page), dispatch it, and check the published page in a
  browser (boots a ROM from a local file; nothing is uploaded; the About
  panel and the privacy wording are right). The page ships no ROM.
- [ ] CHANGELOG and release notes for 0.1.0 (what it is, the seven
  models, clean-room and AI notice, no ROMs included and where users get
  them, known limits, unsigned installers); README install section
  (`cargo install`, archives, Homebrew and Scoop once their tokens
  exist, the desktop installers, the web page URL).
- [ ] A release checklist in `kb/docs/releasing.md`: the exact commands,
  in order, from "secrets present" to "published", including the
  crates.io publish order and what to check after each step.
- [ ] Audit before anything goes out: no ROM, state file, token or local
  path in any package (`cargo package --list` per crate), in the web
  bundle, or in the installers.

## Waiting for the owner (cannot be done for them)

- A crates.io API token as the repository secret the shared workflow
  reads (`CARGO_TOKEN`), or `cargo login` on this machine. Publishing a
  crate name is permanent.
- `HOMEBREW_TAP_TOKEN` and `SCOOP_BUCKET_TOKEN`, or a decision to release
  without Homebrew and Scoop (needs the shared workflow to make those
  jobs optional).
- Signing certificates, if signed installers are wanted.
- The go for the tag: creating the GitHub release `v0.1.0` is what
  starts the real publication.

## Acceptance criteria

- [ ] The publish rehearsal passes for the whole chain and
  `saturnus-cli`'s dependency tree has no wasm-bindgen.
- [ ] The web page is live on GitHub Pages and works with a local ROM.
- [ ] Installers for the three platforms exist as workflow artifacts.
- [ ] With the owner's secrets in place, the release checklist is a list
  of commands with nothing left to decide.

## Outcome

(to be written)
