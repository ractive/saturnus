---
title: "Owner checks left from iterations 32 to 53"
type: backlog
date: 2026-10-19
status: planned
tags:
  - backlog
  - owner
priority: low
---

# Owner checks left from iterations 32 to 53

Checks that need a person (real calculators, phones, other browsers, the desktop app by hand, decisions) left open when the plans were closed. Entries starting "Unverified:" are implementation tasks whose completion could not be confirmed from the repository.

## Iteration 32: [[iterations/iteration-32-memory-actions]]

- [ ] Owner: the menu, context menu and divider in the desktop app and a desktop browser, and the menu on the phone

## Iteration 33: [[iterations/iteration-33-move-copy]]

- [ ] Owner: Copy to… and Move to… on a real calculator's ROM in the desktop app and a browser, the picker on the phone, and the double-click to edit

## Iteration 34: [[iterations/iteration-34-saved-states]]

- [ ] Owner: the four commands in a browser and in the desktop app, on
  a phone too

## Iteration 35: [[iterations/iteration-35-tour]]

- [ ] Owner acceptance: the tour reads well and does not overwhelm

## Iteration 36d: [[iterations/iteration-36d-site]]

- [ ] Owner, once: list `httpdocs/saturnus/` on the server after that deploy. It holds only `.htaccess`, `.ftp-deploy-sync-state.json`, `index.html`, the landing files, `sw.js` and `emulator/`. Anything else never went through the action, so the sync state does not know it (for example a leftover of the first copy, ractive.ch PR 8). Delete it by hand
- [ ] Owner: in that browser, `/saturnus/emulator/` lists the kept ROMs, and exactly one worker is left, scoped to `/saturnus/emulator/`. Install the web app again from `/saturnus/emulator/`

## Iteration 37: [[iterations/iteration-37-tx-skeleton]]

- [ ] Owner: on the 48SX and the 49G in Chrome or Edge with a USB serial cable: connect without typing settings, the model shown right, list, get a program and a GROB, put them back under new names, the preview of both; then the same against the emulated 48GX in Firefox or Safari

## Iteration 38: [[iterations/iteration-38-tx-two-panes]]

- [ ] Owner: on the 48SX and the 49G: a folder chosen in Chrome, a drag each way, rename and purge on both sides, a backup and a restore, the folder copy of HOME; the emulated 49G to the real 49G by drag; in Firefox the fallback with the emulated calculator

## Iteration 39: [[iterations/iteration-39-tx-aplets]]

