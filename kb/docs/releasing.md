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

The core crate `saturnus` is on a crates.io publishing path: hptx-cli will
be published and needs every dependency there (order: kermit-proto,
xmodem-proto, saturnus, hptx-core, hptx-saturnus, hptx-cli). Nothing is
published yet; the name `saturnus` was free on crates.io on 2026-10-05.
`cargo publish --dry-run -p saturnus --locked` runs in CI's
`quality-gates` and in `just gates`, so a git or path-only dependency in
the core, or a package that does not verify, fails the PR. The crate has
its own `README.md` and publishes only `src/`, `examples/` and the README
(`include` in its `Cargo.toml`): `tests/` stays out because its golden
files are screen dumps of HP's ROMs.

The other workspace crates stay unpublished for now. `saturnus-mcp`
cannot be published while it pins `hptx-core` by git.

## Pinning policy

Third-party actions are pinned to a full commit SHA with a `# vX.Y.Z`
comment; Dependabot proposes the updates. First-party reusable workflows
and actions are referenced by tag, not SHA: `ractive/release-workflows`
by exact tag (`@v0.2.1`), `ractive/setup-hyalo@v1` floating on its major
tag, as in hyalo. Both repositories belong to the owner, so a tag is a
reviewed release; Dependabot ignores them.
