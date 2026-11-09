---
title: Files in the app
description: Files, the app's view for moving variables between a computer, an emulated calculator and a real one on a serial cable, in a browser with Web Serial or in the desktop app; aplets for the 38G, 39G and 40G; files on an HP 100LX or 200LX palmtop.
section: transfer
order: 4
---

Files is the app's second view, beside Calculator. It shows two places side by side, a calculator and a folder or two calculators, and copies variables between them by drag and drop. A place can be a calculator emulated in the page or a real one on a serial cable. It runs the same transfer code as [satx](satx.md), compiled to [WebAssembly](https://webassembly.org/).

[Open Files](../../app/#/files)

## What you need

| | |
|---|---|
| An emulated calculator | any browser |
| A real calculator | Chrome, Edge or Firefox on a computer, or the desktop app |
| A folder on your disk | Chrome or Edge, or the desktop app |
| Models on a cable | 48S/SX, 48G/G+/GX and 49G as servers (the 49G's Kermit server or XSERV); 38G, 39G and 40G served a folder; the HP 100LX and 200LX palmtops through their Filer |
| Not supported | the 42S and the other Pioneer models, which have no serial port |

Chrome and Edge reach the serial port through [Web Serial](https://developer.mozilla.org/en-US/docs/Web/API/Web_Serial_API) and a folder through the [File System Access API](https://developer.chrome.com/docs/capabilities/web-apis/file-system-access). Firefox has Web Serial too (seen in version 159), so Files offers "Add a calculator…" there; it has not been tried with a real calculator yet. Firefox and Safari have no folder picker: there Files keeps its files in the page ("Files in this browser"), which you add from your computer and download again. Safari has no Web Serial either, so it works with emulated calculators only. A real calculator then needs the desktop app, which has native serial ports and folders.

## The two panes

The left pane always shows a calculator (or a palmtop); the right pane shows a folder on your computer or a second calculator. On a narrow screen one pane shows at a time, with a switch above them.

Each pane has a tree of directories and a list of the variables in the current one, with their type, size and checksum as the calculator reports them. Selecting a variable shows a preview under the list: numbers, strings, lists and the other data objects as text, GROBs as pictures. Programs show as text when the calculator sends its text form.

To copy, drag rows to the other pane, or choose "Copy to the other pane" in a row's menu or press F5. F2 renames and Delete purges. Every copy is a job in the queue under the panes, with its progress in bytes; the jobs that wait can be cancelled.

Nothing is replaced without asking. Before a copy starts, one question lists what it would change: files with the same name, a binary file of the other family (a 48 file going to a 49G is stored as a string, so Files sends its text form instead), anything with no text form. A purge or a restore asks first as well.

## A real calculator on a cable

1. Plug in the [cable](cables.md) and its USB serial adapter.
2. In Files, choose "Add a calculator…", then "An HP 48 or 49". The dialog shows, for each model, the keys that start its server: `SERVER` and ENTER on a 48S or 48SX, right-shift then → on the 48G series, `XSERV` and ENTER on the 49G (or `SERVER` and ENTER for its Kermit server). Start it on your calculator.
3. Choose "Choose the port". The browser lists the serial ports; pick the adapter. A port the page was given before is offered again next time.

Files then finds the speed and parity by itself: it tries 9600, 4800, 2400 and 1200 baud, with no, odd and even parity, and listens for the server's answer. It reads the model and the server from the calculator, switches it to binary transfers for exact copies and loads the HOME directory. When you disconnect, it ends the server and puts the calculator's own transfer setting back.

### XSERV on the 49G

XSERV, the 49G's XModem server, copies about twice as fast as its Kermit server: a 4 KB string took 8.4 seconds instead of 16.7 on a real 49G. Files finds it by itself and lists, copies, makes, renames and purges over it. Text files, files kept as strings, the graphics screen, backup and restore need the Kermit server: Files starts it inside XSERV for them and comes back to XSERV afterwards, which adds a few seconds. Disconnect ends XSERV. A variable's size is the same with either server. Connected to a 49G's Kermit server, Files says once, in a note under the pane's name, that XSERV copies about twice as fast. XSERV is built into the 49G from ROM 1.11 on; with an older ROM, type `SERVER`. An HP 48 running the XSrvr48 library answers as XSERV does, but Files uses XSERV on the 49G only and says so: start `SERVER` on the 48.

If the port does not open, the dialog's "Trouble with the cable?" lists the usual reasons. On Linux your user has to be in the `dialout` group (`uucp` on Arch). On Windows, current drivers refuse adapters with fake Prolific chips; one with an [FTDI](https://en.wikipedia.org/wiki/FTDI) or a genuine chip works. If the speed is not found, "Use a fixed speed" under "Trouble with the cable?" sets one, with the parity; Files keeps it for the next connect.

### When the calculator stops answering

If the calculator says nothing while Files waits for it, the pane says "The calculator stopped answering" instead of its list: after about 8 seconds on XSERV (longer while a file moves: up to about 25 seconds at 1200 baud), 20 to 25 on the Kermit server. Usually its server has ended (ON was pressed, or a key typed), or the cable came loose. The pane shows the keys that start the server again; then choose "Reconnect".

What the calculator's "⋯" menu offers:

- **New directory…**, and Rename… and Purge… on a row.
- **Back up…**: all of HOME in one file, through the calculator's ARCHIVE. The dialog shows the free memory and refuses early when a copy of HOME does not fit.
- **Copy everything to a folder**: the whole HOME tree as folders and `.hp` files, one variable at a time. Slower, but it needs no free memory.
- **Restore from a backup file…**: puts a backup back. It replaces everything in HOME, and the calculator warm-starts; the pane shows the keys that start its server again, then "Reconnect".
- **Graphics screen**: PICT, the plots and drawings, as a picture.
- **Fast transfer (XModem)**, on the 48G series, and on a 49G whose ROM has no XSERV (1.10 and earlier): Files puts the command (`« 'PRG' XSEND »`) on the calculator's stack and ends the server; you press EVAL on a 48G, right-shift then SYMB on the 49G. Afterwards start the server again (right-shift, then → on a 48G; `SERVER` on the 49G) and reconnect. A 49G with XSERV has no fast transfer in Files: XSERV copies as fast without these steps.

A variable's own menu has Download and Download as text, for a file in your downloads instead of the other pane.

"Show the packets" opens a console with every byte that goes over the cable, for when something goes wrong.

## An emulated calculator

An emulated 48SX, 48GX or 49G in Files is the same calculator as in Calculator: what you type there shows in Files at once, and what Files copies there is in the calculator's memory at once. Its memory is read and written directly, without its serial port, so it needs no `SERVER` and nothing appears on its screen.

So a copy from the emulator to a real calculator is one drag: the emulated calculator in the left pane, the real one in the right. A file can also be dropped from your computer onto the drawn calculator in Calculator; it is stored in the current directory.

A ROM file chosen in Files instead starts a calculator of its own, which talks over its emulated serial port like a real one and keeps nothing when you leave.

## The 38G, 39G and 40G

These calculators start every transfer themselves, as clients, and expect a "disk drive" on the cable. Files can be that disk drive: choose "Add a calculator…", then "An HP 38G, 39G or 40G", pick a folder, and "Start serving" (the calculator on the cable is chosen already). For an emulated one, choose its ROM in "Add an emulated calculator…" (a 38G, 39G or 40G ROM that Calculator keeps, or a ROM file): the serve screen opens with it. In a browser that cannot reach a serial port, the left pane and its Change menu also offer "Serve a folder to a 38G, 39G or 40G…".

Files reads the folder each time the calculator asks for its list of files, so an aplet put in the folder while serving shows up at the next RECV.

On the calculator, choose SEND or RECV in the Aplet Library (LIB on the 38G, APLET on the 39G and 40G), then "Disk drive…". Files lists the transfers as they happen. The aplets the calculator sends land in the folder under its own names, and any aplet of that family in the folder can be received, including ones from elsewhere. On the 39G and 40G, "Send by wire" also sends a binary aplet by XModem, to the calculator's "HP39/40 (Wire)". That way is tested on the emulated 39G only.

"Stop serving" ends it.

## An HP 100LX or 200LX palmtop

The [HP 100LX and 200LX](https://en.wikipedia.org/wiki/HP_200LX) are palmtop PCs, not calculators, but their built-in Filer can serve its disks over the serial cable, and Files shows them like a folder: the drives at the top (C: the internal disk, A: the memory card, D: the ROM disk, which is read-only), then directories and files with their size and date. Copy, rename, delete and make directories as in a folder, in either pane.

1. On the palmtop, open Filer, press MENU and choose Communications, Remote Settings. Set Baud to 19200, Interface to COM1 and Server Mode on, then OK. Leave Filer open and in front.
2. Plug in the palmtop's own serial cable and a USB serial adapter. HP's palmtop cable has the null modem built in, so the adapter goes straight into it. At the palmtop the plug goes in with the HP logo up.
3. In Files, choose "Add a calculator…", then "An HP 100LX or 200LX palmtop", then "Choose the port".

Never plug the palmtop's cable into an HP 48 or 49G, and never use a 38G cable or adapter on the palmtop: the plugs fit, but the wiring differs and can damage the device.

Files talks at 19200 baud, the speed the palmtop's serial chip handles safely according to the [palmtops FAQ](https://ftp.lip6.fr/pub/doc/faqs/hp/palmtops-faq). A faster one can be set under "Trouble with the cable?", "Use a fixed speed", with the same speed on the palmtop; above 19200 the chip may lose bytes, and a copy can fail part way. When nothing answers at another speed, Files says to go back to 19200.

The palmtop takes DOS names only: up to 8 letters or digits, a dot and up to 3 more, in capitals (`NOTES.TXT`). A file with a longer name is left out of a copy with the reason, so rename it first; a calculator's variable goes over as its `.hp` file, so `PRG` becomes `PRG.HP`. A copy moves about 1.7 KB a second at 19200 baud: the queue shows its progress, and Stop ends it at the palmtop's next reply (a file half written is deleted again). If the palmtop says nothing for about 18 seconds, the pane says "The palmtop stopped answering": check that Filer is still open in front with server mode on, that the palmtop has not turned itself off, and the cable, then choose "Reconnect". Disconnect leaves Filer in server mode for the next time.

This works against a test server here; on a real palmtop it is still to be tried out.

## Sources

- The app's Files view and its design notes in this repository: the panes, the queue, the dialogs and their labels, the speed search, the connection checks, the aplet server, the palmtop's Filer settings, drives, names and timing.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): the server's default speed and parity.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): ARCHIVE and RESTORE (27-12).
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf) and [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): sending and receiving aplets to a disk drive (1-26, 16-5).
- [comp.sys.palmtops HP100LX FAQ](https://ftp.lip6.fr/pub/doc/faqs/hp/palmtops-faq): the palmtop's serial chip and its safe speed.
- [MDN: Web Serial API](https://developer.mozilla.org/en-US/docs/Web/API/Web_Serial_API) and [File System API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API): which browsers have them.