- [ ] Owner: on the 38G (the owner's 10-pin cable): send an aplet to a folder, receive it back, receive an aplet made on the emulated 38G; XModem put and get on the 49G

## Iteration 40: [[iterations/iteration-40-tx-desktop]]

- [ ] Owner: the macOS app opens without the quarantine dance (signed and notarised), installs from the cask; on the 48SX, 49G and 38G with the app: connect, a drag each way, a backup; the Windows and Linux installers on a real port if a machine is at hand

## Iteration 41: [[iterations/iteration-41-shared-calculators]]

- [ ] Owner: the 48GX used in the emulator, then in saturnus tx and back (a variable made in one shows in the other); the emulator tab in the background for a minute, then taken over from saturnus tx; on the phone the same with the emulator app
- [ ] The 48SX and the 49G the same way as the 48GX (the 49G's state holds its flash)
- [ ] The emulator tab left in the background for a minute (or frozen in Chrome's `chrome://discards`), then the calculator taken from saturnus tx: it takes over after about 3 s, and the emulator tab, brought back, shows "In use by saturnus tx." and does not overwrite what tx did
- [ ] A copy running in saturnus tx when the emulator asks (Take it back): the copy ends, then the calculator goes back, not in SERVER
- [ ] saturnus tx closed (tab closed) while it has the calculator: the emulator takes it at once, at its stack (it ends the server itself since 2026-10-13), and the copies made are there
- [ ] The emulator page opened while saturnus tx has the calculator: it shows "In use by saturnus tx." and does not take it until you press Take it back or choose the model
- [ ] Two emulator tabs: the second shows "In use in another emulator tab."
- [ ] A calculator turned off in the emulator, then taken in saturnus tx: it is turned on and SERVER starts; a command line left open is cancelled first
- [ ] On the phone: the emulator app and saturnus tx installed side by side, the handover both ways; Android discarding the background tab
- [ ] Mixed builds: an installed page on an older build meeting a state saved by a newer one says "Reload this page to use it." (only when the state format changes; none does in this iteration)
- [ ] "Choose a ROM file…" in saturnus tx still starts an empty calculator that leaves the emulator's alone

## Iteration 42: [[iterations/iteration-42-pioneer-27s-17b]]

- [ ] Owner: put the images in `roms/` (27S, 17BII English, 17BII
  international if you have it, 17B) and say where each came from.
- [ ] Owner: look at the three skins (1280 and 390 wide screenshots in
  the PR) against your memory of the calculators and published photos.
- [ ] Owner: decide whether the landing page lists them.
- [ ] Unverified: Each skin is operable by mouse and touch; screenshots at 1280 and
  390 wide, headless Chrome only (never cmux-cua or screencapture), two
  per skin, in the PR.

## Iteration 43a: [[iterations/iteration-43a-hp28s-machine]]

- [ ] Owner: put the 28S image in `roms/` and say where it came from
  (own dump over IR to the 48SX, or a circulating image).
- [ ] Owner: read the feasibility verdict and the map decision before the
  core work goes on (a comment on the PR is enough).

## Iteration 43b: [[iterations/iteration-43b-hp28s-skin-hosts]]

- [ ] Owner: the clamshell layout on the desktop and on the phone (the
  PR's screenshots, then the deployed preview on your phone).
- [ ] Owner: the skin's look against your memory and published photos.
- [ ] Owner: landing page card yes or no.
- [ ] Unverified: The 28S runs in the web emulator and the desktop app from a chosen
  ROM file, operable by mouse, touch and computer keyboard.

## Iteration 44: [[iterations/iteration-44-hp19bii]]

- [ ] Owner: decide whether saturnus runs a class-b 19BII image at all;
  if yes, put it in `roms/`.
- [ ] Owner: the skin's look (screenshots at 1280 and 390 in the PR).

## Iteration 45: [[iterations/iteration-45-tx-progress-console]]

- [ ] Owner: on a real 48SX over the cable, click a picture: the preview says what it reads with the bar and the time; the console shows the packets, a NAK in red when the cable hiccups
- [ ] Owner: "Open in the emulator" on the left pane's emulated 48GX takes it over in the emulator tab; saturnus tx says "In use by the emulator"
- [ ] Owner: in Firefox, the right pane says "Files in this browser" and how to save a file

## Iteration 46: [[iterations/iteration-46-shared-worker]]

- [ ] Owner: the emulator and saturnus tx side by side on the 48SX and the 49G as on the 48GX (a variable typed in one shows in the other at once)
- [ ] Owner: on the phone (Chrome on Android): both installed pages, a copy from tx while the emulator shows the calculator; Samsung Internet shows the plain message in tx
- [ ] Owner: Safari (16 or later): the same side by side
- [ ] Owner: the emulator tab in the background for a minute while tx copies: the copy runs, the emulator shows the result when brought back
- [ ] Owner: after a deploy with an emulator tab still open on the old build, a newly opened tx tab says "Reload the page" (and works once both are reloaded)

## Iteration 47: [[iterations/iteration-47-hub-device]]

- [ ] Owner: on the 48GX and 49G, emulator and tx side by side: a drag each way, rename, purge, backup; typing in the emulator meanwhile; the chip's Show brings tx forward — superseded by the one app (iterations 50d and 51); the owner checks iteration 50d/51 instead
- [ ] Owner: the new landing headline and the nav labels read right on the site and on a phone

## Iteration 49: [[iterations/iteration-49-tx-desktop-roots]]

- [ ] Owner: in the desktop app on macOS, a file outside home is listed but not changed until its folder is opened with "Open a folder…", and after a restart that folder is still writable and in Recent folders; a USB stick picked once stays writable; deleting `~/Documents` recursively is refused; the emulator app's 48GX ROM shows under "Add an emulated calculator" and boots — superseded by the one app (iterations 50d and 51); the owner checks iteration 50d/51 instead

## Iteration 50d: [[iterations/iteration-50d-one-document]]

- [ ] Owner: on the Mac (Chrome) and a phone: home lists the kept calculators; Calculator → Files → back keeps the calculator running; a cable calculator connects in Files; the installed app opens where it should

## Iteration 51: [[iterations/iteration-51-one-desktop-app]]

- [ ] Owner: on macOS, Files on the running 48GX lists and copies; a cable calculator connects; a minimised calculator keeps time

## Iteration 52: [[iterations/iteration-52-one-app-sweep]]

- [ ] Left to the owner (other devices, not reachable from here): uninstall the installed web apps "saturnus" and "saturnus tx" on the phone and iPad, without clearing ractive.ch's site data (kept ROMs, states, settings live there). On iOS and iPadOS an installed home-screen app has storage of its own: ROMs kept inside it go with it; keep their files before uninstalling

## Iteration 53: [[iterations/iteration-53-roms-on-home]]

- [ ] On the web: New calculator → Choose… a ROM → the calculator starts; ⋯ Change ROM… and Remove work on the home page; the Calculator panel has no ROM section
- [ ] On the desktop app: the same
