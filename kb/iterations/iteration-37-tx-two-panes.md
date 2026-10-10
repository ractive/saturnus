---
type: iteration
title: "Iteration 37: saturnus-tx two panes, local folder, drag and drop, backup and restore"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - web
branch: iter-37/tx-two-panes
---

# Iteration 37: two panes, the local folder, backup and restore

The HPComm shape ([[docs/saturnus-tx]], "Layout"): two panes, each a
device or a folder; drag and drop between them and from the computer's
file manager; the operations that change things (rename, purge, new
directory); backup and restore of a whole calculator.

## What it does

- **Two panes**, each showing a place: a calculator (real or emulated)
  or a folder. Default: the calculator left, a folder right. Stacked
  with a switch below 760 px.
- **The folder**: the File System Access API in Chrome and Edge (handle
  kept, permission asked again per visit); elsewhere a "Files" list in
  the page (OPFS or memory) with uploads, drops and downloads (a `.zip`
  for a directory or a selection).
- **Drag and drop** pane to pane and from the file manager (files; in
  Chromium also folders); F5 copies the selection to the other pane, F2
  renames, Delete purges; "Copy to the other pane" in the menus.
- **Device to device**: emulated to real and back, a get then a put,
  with a warning for a 48 object going to a 49G or back.
- **Rename, purge, new directory** on both sides; a non-empty directory
  asks first with its count; "Replace X?" before any name in use, on
  either side.
- **Directories**: a calculator directory becomes a folder (one get per
  variable), or one `.hp` with "Get as one file"; a folder put to the
  calculator becomes a directory with its contents.
- **Backup**: the free memory checked first, the backup file named with
  the model and the time; "Copy everything to a folder" as the
  alternative that needs no free memory.
- **Restore**: the modal question, the put and RESTORE, the pane says
  the calculator restarted and offers to reconnect.
- **Graphics screen** (PICT) in the device's "⋯", shown as a GROB.
- **A queue**: one operation per device at a time, the rest queued,
  visible, cancellable before they start; progress in bytes.

## Tasks

- [ ] `Folder` interface: File System Access, the in-page fallback (OPFS, memory), downloads and `.zip`; tests with fakes
- [ ] saturnus-tx: recursive get and put of directories, rename, purge (`PGDIR` with the count), mkdir, backup with the memory check, restore, PICT, the copy plan between devices with the header check; unit tests over recorded traces
- [ ] Web: two panes, the place switcher, the stacked narrow layout, drag and drop (pane to pane, file manager to pane, optional `DownloadURL` drag out), the keys, the queue and progress, the replace and purge questions (`confirm.js`)
- [ ] Tests: node tests for the panes' actions and the queue; the page with `FakeLink` and a fake folder (drag, replace, purge, overflow at the five widths); `just tx-e2e DIR`: backup and restore, folder copy of HOME and back byte for byte, emulated 48GX to emulated 49G
- [ ] Docs: [[docs/saturnus-tx]] updated where the build differs, `web/README.md`, CHANGELOG
- [ ] Owner: on the 48SX and the 49G: a folder chosen in Chrome, a drag each way, rename and purge on both sides, a backup and a restore, the folder copy of HOME; the emulated 49G to the real 49G by drag; in Firefox the fallback with the emulated calculator

## Acceptance

- Every operation of the design's "Flows" for the 48/49 works against
  the emulated 48SX, 48GX and 49G in `just tx-e2e`.
- Nothing on either side is replaced or removed without a question.
- A backup restored onto a fresh emulated calculator gives back the same
  HOME (names, sizes, checksums).
