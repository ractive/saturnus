---
type: iteration
title: "Iteration 70: Files coherence pass after Kermit, XSERV and fast-transfer changes"
date: 2026-11-08
status: in-progress
tags:
  - iteration
  - web
  - ux
branch: iter-70/files-coherence
---

# Iteration 70: Files coherence pass

Iterations 68a, 68b and 69 changed how Files reaches a calculator: the
aplet server reads the folder at each RECV, "Add a calculator" asks which
kind first, fast transfer puts `« 'NAME' XSEND »` on the stack, the restart
hint depends on the model, Disconnect lets go of the port after 30 s, and
on the 49G Files talks to XSERV, switching to the Kermit server and back
for what XSERV cannot do. Owner 2026-11-08: "Check if everything's
coherent. Check if the UI still is on par with how things work now. Maybe
the UI can be simplified." After [[iterations/iteration-69-files-over-xserv]].

## Tasks

- [x] **Walk every flow** on the emulators (web page and desktop app where
  it differs), with screenshots of anything wrong: connect to a 48SX,
  48GX and 49G over Kermit; a 49G in XSERV with a list, both transfers,
  a change, a directory, PICT and a backup through the Kermit nesting;
  fast transfer over Kermit on a 48GX and a 49G; the restart hints;
  Disconnect; the emulated calculators; the 38G, 39G and 40G serve screen
  from "Add a calculator"; failures (a calculator that stops answering,
  the wrong server running, a busy port).
- [x] **Words against behaviour:** every instruction Files gives (keys,
  `SERVER` or `XSERV`, restart hints, the settings line "Connected now:
  …"), the help and docs (`web/docs/pages/files.md`, `web/README.md`,
  the satx README, `kb/docs/saturnus-tx.md`): each says what the code does.
- [x] **Simplifications:** list what the changes made redundant or
  confusing, for example fast-transfer items where XSERV makes every
  transfer fast, two ways into the serve screen ("Add a calculator" and
  the Change menu), per-model connect texts that could be one, settings
  Files can now handle itself (transfer mode, server choice).
- [ ] **Owner decision** on the simplifications that change how Files
  works (a short list with screenshots); fix wording and plain
  inconsistencies directly.
- [ ] Implement what the owner agrees to; tests and docs.

## Acceptance

- Every flow above works and says what it does; no instruction names a
  key, command or menu that does not match the model and the server.
- `just gates`, `hyalo lint` clean.

## Walked (2026-11-08, first part)

On the page in headless Firefox 159 (ff-rdp): a 48GX and a 49G started
from ROM files in Files (Kermit, fast transfer with "Press the keys for
me", reconnect, turn off); the same calculators connected as a cable
would be (`emulated: false`, the cable texts) on their Kermit servers
and the 49G on XSERV (list, copies both ways, a text file and a string
through the Kermit nesting, a new directory, a rename, a refused rename,
a purge, a directory walk, PICT, a backup, a restore and the reconnect
into XSERV, Disconnect, a calculator that stops answering); a fast
transfer on the 49G's Kermit server and the reconnect into XSERV after
it; "Add a calculator" both ways; the serve screen with an emulated
38G. satx against the emulated 49G (`saturnus run --autostart --serial
tcp:4866`, `satx finish`, `saturnus ctl type --run XSERV`): `info`
names XSERV, Kermit commands stop with a hint, `xserv ls`, `mem`,
`put`, `quit`. Screenshots in the session's scratchpad `iter-70/`.

Fixed directly (wording and plain inconsistencies): the settings note no
longer says every connection is Kermit, and "Connected now" names XSERV;
the 48SX's connect note says α twice (α once types one letter, flag -60
clear); a refused change over XSERV no longer reads "Unexpected reply:
the calculator did not rename X, …" (`TxError::Unconfirmed`); a
disconnect whose FINISH got no answer no longer also says "The
calculator has left server mode."; a new connection, a restore and a
fast transfer clear the old preview (a graphics screen stayed shown
across disconnects); an empty (0 × 0) graphics screen says it is empty;
the docs: the 49G's Kermit server is started by typing `SERVER` (the
satx and Kermit pages still said right-shift → quickly), `web/README.md`
on connect, XSERV and the fast transfer, `kb/docs/saturnus-tx.md` on
the serve screen's ways in, the settings as built and the menus.

## Proposals for the owner

Each changes how Files works; none is built. Screenshots: the session
scratchpad's `iter-70/`.

1. **No fast transfer on the 49G.** Now: on a 49G's Kermit server the
   status says XSERV copies faster, and the menus still offer "Fast
   transfer … (XModem)…", a seven-step dance (server ends, a key on the
   calculator, wait, start the server again, reconnect) for the same
   speed. Proposed: fast transfer on the 48GX only; on the 49G the XSERV
   hint is the one way to fast copies. Cost: `XMODEM_MODELS`, the 49G
   cases of `tx-xmodem.test.mjs` and the page's 49G xmodem trace test,
   docs; satx keeps `--protocol xmodem`. Screenshots
   `15-fast-get-49g-cable-dialog.png`, `14-49g-kermit-xserv-hint.png`.
