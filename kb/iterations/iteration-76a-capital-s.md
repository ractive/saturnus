---
type: iteration
title: "Iteration 76a: the product is Saturnus, with a capital S"
date: 2026-11-16
status: in-progress
tags:
  - iteration
  - copy
  - web
branch: iter-76a/capital-s
---

# Iteration 76a: the product is Saturnus, with a capital S

The owner (2026-11-16): "capitalize Saturnus everywhere". The desktop
app is "Saturnus" on macOS and Windows since be79818a (`productName`);
the prose still says "saturnus". This iteration covers everything people
read **except the content pages** (`web/landing/`, `web/docs/`), which
[[iterations/iteration-76b-seo-content]] rewrites at the same time.

## The rule

- **Saturnus** when the text names the product: "Saturnus emulates…",
  "the Saturnus desktop app", window and page titles, the PWA's name,
  the Linux menu entry (`Name=`), crate descriptions, docs.rs prose,
  READMEs, help text that describes the program.
- **saturnus** stays lowercase wherever it is an identifier: the CLI
  command and its examples (`saturnus rom fetch`), crate and package
  names (`saturnus-core`, `saturnus-app`), URLs and paths
  (`/saturnus/app/`, `ractive.ch/saturnus/`), the bundle id
  `ch.ractive.saturnus`, URI schemes (`saturnus://`), env vars
  (`SATURNUS_ROM_DIR`), file names (`saturnus.mcpb`, `saturnus-mac.dmg`),
  the GitHub repository, anything in backticks or code.
- `satx` stays lowercase (a command name).
- Not touched: `kb/` history (decision log, iterations, research), the
  released sections of `CHANGELOG.md`, `.github/release-notes/` of past
  releases, and the calculator wiki. The changelog's intro and its
  Unreleased section do change.

## Tasks

- [ ] `README.md`, `web/README.md`, the crate READMEs, the
  `description` fields of every `Cargo.toml`, crate-level docs
  (`//!`) shown on docs.rs.
- [ ] The CLIs' (`saturnus`, `satx`) help and messages where they name
  the product; MCP server name/instructions and tool descriptions where
  they name the product (not the server id if clients key on it).
- [ ] The desktop app: the window title in `tauri.conf.json`, its menus
  and dialogs, `crates/saturnus-tauri/linux/saturnus.desktop`
  (`Name=Saturnus`), the bundle's `shortDescription`/`longDescription`.
- [ ] The web app (`web/*.js`, `web/index.html`, `web/tx/`,
  `web/shared/`, `web/components/`, `web/pwa/` manifest name and
  short_name, `web/mcpb/`), and the sources of generated texts
  (`scripts/about-json.py`, `crates/saturnus-cli/data/commands/reference.json`
  then `just commands`), the Homebrew/Scoop/cask templates' `name`/`desc`.
- [ ] `CHANGELOG.md` intro and Unreleased; `kb/docs/writing-style.md`
  gets the rule above in its glossary.
- [ ] Tests that assert the old strings are updated, not deleted.
