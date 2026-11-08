---
type: iteration
title: "Iteration 70: Files coherence pass after Kermit, XSERV and fast-transfer changes"
date: 2026-11-08
status: planned
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

- [ ] **Walk every flow** on the emulators (web page and desktop app where
  it differs), with screenshots of anything wrong: connect to a 48SX,
  48GX and 49G over Kermit; a 49G in XSERV with a list, both transfers,
  a change, a directory, PICT and a backup through the Kermit nesting;
  fast transfer over Kermit on a 48GX and a 49G; the restart hints;
  Disconnect; the emulated calculators; the 38G, 39G and 40G serve screen
  from "Add a calculator"; failures (a calculator that stops answering,
  the wrong server running, a busy port).
- [ ] **Words against behaviour:** every instruction Files gives (keys,
  `SERVER` or `XSERV`, restart hints, the settings line "Connected now:
  …"), the help and docs (`web/docs/pages/files.md`, `web/README.md`,
  the satx README, `kb/docs/saturnus-tx.md`): each says what the code does.
- [ ] **Simplifications:** list what the changes made redundant or
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