2. **One restart text per model.** Now: after a restore the card says
   "Start its server on the calculator again" with no keys; after a fast
   transfer the 49G is told `SERVER`, although Files prefers XSERV and
   the reconnect finds either (emulated 49G: the reconnect into XSERV
   after a fast transfer and after a restore worked, algebraic mode
   back). Proposed: both use the connect dialog's keys for the model
   (`START_SERVER`, XSERV on the 49G). Cost: small (`pane.js`,
   `xmodem.js` `RESTART`, tests). Screenshots
   `16-fast-get-49g-cable-done-restart-hint.png`,
   `17-xserv-restore-restarted-card.png`.
3. **One way into the serve screen, asked once.** Now: "Add a
   calculator" → "An HP 38G, 39G or 40G" opens the serve screen, which
   asks again for "A calculator on a cable" or an emulated one; each
   pane's Change menu also has "Serve a folder to a 38G, 39G or 40G…",
   which is no place for that pane but a screen over both. Proposed:
   drop the Change entry; from "Add a calculator" the cable is chosen
   already; an emulated 38G, 39G or 40G is offered in "Add an emulated
   calculator" and opens the serve screen with it chosen; the empty
   state's button stays where no serial port is reached. Cost: small
   (`pane.js`, `app.js`, `serve.js`, page tests). Screenshots
   `22-change-menu-two-ways-to-serve.png`, `20-serve-screen-from-add.png`,
   `01-add-a-calculator-kind.png`.
4. **One connect step instead of a model picker.** Now: the dialog asks
   for the model, which only picks the text (Files reads the model from
   the calculator); the 49G's text is the longest (alpha, speed, ROM
   1.10, "Files works with either", two display texts). Proposed: one
   short table, 48S/SX `SERVER`, 48G series right-shift →, 49G `XSERV`
   (or `SERVER`), with the keys drawn, then "Choose the port". Cost:
   small (`app.js`, `index.html`, connect page tests, docs). Screenshots
   `02-connect-48gx.png`, `03-connect-49g.png`, `04-connect-48sx.png`.
5. **Same names for the same actions.** Now: the empty pane says "Add a
   calculator" and "Add an emulated calculator", the Change menu "A
   calculator on a cable…" and "An emulated calculator…", and the dialog
   asks "Which calculator is on the cable?" though its 38G path also
   serves emulated ones. Proposed: the card's labels everywhere, the
   question "Which calculator?". Cost: trivial (labels, test selectors).
   Screenshots `00-empty-firefox.png`, `22-change-menu-two-ways-to-serve.png`.
6. **No Settings dialog.** Now: a Settings button in the top bar and
   "Connection settings…" in each calculator's "⋯" open one dialog with
   "Find the speed and parity automatically", a fixed speed and parity,
   and a "Connected now" line; the search finds every server, XSERV
   included. Proposed: "Use a fixed speed" inside the connect dialog's
   "Trouble with the cable?", the button and the menu item gone. Cost:
   small to medium (`app.js`, `index.html`, `actions.js`, tests, docs).
   Screenshot `12-settings-xserv-says-kermit.png` (before this part's
   fix of its note).
7. **A calculator that stops answering says so.** Now: after ON on the
   calculator, a refresh waits 60 s (XSERV) or about 150 s (Kermit), the
   list says "This directory could not be read: unexpected reply: XSERV
   L: no answer within 60 s; …" or "… kermit: timed out: too many
   retries", and the status keeps "Connected at 9600 baud.". Proposed:
   a "The calculator stopped answering" card as for a left server, with
   Reconnect and Disconnect and the display text to look for; the
   protocol words mapped in `failureText`; satx-core's XSERV no-answer
   error no longer an "unexpected reply" (also seen by satx). Cost:
   medium (an error kind from saturnus-tx, the card, satx-core's error,
   tests). Screenshots `18-xserv-calculator-stopped-answering.png`,
   `19-kermit-48gx-stopped-answering.png`.
8. **The XSERV hint once, not as the status.** Now: every idle status of
   a 49G on its Kermit server is two lines, "Connected at 9600 baud.
   XSERV copies faster: disconnect, type XSERV on the 49G and connect
   again." Proposed: "Connected at 9600 baud." with a note under it that
   can be closed, or a "Switch to XSERV…" item that disconnects, shows
   the keys and connects again. Cost: small. Screenshot
   `14-49g-kermit-xserv-hint.png`.
9. **Firefox.** Firefox 159 has `navigator.serial`, so Files offers "Add
   a calculator" there (and "Files in this browser", no folder picker),
   while `files.md` and `web/README.md` say Firefox has no Web Serial.
   Not tried on a cable. Proposed: test Firefox with the owner's cable,
   then say so in the docs, or keep calling it untested. Screenshot
   `00-empty-firefox.png`.
