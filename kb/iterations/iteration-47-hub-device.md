---
title: "Iteration 47: saturnus tx reaches the emulator's calculator through the hub's memory commands (HubDevice), and a typed command policy"
type: iteration
date: 2026-10-14
status: completed
branch: iter-47/hub-device
tags:
  - iteration
  - saturnus-tx
  - web
  - protocol
---

# Iteration 47: HubDevice, the command policy, and the apps' names

Owner decision (2026-10-14): the emulator and saturnus tx stay two
separate apps; tx is mainly for real calculators over a cable. A later
merge must stay possible, so nothing new is built on the tab messages of
`web/shared/site-links.js` or on the hub's serial-line ownership.

Today tx reaches the emulator's own (shared, kept-ROM) calculator the
way it reaches a cable: it types SERVER into the shared machine, holds
its serial line in the hub (`web/hub.js`: `holder`, `LINE_TAKEN`,
`keysElsewhere`, `lineMoved`/`SERIAL_IDLE_MS` speed juggling) and runs
the Kermit client of `saturnus-tx-web` over `EmulatorLink`. Every
problem of iterations 41 and 46 (ATTN in the emulator ending tx's
server, the emulator's writes refused while tx holds the line, Max
speed while bytes move, ON-tapping a calculator left off) comes from
that. The engine already has the memory commands the emulator's own
explorer uses ([[docs/saturnus-tx]]; `web/protocol.md`, "The user
memory, read-only" and "written"): `watchMemory`/`memoryChanged`,
`memoryTree`, `fetchFile`, `storeFile`, `rename`, `purge`, `createDir`,
`copy`, `move`, `changeDir`, `setFlag`, `flags`. They run as hidden
transactions inside the engine, in emulated time, ordered with keys and
the emulator page's own writes, with no line held.

## Design

### HubDevice

- **`web/shared/hub-device.js`**, class `HubDevice extends EventTarget`,
  implements the device interface of `web/shared/device.js` (`home`,
  `list(path)`, `read`, `write`, `rename`, `purge`, `mkdir`, the `change`
  event) and the parts of `KermitDevice`'s surface that `web/tx/pane.js`
  and `web/tx/transfers.js` call (`getTree`, `putTree`, `busy` and
  `progress` events, `cancelQueued`, `refresh`, `connection`, `close`).
  It lives in `web/shared/`, not `web/tx/`, and takes any object with
  `request(cmd, args)` and protocol events, so the emulator page could use
  it in a merged app.
- **Connection**: the hub's existing `join` (`web/shared/calc-hub.js`,
  `joinHub`), `hello`, `attach` (unchanged: the kept ROM's bytes) and
  `leave`. No `serialOpen`, no `run "SERVER"`, no `probeServer`, no
  `setSpeed`. The machine runs at the emulator page's speed; the
  transactions run as fast as the host can at any speed (protocol.md).
- **Mapping** (verified against `crates/saturnus-host/src/protocol/mod.rs`
  `handle` and `write_op`):

  | Device call | Hub commands |
  | --- | --- |
  | `list(path)` | `memoryTree` once per `memoryChanged`, the directory picked out of the tree; entries `{name, kind: type, size, checksum, directory: !!variables}` |
  | `read(path, name)` binary | `fetchFile {dir, name}` → `data` |
  | `read(path, name, {text})` | gap 2 below |
  | `write(path, name, bytes, {replace})` | `storeFile {dir, name, data}`; `replace` gap 3 |
  | `mkdir` | `createDir` |
  | `rename(path, from, to)` | `rename {dir, name: from, to}`; "to exists" mapped to the pane's `nameClash` |
  | `purge(path, name, {count})` | the `notEmpty` count from the tree, then `purge` (the engine's purge is `PGDIR` for a directory) |
  | `getTree` / `putTree` | composed in HubDevice from `memoryTree`, `fetchFile`, `createDir`, `storeFile` (one command per variable, the `item` progress as KermitDevice's) |
  | `change` event | `watchMemory {on: true}`; every `memoryChanged` drops the cached tree and fires `change` with no path |

  A change typed on the calculator in the emulator now shows in tx at
  once (KermitDevice could not see it while its server was busy serving).
- **Refusals** come from the engine as they do for the emulator's
  explorer: a command line open ("finish or cancel it first"), flag -33
  on a 48, typing in progress, a 512 KiB cap on `storeFile`. HubDevice
  turns them into the pane's toasts in tx's words, e.g. "The calculator
  is busy in the emulator: finish or cancel the command line there."

### Feature gaps of the memory commands, decided

| Gap | Decision | What the user sees |
| --- | --- | --- |
| 1. Free memory (`list`'s `free`, `memory()`) | Not added: `memoryTree` has no free figure, and a `MEM` transaction per listing is not worth it | No free-memory figure on the emulator's calculator; the backup check runs inside the backup command |
| 2. Get as text (the calculator's `%%HP:` text form) | **Add** `text: true` to `fetchFile`: the engine's Kermit get in ASCII mode (flag -35 cleared and put back, as `storeFile` does for text) | "Get as text" works as on a cable |
| 3. Put with `replace` (KermitDevice refuses a name in use unless asked) | **Add** `replace` (optional, false) to `storeFile`: refused when the name exists unless `replace`, then the old variable purged in the same transaction (today flag -36 decides) | The same "Replace X?" question as on a cable |
| 4. Backup (`ARCHIVE` to `:0:`, get, purge) | **Add** `backup` to the engine: one hidden transaction with the steps of `crates/saturnus-tx/src/ops.rs` `backup`, refused with the needed and free bytes when it does not fit | "Back up" works; the file is the same `.hp` a cable gives |
| 5. Restore | **Never** on the emulator's calculator (owner, 2026-10-14): the emulator's Load state covers it; stays for cable and private calculators | "Restore" is not offered on the emulator's calculator; a line says to use the emulator's Load state |
| 6. Graphics screen (PICT) | **Never** on the emulator's calculator (owner, 2026-10-14): the "+" above the emulator's display (screen image, Store screen) covers it; stays for cable and private calculators | The action is not offered on the emulator's calculator |
| 7. Fast transfer (XModem) | Not needed: a hidden transaction takes 0.15-0.25 s of wall time in the browser | The fast-transfer action is not offered; nothing to wait for |
| 8. Flags | No gap: `flags` and `setFlag` exist; tx has no flags view to feed | Nothing |

### What becomes dead, and what stays

- **Removed**: in `web/hub.js` `keysElsewhere`, `LINE_TAKEN`,
  `LINE_ELSEWHERE`, `PORT_HELD` rewriting, `lineMoved`, `lineBusy`,
  `lineTimer`, `SERIAL_IDLE_MS`, the holder clause of `speedOf`, the
  holder's `serialClose` + `endServer` in `detach`. Serial commands pass
  through only for a page alone on its engine (its home, a plain Worker);
  from a page that shares its machine `serialOpen` and `serialWrite` are
  refused (owner, 2026-10-14) with "This calculator is shared with
  another page, so its serial port cannot be opened here. Use a
  calculator of its own (a ROM file chosen in saturnus tx)."
  (`serialClose` is answered and does nothing). No serial console in the
  emulator is planned; this is reopened if one is. In `web/tx/emulator.js`
  the `shared` branch, `IN_USE`, `startServer({gentle})`, `blank()`'s
  ON tap; in `web/tx/kermit-device.js` `recover`, `suspectServer`,
  `suspect`/`suspicions`, `restartServer`, `watched`, `SILENT_MS`,
  `WRITE_STOPPED` (a private calculator has no other hands; check
  `web/test/tx-page.test.mjs` and `tx-e2e.test.mjs` before deleting).
  `pane.js`: `place.sharedEmu`, the `keysElsewhere` listener.
- **Stays**: the private calculator (`web/tx/emulator-worker.js`, a ROM
  file chosen in tx; aplet serving in `web/tx/serve.js`) goes on typing
  SERVER and runs the real Kermit and XModem code over `EmulatorLink`,
  with its own `setSpeed`/`atSpeed` and the e2e suites
  (`tx-e2e.test.mjs`, `tx-e2e-xmodem.test.mjs`, `tx-serve-e2e.test.mjs`):
  the honest test of the cable path. The hub's `endServer` of a restored
  kept state stays (old states saved in SERVER).

### The emulator's chip and drop

- **Chip**: the hub sends a new event `peers` (`{tx: true|false}`) to the
  emulator pages of a machine when a tx page attaches or leaves. The
  emulator shows "Connected to saturnus tx · Show" beside the status.
  Show is a plain link to tx with `target="saturnus-tx"`
  (`TAB_NAMES.transfer`), which brings an existing named tab forward
  without a message. No Disconnect button and no hub command for it
  (owner, 2026-10-14): tx disconnects in tx.
- **Drop on the drawn calculator**: a file dropped on `<sat-calculator>`
  (`web/components/sat-calculator.js`) with an HP transfer header
  (`HPHP48-`, `HPHP49-`, `%%HP:`) is a calculator file; any other file
  goes to `chooseFiles` (`web/components/sat-controls.js`) as today, and
  what its result did not take as a ROM, at most 512 KiB (the
  `storeFile` cap), is a calculator file too. Calculator files are
  stored in the calculator's current directory (`memoryTree`'s `path`)
  through the existing `MemoryWrites.storeFiles(dir, files)`
  (`web/writes.js`), with its toast ("X.hp stored as X in HOME").
  `dropRefusal` applies (a model without a server, no calculator
  running). `dropNotice` in `web/norom.js` points at the calculator, not
  only the memory view. A drop elsewhere on the page stays as it is.

### Command policy (review finding B)

- `crates/saturnus-host/src/protocol/command.rs`: `pub enum CommandKind`
  (one variant per command of web/protocol.md, `FromStr`/`name()` with
  the JSON names unchanged) and `pub struct Policy { refused_while_typing,
  write, server, serial, http: Option<Endpoint> }` from one exhaustive
  `const fn policy(self) -> Policy` match: a new command does not compile
  until its policy is written.
- Replaces `REFUSED_WHILE_TYPING`, `WRITE_COMMANDS`, `SERVER_COMMANDS`,
  `SERIAL_COMMANDS` (`protocol/mod.rs`; `Engine::refusal`, `admit`, the
  `c if WRITE_COMMANDS.contains(&c)` arm of `handle`), the re-export in
  `crates/saturnus-drive/src/runner.rs` (`WRITE_COMMANDS`, also
  `NATIVE_COMMANDS`) and `MEMORY_COMMANDS` and `Endpoint::commands` in
  `crates/saturnus-cli/src/control/server.rs`. `handle` dispatches on
  `CommandKind` (an unknown name stays "unknown command"). The loops in
  `protocol/tests.rs` iterate `CommandKind::ALL` filtered by policy.
- No JSON change: `web/test/protocol-script.expected.json` stays
  byte-identical without `SATURNUS_BLESS`.

### Names

- Navigation: the emulator's link to tx (`#to-transfer` in
  `web/index.html`, today "Transfer files: saturnus tx") reads **Files**;
  tx's link to the emulator (`#to-emulator` in `web/tx.html`, today
  "Emulator") reads **Calculator**. Titles and product names stay
  ("saturnus", "saturnus tx").
- Landing (`web/landing/index.html`): `<h1>` "saturnus: classic HP
  calculators in your browser" (today "The HP 48, in your browser."),
  `<title>` to match. "HP" only as words in running text; no logo or
  wordmark ([[docs/clean-room-rule]]).

## Tasks

- [x] `crates/saturnus-host`: `CommandKind` and `Policy` in `protocol/command.rs`; `refusal`, `admit` and `handle` on it; the four arrays removed; tests iterate `CommandKind::ALL`
- [x] `saturnus-drive` runner and `saturnus-cli` control server take the HTTP endpoint and native sets from `Policy`; `MEMORY_COMMANDS` gone; `server.rs`'s test `memory_takes_every_write_and_the_text_read` rewritten on the policy
- [x] Engine: `fetchFile` with `text: true` (ASCII mode), `storeFile` with `replace`, `backup` (one transaction: ARCHIVE `:0:`, get, purge; refused when it does not fit), each in `transfer.rs` `Op`, with unit tests in `crates/saturnus-host/tests/transfer.rs` (ROM-gated) and the protocol tables in `web/protocol.md`
- [x] Hub: `peers` event; `serialOpen`/`serialWrite` refused on a shared machine with the message above; the dead code listed above removed; `web/test/hub.test.mjs` rewritten (the LINE_TAKEN and keysElsewhere tests go; peers and the serial refusal come)
- [x] `web/shared/hub-device.js` (`HubDevice`) with unit tests on a fake hub (`web/test/hub-device.test.mjs`: list from the tree, change on `memoryChanged`, the mapping table, nameClash, notEmpty count, refusal texts, getTree/putTree progress)
- [x] tx: `addKept` (`web/tx/app.js`) and `connectFromEmulator` connect a HubDevice pane (no SERVER, no line); the pane hides Restore, Graphics screen and the fast transfer for it, with the lines of the gap table; `EmulatedCalculator` private only; `KermitDevice` recovery code removed
- [x] Emulator page: the chip (status area, `web/app.js` / `web/components/sat-controls.js`) with Show, no Disconnect; drop of calculator files on `<sat-calculator>` into the current directory with a toast
- [x] Names: nav labels Files / Calculator, landing `<h1>` and `<title>`; `web/test/site.test.mjs`, `nav-page.test.mjs` and the landing screenshots (`web/landing/shots.mjs`) updated
- [x] `web/test/tx-shared.test.mjs` rewritten: tx lists a variable typed in the emulator, copies both ways, an emulator ATTN or an open command line in the middle (a refusal toast, no stall, no SERVER on the emulator's screen), the chip shows while tx is connected and goes when tx disconnects, a `serialOpen` from either page on the shared machine is refused with the message; the private-calculator e2e (`just tx-e2e DIR`) unchanged and green
- [x] Docs: `web/protocol.md` (Shared calculators: `peers`, the serial refusal on a shared machine, `keysElsewhere` gone; the new fields; the policy note under Typing), [[docs/saturnus-tx]] (Devices: HubDevice beside KermitDevice), `web/README.md`, CHANGELOG (Unreleased), decision log (the gap decisions)
- [ ] Owner: on the 48GX and 49G, emulator and tx side by side: a drag each way, rename, purge, backup; typing in the emulator meanwhile; the chip's Show brings tx forward — superseded by the one app (iterations 50d and 51); the owner checks iteration 50d/51 instead
- [ ] Owner: the new landing headline and the nav labels read right on the site and on a phone

## Acceptance

- Connecting tx to the emulator's 48SX, 48GX or 49G types nothing on
  the calculator: its screen never shows SERVER, and `serialOpen` is
  never sent to a shared machine (asserted in `tx-shared.test.mjs` from
  the hub's traffic).
- A variable typed in the emulator appears in tx's list without a
  refresh; ATTN or an open command line in the emulator never leaves tx
  waiting (a refusal in words, at once).
- `rg "keysElsewhere|LINE_TAKEN|SERIAL_IDLE_MS|suspectServer" web crates`
  finds nothing.
- `REFUSED_WHILE_TYPING`, `WRITE_COMMANDS`, `SERVER_COMMANDS`,
  `SERIAL_COMMANDS` and `MEMORY_COMMANDS` are gone; adding a variant to
  `CommandKind` without a policy fails to compile; the protocol script's
  expected output is unchanged.
- The emulator shows "Connected to saturnus tx · Show" exactly while a
  tx page is attached to its machine; there is no Disconnect and no
  `dismiss` in `web/hub.js` or `web/protocol.md`.
- `serialOpen` on a machine two pages share is refused with the
  sentence of the design; a page alone on its engine still opens it.
- Restore and the graphics screen are not offered for the emulator's
  calculator, and still work on a private one (`tx-e2e`).
- A `.hp` file dropped on the drawn 48GX is stored in the current
  directory with a toast; a ROM dropped there is still taken as a ROM.
- The private calculator path and its e2e suites pass unchanged.

## Out of scope

- Merging the two apps; any new use of `openSiteTab`/`onSiteMessage`.
- Restore, PICT and XModem for the emulator's calculator (never, see the gap table).
- A Disconnect on the emulator's side; a serial console in the emulator.
- The desktop apps (separate processes; [[iterations/iteration-49-tx-desktop-roots]]).

## Settled by the owner (2026-10-14)

1. Restore and PICT on the emulator's calculator: never; the emulator's
   Load state and the "+" above its display cover them. They stay for
   cable and private calculators.
2. The chip is "Connected to saturnus tx · Show" only: no Disconnect, no
   `dismiss` command.
3. Serial commands on a shared machine are refused with a clear message;
   a serial console in the emulator is not planned (reopen then).
