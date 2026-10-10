---
type: iteration
title: "Iteration 36b: saturnus-core, the saturnus CLI, satx"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus
  - satx
  - consolidation
branch: iter-36b/renames
---

# Iteration 36b: saturnus-core, the saturnus CLI, satx

The second consolidation iteration ([[decision-log]], "2026-10-10
(consolidation)"). These are real renames, not `package =` aliases, and
there is no compatibility for the old names.

| Today | Directory | Package | Binary |
|---|---|---|---|
| saturnus (core) | `crates/saturnus-core` | `saturnus-core` | |
| saturnus-cli | `crates/saturnus-cli` | `saturnus` | `saturnus` |
| satx-cli (from 36a) | `crates/satx-cli` | `satx` | `satx` |

The Linux packages:
- the CLI's .deb and .rpm are called `saturnus`;
- the desktop app's are called `saturnus-app`, through a Linux-only
  `productName`, so macOS and Windows keep "saturnus";
- satx's are called `satx`.

## Tasks

- [ ] `git mv crates/saturnus crates/saturnus-core`; package `saturnus-core`. Change `saturnus::` to `saturnus_core::` (95 places in 49 files), plus the feature references, the profile keys and the `documentation` URL
- [ ] Package `saturnus-cli` becomes `saturnus`. Change every `-p saturnus-cli` and `-p saturnus` in the justfile, CI, scripts, tests and READMEs (64 places). The deb/rpm metadata name becomes `saturnus`. `--version` prints "saturnus"
- [ ] `crates/saturnus-tauri/tauri.linux.conf.json` with `{"productName": "saturnus-app"}`. The window title stays "saturnus"
- [ ] Package `satx-cli` becomes `satx`. Read through the help text, prompts, README and `kb/satx` that the import's text replacement produced
- [ ] Use the map from [[iterations/iteration-36a-import]] on the 139 lines of our own files that use the old name: the README, kb (the decision log and the older iterations too), deny.toml, `web/about.json` with `scripts/about-json.py` (the satx credit links to `ractive/saturnus/tree/main/crates/satx-cli`), `scripts/diff-vs-saturnng.sh`, and the comments in saturnus-cli `control/mod.rs` and `serial.rs`, the core's `tests/e2e.rs` and saturnus-objects `charset.rs`. Then `git grep -i hptx` prints nothing
- [ ] READMEs, crate READMEs, CHANGELOG ("Unreleased: renames"), kb/docs (architecture, releasing, ci, test-policy, knowledge-sources, control-api-security)
- [ ] `just package` passes for every crate that is published (description, licence, `repository = ractive/saturnus`, README)
- [ ] `just gates`, then /create-pr, /review-pr, /merge-pr
- [ ] calculator-knowledgebase: the satx entry in the README's project list, CLAUDE.md and all 26 wiki files, with the same map (its own PR)
- [ ] CLAUDE.md here: the hardware wiki pointer moves from `~/devel/hp-literature/` to calculator-knowledgebase. The owner's memory notes use the new names
- [ ] saturnus PR #103: rebased onto the new names, renumbered to iterations 37–40, URL `/saturnus/transfer/`
- [ ] The titles and bodies of the 14 saturnus PRs that use the old name (`gh pr edit`), and the owner's own review comments (`gh api -X PATCH`)
- [ ] Owner: the GitHub About text of ractive/saturnus

## Acceptance

- `cargo install --path crates/saturnus-cli` installs `saturnus`.
  `cargo install --path crates/satx-cli` installs `satx`.
- `git grep -i hptx` prints nothing in this repository and in
  calculator-knowledgebase.
- Nothing is published.
