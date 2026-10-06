---
title: Releasing
type: docs
date: 2026-10-05
status: active
tags:
  - infrastructure
---

# Releasing

`.github/workflows/release.yml` calls the shared pipeline
`ractive/release-workflows/.github/workflows/release.yml@v0.2.1`. Publishing
a GitHub release `vX.Y.Z` runs it: the tag must match `saturnus-cli`'s
version, then `cargo audit` and `cargo deny check`, a build and test
matrix (Linux gnu/musl x86_64 and aarch64, macOS aarch64, Windows x86_64
and aarch64; the emulated aarch64 Linux targets and Windows on ARM build
only), archives with the `saturnus` binary, LICENSE and README, SBOMs and
build provenance, and the upload to the release.

A manual run (`workflow_dispatch`, Actions tab or
`gh workflow run release.yml`) is a dry run: it builds, tests and packages,
and uploads the archives as workflow artifacts only. A dispatch needs the
workflow on the default branch.

Deliberately off: crates.io (`saturnus-mcp` has a git dependency on
hptx-core, which crates.io does not accept), winget, AUR, Cloudsmith and
the deb/rpm packages. Not configurable in v0.2.1 and therefore on for a
real release: the Homebrew formula (`ractive/homebrew-tap`) and the Scoop
manifest (`ractive/scoop-bucket`); they need the `HOMEBREW_TAP_TOKEN` and
`SCOOP_BUCKET_TOKEN` repository secrets. The shared workflow ships one
binary, so `saturnus-mcp` is built but not in the archives; shipping it
needs a multi-binary input in `ractive/release-workflows` (follow-up
there, not a workaround here).

A dry run needs no secrets; the caller grants `contents: write`,
`id-token: write` and `attestations: write`, which the shared workflow's
`build` job inherits for the provenance attestation.

To enable crates.io, winget, AUR, Cloudsmith or deb/rpm later, set the
matching input (`publish-crates`, `winget-identifier`, `aur-package`,
`cloudsmith-repo`, `enable-linux-packages`) and add its secret
(`CARGO_TOKEN`, `WINGET_TOKEN`, `AUR_SSH_PRIVATE_KEY`,
`CLOUDSMITH_API_KEY`), as described in the shared workflow's README.

## crates.io: the core crate

The core crate `saturnus` is kept publishable to crates.io, but no
publication is planned: the hptx side first asked for it (hptx-cli was to
be published and would have needed the core there), then withdrew that on
2026-10-05 (hptx-cli stays off crates.io). The check is kept for its own
sake, since it is cheap and keeps the option open. Nothing is published;
the name `saturnus` was free on crates.io on 2026-10-05.
`cargo publish --dry-run -p saturnus --locked` runs in CI's
`quality-gates` and in `just gates`, so a git or path-only dependency in
the core, or a package that does not verify, fails the PR. The crate has
its own `README.md` and publishes only `src/`, `examples/` and the README
(`include` in its `Cargo.toml`): `tests/` stays out because its golden
files are screen dumps of HP's ROMs.

The other workspace crates stay unpublished for now. `saturnus-mcp`
cannot be published while it pins `hptx-core` by git.

## Web page: GitHub Pages and ractive.ch

`.github/workflows/pages.yml` (manual dispatch) builds the wasm package,
assembles the site with `web/site.sh` (every page file in `web/`, the
components, the package, and `web/site.htaccess` as `.htaccess` for
Apache hosts), and publishes it twice from that one build: to GitHub
Pages (https://ractive.github.io/saturnus/, Pages enabled with the
Actions source on 2026-10-06) and by FTP to the owner's site
(https://ractive.ch/saturnus/, `httpdocs/saturnus/` on ractive.ch; the
`FTP_PASSWORD` repository secret, the same the site's own deploy uses).
The first copy on ractive.ch was committed into that site's repository
by hand (ractive.ch PR 8); from then on this workflow keeps both in step.
Dispatch it after every merge to `main` that changes `web/` or the
bindings.

## Desktop app

`.github/workflows/desktop.yml` builds the Tauri app's installers with
`tauri-apps/tauri-action` on macOS (Apple silicon), Windows and Linux. It
runs only when dispatched by hand (Actions tab or `gh workflow run
desktop.yml`), needs no secrets and uploads the installers as workflow
artifacts; given a `release-tag` it builds that tag (not the dispatched
branch) and attaches the installers to that existing release (the
release must exist first, for example from `release.yml`); assets
already on the release are never replaced, so a rerun needs them deleted
by hand first.
It is not triggered by publishing a release, so a CLI release does not
build the app until the owner wires it in.

The installers are **unsigned**. What the owner has to provide before
shipping them to anyone:

- macOS: an Apple Developer ID certificate and notarisation credentials
  as repository secrets (`APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
  `APPLE_PASSWORD`, `APPLE_TEAM_ID`, the names tauri-action reads), then
  pass them as `env:` to the build step. Unsigned, Gatekeeper refuses the
  `.dmg` until the user allows it in System Settings.
- Windows: a code-signing certificate (Tauri's `bundle.windows`
  `certificateThumbprint` or a `signCommand`), else SmartScreen warns.
- Linux: nothing; `.deb`, `.rpm` and the AppImage are unsigned as usual.
- The bundle identifier is `ch.ractive.saturnus`
  (`crates/saturnus-tauri/tauri.conf.json`); changing it later orphans
  the app's stored preferences, so decide it before the first release.
- The app's version (`tauri.conf.json`, `Cargo.toml`, 0.1.0) is not tied
  to the CLI's release tag yet.

No updater is configured (it would need a signing key pair and an
endpoint).

## Web page

`.github/workflows/pages.yml` builds the wasm package and publishes the
page to GitHub Pages. Manual only. Before the first run the owner has to
enable Pages in the repository settings (Settings, Pages, Source:
"GitHub Actions"); that creates the `github-pages` environment the deploy
job uses. Until then the deploy job fails and nothing is published. No
secrets are needed. The page then lives at
`https://ractive.github.io/saturnus/` (all its paths are relative).

## Pinning policy

Third-party actions are pinned to a full commit SHA with a `# vX.Y.Z`
comment; Dependabot proposes the updates. First-party reusable workflows
and actions are referenced by tag, not SHA: `ractive/release-workflows`
by exact tag (`@v0.2.1`), `ractive/setup-hyalo@v1` floating on its major
tag, as in hyalo. Both repositories belong to the owner, so a tag is a
reviewed release; Dependabot ignores them.
