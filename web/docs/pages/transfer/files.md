---
title: Files in the app
description: Files, the app's view for moving variables between a computer, an emulated calculator and a real one on a serial cable, in Chrome or Edge with Web Serial or in the desktop app; aplets for the 38G, 39G and 40G.
section: transfer
order: 4
---

Files is the app's second view, beside Calculator. It shows two places side by side, a calculator and a folder or two calculators, and copies variables between them by drag and drop. A place can be a calculator emulated in the page or a real one on a serial cable. It runs the same transfer code as [satx](satx.md), compiled to [WebAssembly](https://webassembly.org/).

[Open Files](../../app/#/files)

## What you need

| | |
|---|---|
| An emulated calculator | any browser |
| A real calculator | Chrome or Edge on a computer, or the desktop app |
| A folder on your disk | Chrome or Edge, or the desktop app |
| Models on a cable | 48S/SX, 48G/G+/GX and 49G as servers; 38G, 39G and 40G served a folder |
| Not supported | the 42S and the other Pioneer models, which have no serial port |

Chrome and Edge reach the serial port through [Web Serial](https://developer.mozilla.org/en-US/docs/Web/API/Web_Serial_API) and a folder through the [File System Access API](https://developer.chrome.com/docs/capabilities/web-apis/file-system-access). Firefox and Safari have neither: there Files keeps its files in the page ("Files in this browser"), which you add from your computer and download again, and works with emulated calculators only. A real calculator then needs the desktop app, which has native serial ports and folders.

## The two panes

The left pane always shows a calculator; the right pane shows a folder on your computer or a second calculator. On a narrow screen one pane shows at a time, with a switch above them.

Each pane has a tree of directories and a list of the variables in the current one, with their type, size and checksum as the calculator reports them. Selecting a variable shows a preview under the list: numbers, strings, lists and the other data objects as text, GROBs as pictures. Programs show as text when the calculator sends its text form.

To copy, drag rows to the other pane, or choose "Copy to the other pane" in a row's menu or press F5. F2 renames and Delete purges. Every copy is a job in the queue under the panes, with its progress in bytes; the jobs that wait can be cancelled.

Nothing is replaced without asking. Before a copy starts, one question lists what it would change: files with the same name, a binary file of the other family (a 48 file going to a 49G is stored as a string, so Files sends its text form instead), anything with no text form. A purge or a restore asks first as well.

## A real calculator on a cable

1. Plug in the [cable](cables.md) and its USB serial adapter.
2. In Files, choose "Add a calculator" and pick the model. The dialog shows the keys that start the Kermit server on that model: `SERVER` and ENTER on a 48S or 48SX, right-shift then → on the 48G series, right-shift and → quickly on the 49G.
3. Choose "Choose the port". The browser lists the serial ports; pick the adapter. A port the page was given before is offered again next time.

Files then finds the speed and parity by itself: it tries 9600, 4800, 2400 and 1200 baud, with no, odd and even parity, and listens for the server's answer. It reads the model from the calculator, switches it to binary transfers for exact copies and loads the HOME directory. When you disconnect, it ends the server and puts the calculator's own transfer setting back.

If the port does not open, the dialog's "Trouble with the cable?" lists the usual reasons. On Linux your user has to be in the `dialout` group (`uucp` on Arch). On Windows, current drivers refuse adapters with fake Prolific chips; one with an [FTDI](https://en.wikipedia.org/wiki/FTDI) or a genuine chip works. If the speed is not found, "Connection settings…" in the pane's "⋯" menu sets a fixed one.

What the calculator's "⋯" menu offers:

- **New directory…**, and Rename… and Purge… on a row.
- **Back up…**: all of HOME in one file, through the calculator's ARCHIVE. The dialog shows the free memory and refuses early when a copy of HOME does not fit.
- **Copy everything to a folder**: the whole HOME tree as folders and `.hp` files, one variable at a time. Slower, but it needs no free memory.
- **Restore from a backup file…**: puts a backup back. It replaces everything in HOME, and the calculator warm-starts; Files offers to reconnect once `SERVER` runs again.
- **Graphics screen**: PICT, the plots and drawings, as a picture.
- **Fast transfer (XModem)**, on the 48G series and the 49G: Files ends the server and tells you what to type on the calculator, `XSEND` or `XRECV` with the name.

A variable's own menu has Download and Download as text, for a file in your downloads instead of the other pane.

"Show the packets" opens a console with every byte that goes over the cable, for when something goes wrong.

## An emulated calculator

An emulated 48SX, 48GX or 49G in Files is the same calculator as in Calculator: what you type there shows in Files at once, and what Files copies there is in the calculator's memory at once. Its memory is read and written directly, without its serial port, so it needs no `SERVER` and nothing appears on its screen.

So a copy from the emulator to a real calculator is one drag: the emulated calculator in the left pane, the real one in the right. A file can also be dropped from your computer onto the drawn calculator in Calculator; it is stored in the current directory.

A ROM file chosen in Files instead starts a calculator of its own, which talks over its emulated serial port like a real one and keeps nothing when you leave.

## The 38G, 39G and 40G

These calculators start every transfer themselves, as clients, and expect a "disk drive" on the cable. Files can be that disk drive: choose "Serve a folder to a 38G, 39G or 40G", pick a folder and the calculator, on a cable or emulated, and "Start serving".

On the calculator, choose SEND or RECV in the Aplet Library (LIB on the 38G, APLET on the 39G and 40G), then "Disk drive…". Files lists the transfers as they happen. The aplets the calculator sends land in the folder under its own names, and any aplet of that family in the folder can be received, including ones from elsewhere. On the 39G and 40G, "Send by wire" also sends a binary aplet by XModem, to the calculator's "HP39/40 (Wire)". That way is tested on the emulated 39G only.

"Stop serving" ends it.

## Sources

- The app's Files view and its design notes in this repository: the panes, the queue, the dialogs and their labels, the speed search, the connection checks, the aplet server.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): the server's default speed and parity.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): ARCHIVE and RESTORE (27-12).
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf) and [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): sending and receiving aplets to a disk drive (1-26, 16-5).
- [MDN: Web Serial API](https://developer.mozilla.org/en-US/docs/Web/API/Web_Serial_API) and [File System API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API): which browsers have them.
