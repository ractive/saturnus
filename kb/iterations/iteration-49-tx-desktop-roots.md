---
title: "Iteration 49: a Rust-side safety net for the desktop tx app's files, and the emulator app's ROMs in tx"
type: iteration
date: 2026-10-14
status: completed
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
  **Picked folders are remembered across runs** (owner, 2026-10-14):
  Rust keeps them in `folders.json` in tx's own config folder
  (`app_config_dir()` of `ch.ractive.saturnus-tx`, written only by Rust,
  mode 0600 on Unix as the emulator's `settings.json`) and loads them at
  start. A picked folder that is gone is dropped from the roots when the
  file is read. Mounted volumes and drives (`/Volumes/...`, `D:\`) are
  **not** automatic roots: the user picks one once and it is remembered
  like any other folder.
- **Checked**: `folder_read`, `folder_create`, `folder_write`,
  `folder_mkdir`, `folder_remove`, `folder_rename` resolve the path
  (`files::resolve`), canonicalise its nearest existing ancestor
  (`std::fs::canonicalize`; a new file's parent, so a link inside home
  that points out of it does not count as inside) and refuse a path that
  is not under a root with `NoModificationAllowedError: outside the
  folders this app may change. Open it with "Open a folder…" first.`,
  which `web/tx/folder.js` shows as it shows the other errors.
- **Not checked**: `folder_list` and `folder_stat` (browsing the whole
  computer stays unconfined, owner 2026-10-14), `folder_home`,
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

### Recent folders (desktop and web)

A "Recent folders" section in the folder pane's place switcher
(`openSwitcher` in `web/tx/pane.js`, the "Show in this pane" menu): the
last 8 folders the pane showed, newest first, each one click to reopen,
with "Clear recent folders" under them.

- **Desktop**: Rust keeps the list in the same `folders.json` as
  `{names, picked, used}` entries. New commands: `folder_recent` (the
  list, entries that no longer exist left out), `folder_used(path)` (the
  page reports a folder the pane opened; it moves to the top, the list
  is cut at 8) and `folder_recent_clear`. `folder_used` never makes a
  root: only an entry with `picked` (set by `folder_pick`) is one, so a
  page cannot widen the roots by reporting a path. A recent folder under
  home or under a picked folder is inside a root anyway; reopening one
  is `folder_list` on its names.
- **Web** (Chrome and Edge, File System Access): today `folder.js` keeps
  one picked handle (`handleStore`: `TRANSFER_DB`, store `FOLDER_STORE`,
  key `FOLDER_KEY`; `pickFolder`, `keptFolder`, `forgetFolder`). It
  becomes a list of up to 8 handles in `FOLDER_STORE` (key: an id;
  value `{handle, name, used}`), a folder picked again found with
  `isSameEntry` and moved to the top. Opening a recent one asks
  `requestPermission({mode: "readwrite"})` in the click, so it is one
  click when the browser asks again on a new visit. `FOLDER_KEY` and the
  single-handle code go; the folder kept today is not carried over (no
  migrations, owner's standing rule): the user picks it once more.
- Browsers without the picker (OPFS or memory "Files") show no recent
  list.

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

- [x] `files::Roots` (canonical roots, `add`, `check`), the recursive-remove rule and the served-write rule in `crates/saturnus-tx-tauri/src/files.rs`, with the unit tests above
- [x] `lib.rs`: `Roots` managed state seeded with home; `folder_pick` adds the chosen folder; the six commands check it; the `x-serve` header read in `folder_write`/`folder_create`; the scope note rewritten
- [x] `web/tx/tauri.js` (`NativeFileHandle`, `NativeDirectory`): the `x-serve` header for the aplet server's writes; `web/tx/folder.js` shows the new refusal in plain words; `web/test/tx-tauri.test.mjs` covers both
- [x] `folders.json` in tx's config folder (Rust-only writes, 0600): picked folders loaded as roots at start; `folder_recent`, `folder_used`, `folder_recent_clear`; tests: a picked folder is a root after a restart (a new `Roots` from the file), `folder_used` of an outside path adds it to the list but not to the roots, a vanished entry is dropped, the list is cut at 8, a malformed file reads as empty
- [x] Recent folders in `openSwitcher` (`web/tx/pane.js`): desktop on the new commands; web on a handle list in `FOLDER_STORE` replacing `FOLDER_KEY` (`handleStore`, `pickFolder`, `keptFolder`, `forgetFolder` in `web/tx/folder.js`, names in `web/shared/storage-names.js`), one-click re-permission; `web/test/tx-folder.test.mjs` and `tx-tauri.test.mjs` cover the list (fake handles and a fake `requestPermission`)
- [x] `emulator_roms` and `emulator_rom` with tests on a temp settings folder (a missing file left out, an over-cap file refused, a malformed settings file read as none); `web/tx/kept-roms.js` uses them in the desktop app, with the SHA-256 check
- [x] Check what PR 173 changed in `files.rs`, `lib.rs` and `serve.js` once it is merged and build on it (rebase this branch after it)
- [x] `just tauri` (clippy in both profiles, tests) green; `just tx-app` builds
- [x] Docs: [[docs/saturnus-tx]] (the desktop app: roots, what is refused and why, the emulator's ROMs), `web/protocol.md` unchanged (no protocol command), CHANGELOG (Unreleased), decision log (picked folders remembered, list and stat left open, volumes picked once, recent folders, the `x-serve` header's limits)
- [ ] Owner: in the desktop app on macOS, a file outside home is listed but not changed until its folder is opened with "Open a folder…", and after a restart that folder is still writable and in Recent folders; a USB stick picked once stays writable; deleting `~/Documents` recursively is refused; the emulator app's 48GX ROM shows under "Add an emulated calculator" and boots

## Acceptance

- No folder command but `folder_list`, `folder_stat` and `folder_home`
  touches a path outside home and the folders ever picked (remembered) (unit
  tests on `Roots`, a link out of home included).
- A recursive remove of home or a direct child of home is refused in
  Rust, whatever the page sends.
- The aplet server's writes directly into home are refused in Rust as
  well as in the page.
- A folder picked once is a root after the app restarts; a path the
  page reports with `folder_used` is never a root unless picked.
- The pane's place switcher lists the last folders used (up to 8) on
  desktop and in Chrome; one click reopens one, asking the browser's
  permission in that click when needed.
- With the emulator desktop app's ROMs set, desktop tx lists them and
  boots one as a private calculator; no path reaches the page.

## Out of scope

- Mounted volumes as automatic roots (owner: pick once).
- Confining browsing (owner: not confined).
- Sandboxing the app (macOS App Sandbox, Flatpak portals).
- Sharing a live calculator between the two desktop apps.

## Settled by the owner (2026-10-14)

1. Picked folders are remembered across runs, and the folder pane gets a
   "Recent folders" list in its place switcher, on desktop (picked
   folders in it are roots) and on the web (handles in IndexedDB, one
   click to grant the permission again).
2. Browsing is not confined: `folder_list` and `folder_stat` stay open.
3. Mounted volumes are not automatic roots: picked once, remembered.
