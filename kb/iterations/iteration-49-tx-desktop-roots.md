---
title: "Iteration 49: a Rust-side safety net for the desktop tx app's files, and the emulator app's ROMs in tx"
type: iteration
date: 2026-10-14
status: planned
branch: iter-49/tx-desktop-roots
tags:
  - iteration
  - saturnus-tx
  - tauri
  - security
---

# Iteration 49: roots for the desktop tx app, and the emulator's ROMs

From the security review of 2026-10-14 (finding A). The owner's decision
of 2026-10-11 stands: the desktop app's folder pane is a file manager
over everything the user's account may reach
(`crates/saturnus-tx-tauri/src/lib.rs`, the scope note). Today every
check is in the page: `folder_read`, `folder_create`, `folder_write`,
`folder_mkdir`, `folder_remove` (with `recursive`) and `folder_rename`
take any list of plain names (`files::resolve`, `files::check_name`), so
one page bug (or injected script, despite the CSP) can read any file or
`remove_dir_all` the home folder. This iteration adds a net on the Rust
side that a page cannot talk its way past, without narrowing what the
user can do by hand.

## Design

### Roots

- **`Roots`** (Tauri managed state, `Mutex<Vec<PathBuf>>`, canonical
  paths): the home folder (`app.path().home_dir()`, as `folder_home`)
  from the start, and every folder the user picks in `folder_pick`
  (Tauri's dialog, opened from Rust: the page cannot add a root itself).
  For this run of the app only; nothing is stored.
- **Checked**: `folder_read`, `folder_create`, `folder_write`,
  `folder_mkdir`, `folder_remove`, `folder_rename` resolve the path
  (`files::resolve`), canonicalise its nearest existing ancestor
  (`std::fs::canonicalize`; a new file's parent, so a link inside home
  that points out of it does not count as inside) and refuse a path that
  is not under a root with `NoModificationAllowedError: outside the
  folders this app may change. Open it with "Open a folder…" first.`,
  which `web/tx/folder.js` shows as it shows the other errors.
- **Not checked**: `folder_list` and `folder_stat` (browsing the whole
  computer stays, as the file-manager decision wants), `folder_home`,
  `save_file` (its path comes from the save dialog).
- **Recursive remove**: `files::remove` with `recursive` refuses a path
  whose canonical form is the home folder or a direct child of it
  (`~/Documents`, `~/Library`, `~/.ssh`), whatever the roots, with
  "this folder is not removed from here; remove it in the system's file
  manager". A non-recursive remove of an empty one stays possible. The
  existing "never the top or a drive" rule stays.
- **Aplet serving from the home folder** (38G, 39G, 40G): PR 173 refuses
  it in the page (`web/tx/serve.js`, `HOME_REFUSED`, with `homeNames()`
  from `web/tx/tauri.js`). The Rust side cannot tell a served write from
  a drag; the page marks the aplet server's writes with an `x-serve: 1`
  header on `folder_write` and `folder_create` (`NativeFileHandle` and
  `NativeDirectory` in `web/tx/tauri.js`), and Rust refuses those directly in the home
  folder. This guards against a page bug, not a hostile page (which
  would drop the header): stated as such in the scope note.
- **Tests** in `crates/saturnus-tx-tauri/src/files.rs` (the module gets
  a `Roots` type with `check(&self, &Path) -> Result<()>` so the rules
  are unit-tested without Tauri): a path under home passes; one outside
  is refused; a picked folder outside home passes after `add`; a link
  inside home pointing outside is refused (Unix); `..` cannot arise
  (names checked); recursive remove of home, of `home/x` refused, of
  `home/x/y` allowed; a served write in home refused, in `home/x`
  allowed. Temp directories stand in for home (the module's
  `temp_dir` test helper).

### The emulator app's ROMs in desktop tx

- Where they are: the emulator desktop app (`crates/saturnus-tauri`,
  identifier `ch.ractive.saturnus`) keeps the ROM paths in
  `settings.json` in its config folder (`settings_file` in
  `crates/saturnus-tauri/src/lib.rs`: `app_config_dir()`; format in
  `crates/saturnus-tauri/src/roms.rs` `load`/`to_json`: `{"version": 1,
  "roms": {"48gx": {"path", "sha256"}}, ...}`), downloaded images under
  `roms` in its data folder (paths in the same file). Desktop tx
  (`ch.ractive.saturnus-tx`) has its own webview origin, so
  `keptRoms()` (`web/tx/kept-roms.js`, IndexedDB `saturnus-roms`) finds
  nothing there today.
- **Smallest way**: two read-only commands in saturnus-tx-tauri, no new
  dependency: `emulator_roms` reads that `settings.json` (the config
  folder of `ch.ractive.saturnus`, beside tx's own: `app.path().config_dir()`
  joined with the identifier; capped at 64 KiB like the emulator's
  `MAX_SETTINGS`) and returns `[{model, fileName, sha256}]` for files
  that exist; `emulator_rom(model)` returns that file's bytes (4 MiB cap,
  as the emulator's `read_capped`). Paths never reach the page
  (web/protocol.md, "ROM slots"). Not root-checked: the path is the
  emulator app's, not the page's.
- The page: `keptRoms()` and `keptRomBytes()` call them when `isTauri()`,
  checks the SHA-256 of the bytes against `sha256` (a changed file is
  left out with a line, as the emulator's slots say `changed`). In the
  desktop app such a ROM runs as a **private** calculator
  (`web/tx/emulator-worker.js`, SERVER typed): the two apps are separate
  processes and share no live calculator ([[iterations/iteration-46-shared-worker]],
  "Not shared"). `addKept` (`web/tx/app.js`) and `web/tx/serve.js`
  (`keptRoms(undefined, APLET_MODELS)`) take them as they take the
  browser's.
- Debug builds read `SATURNUS_SETTINGS_DIR` as the emulator app does,
  so tests never touch the user's settings.

## Tasks

- [ ] `files::Roots` (canonical roots, `add`, `check`), the recursive-remove rule and the served-write rule in `crates/saturnus-tx-tauri/src/files.rs`, with the unit tests above
- [ ] `lib.rs`: `Roots` managed state seeded with home; `folder_pick` adds the chosen folder; the six commands check it; the `x-serve` header read in `folder_write`/`folder_create`; the scope note rewritten
- [ ] `web/tx/tauri.js` (`NativeFileHandle`, `NativeDirectory`): the `x-serve` header for the aplet server's writes; `web/tx/folder.js` shows the new refusal in plain words; `web/test/tx-tauri.test.mjs` covers both
- [ ] `emulator_roms` and `emulator_rom` with tests on a temp settings folder (a missing file left out, an over-cap file refused, a malformed settings file read as none); `web/tx/kept-roms.js` uses them in the desktop app, with the SHA-256 check
- [ ] Check what PR 173 changed in `files.rs`, `lib.rs` and `serve.js` once it is merged and build on it (rebase this branch after it)
- [ ] `just tauri` (clippy in both profiles, tests) green; `just tx-app` builds
- [ ] Docs: [[docs/saturnus-tx]] (the desktop app: roots, what is refused and why, the emulator's ROMs), `web/protocol.md` unchanged (no protocol command), CHANGELOG (Unreleased), decision log (roots per run, list and stat left open, the `x-serve` header's limits)
- [ ] Owner: in the desktop app on macOS, a file outside home is listed but not changed until its folder is opened with "Open a folder…"; deleting `~/Documents` recursively is refused; the emulator app's 48GX ROM shows under "Add an emulated calculator" and boots

## Acceptance

- No folder command but `folder_list`, `folder_stat` and `folder_home`
  touches a path outside home and the folders picked in this run (unit
  tests on `Roots`, a link out of home included).
- A recursive remove of home or a direct child of home is refused in
  Rust, whatever the page sends.
- The aplet server's writes directly into home are refused in Rust as
  well as in the page.
- With the emulator desktop app's ROMs set, desktop tx lists them and
  boots one as a private calculator; no path reaches the page.

## Out of scope

- Persisting picked folders across runs (open question 1).
- Sandboxing the app (macOS App Sandbox, Flatpak portals).
- Sharing a live calculator between the two desktop apps.

## Open questions for the owner

1. Picked folders as roots for this run only (this plan), or remembered
   across runs (then stored in tx's settings and shown in a list the
   user can clear)?
2. Leave `folder_list`/`folder_stat` unrestricted (browse everything,
   change only inside roots, this plan), or confine browsing too?
3. A removable drive (`/Volumes/...`, `D:\`) needs "Open a folder…"
   before a write: acceptable, or should mounted volumes count as roots?
