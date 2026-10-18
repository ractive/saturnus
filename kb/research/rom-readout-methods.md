---
title: "Reading the ROM out of your own calculator: 42S, 27S, 17B, 17BII, 28S, 28C, 19BII"
type: research
status: completed
date: 2026-10-19
tags:
  - rom
  - pioneer
  - clamshell
---
# How owners dump the ROMs of HP Pioneer and clamshell calculators (research, 2026-10-18)

Scope: HP 42S, 27S, 17B, 17BII, 28S/28C, 19BII/19B. The research covered web pages and forum
prose only. No ROM files, ZIPs or ImageArt PNGs were downloaded. No emulator source was read.

Access notes: hpmuseum.org (the forum and the old `archvNNN.cgi` archive) sits behind a
Cloudflare challenge, and archived.hpcalc.org returned 503. Forum threads were therefore read
through Wayback Machine snapshots of the same URLs, and the citations below give the original
URLs. Giesselink's detailed procedures (`PIONEER.TXT`, `CLAMSHEL.TXT`) are inside
`CPROMUPL.ZIP`. That ZIP was **not** opened because of the no-ZIP rule, so where a step depends
on those files the report says so.

## Shared background: the two IR methods (High-End Pioneers 17B/17BII/27S/42S)

All four High-End Pioneers have an IR LED for the HP 82240 printer, using the "RedEye" protocol.
They also have a built-in **memory scanner/browser** in the hidden test mode. Two methods build on that:

**Method A: manual scanner dump (J-F Garnier, Apr 2002).**
1. Enter the test mode: hold ON/EXIT and press LOG (Garnier writes "EXIT-LOG"; Finseth and Vieira write [ON][LOG]).
2. Press the back-arrow key to open the memory browser. The '.' key shows the ROM revision (A/B/C) at the top left.
3. Use the arrow and / * + - keys to move the cursor to address 0.
4. Align the IR LEDs, start HP's `INPRT` on an HP 48, then hold COS on the Pioneer. Each COS sends the 8 bytes shown on the display to the printer port.
5. Release COS at 1000h, wait for INPRT's ~1 s timeout, store the string and move it to a PC.
6. Repeat for each 1000h-nibble block up to 1FFFFh, which is 32 blocks for 64 KB.
7. Leave the test mode with EXIT + √x.

- Hardware: an HP 48 (SX/GX) with `INPRT`, an HP 48–PC cable, and a PC. The case is not opened.
- Effort: very slow. Giesselink says "~4 hours"; Sloyer says "several hours and a sore finger".
- Status: known to work. Garnier did the 42S, and users did 42S rev C and 17B/17BII (keys mapped by position).
- Output: the text stream copies the LCD, with an address prefix on every line. The prefixes must be stripped and the result converted on the PC.

**Method B: Giesselink's ROM upload packet (CPROMUPL, 2003 onward).**
1. Use the same memory scanner to type a ~50-byte machine-code program into RAM (users mention address #52000). This takes 5–15 min and may destroy RAM contents.
2. The program calls the ROM's own IR-transmit routine. Its entry address depends on the ROM revision, so one line must be edited per revision.
   - Known revisions in 2003: 17B rev B, 17BII rev B, 27S rev B, 42S rev A/B/C.
   - A "short description" explains how to find the address on an unknown revision.
3. Start the program with '.'. It streams the whole ROM, framed in blocks with SOH and CRC, to an HP 48 running a binary-capable receiver (`BINPRT`). `INPRT` cannot take NUL bytes.
4. Transfer the captured string to a PC and decode it with `CAPC2BIN`. Check the image with `LEWISCRC`.

- Hardware: an HP 48 with 70 KB (64 KB ROM) or 103 KB (96 KB ROM) free. That means a 48G+/GX or a 48 with a merged 128 KB card. A cable and a PC are also needed.
- Effort: transfer about 16–17 min per 64 KB. The whole job takes about 40–45 min, most of it automated.
- Pitfalls:
  - Stray IR from 50/60 Hz energy-saving lamps corrupts the stream. Work in the dark or cover both calculators with paper.
  - LEDs at the same height, about 5 mm apart.
  - In 2013 a user failed on a 17BII about 12 times because of CFL lamps.
- Status: known to work (Giesselink; Harald, 42S, first try; jebem, 27S with a 48G+).
- Receivers without an HP 48: Giesselink also tried a 2400-baud serial variant aimed at Pocket PCs. It got the 42S under 6 min but was unreliable with most IrDA receivers, and he dropped it. Its "proof of concept" code was posted as hex.
- The HP 49g+/50g cannot receive: it is IrDA-only, not RedEye.
- In 2018 a user proposed an Arduino RedEye-to-USB receiver. No result was posted (WEAK, plan only).
- Giesselink notes that dedicated TSOP-chip RedEye receivers work over larger distances. No documented ROM dump with one was found.

Sources:
- J-F Garnier, "HP-42S: New Facts", MoHPC article 266 (2002): <https://www.hpmuseum.org/cgi-sys/cgiwrap/hpmuseum/articles.cgi?read=266>
- Giesselink, "Pioneer (17B,17BII,27S,42S) ROM image download?" (2003-05-22): <https://www.hpmuseum.org/cgi-bin/archv013.cgi?read=34572> (mirror: <https://archived.hpcalc.org/museumforum/thread-34572.html>)
- Giesselink, "Some thoughts about possibilities uploading High-End Pioneer and Clamshell ROM images", MoHPC article 582 (2006-02-20): <https://www.hpmuseum.org/cgi-bin/articles.cgi?read=582>
- "Trouble dumping Pioneer ROM via IR" (2013, Giesselink's answer about lamps): <https://www.hpmuseum.org/cgi-bin/archv021.cgi?read=245276>
- "HP42S ROM Dump..." (2006): <https://archived.hpcalc.org/museumforum/thread-87367.html>
- "HP-42S Emulator??" (2003, Vieira's step list, Giesselink: "that's the current situation"): <https://archived.hpcalc.org/museumforum/thread-39261.html>
- jebem, "HP Pioneers ROM Dumping (I tested for HP-27S)" (2017, PDF compiling the Giesselink/Garnier/Del Tondo methods; PDF not read): <https://www.hpmuseum.org/forum/thread-7848.html>
- Emu42 page: <https://hp.giesselink.com/emu42.htm> (ROM upload packet: <https://hp.giesselink.com/Emu28/CPROMUPL.ZIP>, not opened)

---

## HP 42S

- **Method A (manual scanner + INPRT):** works. Hardware: HP 48 + cable + PC. Takes hours. Primary source: Garnier art. 266.
- **Method B (CPROMUPL machine code + BINPRT):** works. Hardware: HP 48G+/GX or 48 with a 128 KB card, plus cable and PC. About 45 min. Revisions A, B and C are supported. Several users confirm it.
- **Serial 2400-baud variant (art. 582):** experimental and unreliable. Giesselink gives no support for it.
- ROM is 64 KB (address range 0–1FFFFh in nibbles).
- **Verdict: practical.** An HP 48 is the realistic receiver.

## HP 27S

- **Method B:** works. jebem dumped a 27S with a 48G+ in 2014 and wrote it up in 2017. Giesselink knew the 27S rev B entry in 2003.
- In 2005 one user's 27S froze with the program on revs A and B. The thread has no resolution, so rev A may need its own entry address (unclear).
- **Method A** should also work, since the 27S has the same scanner.
- Sources: thread 7848 (above); "HP-27s Rom Dump problem" (2005): <https://www.hpmuseum.org/cgi-bin/archv015.cgi?read=75242> (mirror archived.hpcalc.org thread-75242); "HP 28s rom dump with 50g" post #5 (jebem): <https://www.hpmuseum.org/forum/post-31423.html>
- **Verdict: practical.**

## HP 17BII

- **Method A:** works. Randy Sloyer (2003): "Process is identical to that of the 42S … map the 42S keys … to the same physical 17B/17Bii keys." Source: <https://www.hpmuseum.org/cgi-bin/archv013.cgi?read=47844>
- **Method B:** works. The 17BII rev B entry has been known since 2003. Giesselink later dumped a 64 KB non-international 17BII himself (2007 forum post, reported in search results; post not read directly, WEAK). Neil Hamilton's 2013 failures were caused by lamp interference.
- The international 17BII carries a second, optional ROM (Garnier). Giesselink says such images are 96 KB and need about 103 KB free on the HP 48.
- **Verdict: practical.**

## HP 17B (original)

- Giesselink listed the "HP17B rev. B" IR entry in 2003, so **Method B** covers it. Vieira says all printing Pioneers (17B/BII, 27S, 42S) have the scanner, so **Method A** applies too.
- No forum report of an actual 17B dump was found. The 17B has no first-hand success report (WEAK on confirmation).
- Emu42 ships a 17B KML skin (SKN17B), so a 17B ROM evidently exists on the author's side.
- **Verdict: practical in principle.** It is the same hardware family as the confirmed models.

## HP 28S

- **Method: Giesselink's clamshell recipe `CLAMSHEL.TXT` in CPROMUPL.ZIP (not read).** The 28S runs a machine-code uploader and streams the ROM over IR to an HP 48.
  - Article 582 shows the 28S variant is a hex string turned into a code object on the calculator. Exactly how the code object is built (SYSEVAL helpers) is in `CLAMSHEL.TXT` and is unclear here.
  - Hardware: HP 48 + cable + PC.
  - Giesselink (2015): the old requirement for a merged 128 KB card "is obsolete". The Feb 2007 `CLAMSHEL.TXT` has a section "2.2) ROM upload in parts" for a stock 48GX/G.
  - ROM is 128 KB (Cyrille de Brébisson, from memory).
  - Status: works. Jeroen Van Nieuwenhove dumped a 28S "flawlessly" with a 32 KB 48G, unloading segments to the PC (2013).
- The HP 50g cannot receive (IR incompatible).
- An experimental 2400-baud serial 28S uploader also exists (art. 582, proof of concept).
- Emu42 page: "how to extract the ROM images from the High End Pioneers and the HP28S read the documentation of the ROM upload packet".
- Sources: <https://www.hpmuseum.org/forum/post-31423.html> (thread "HP 28s rom dump with 50g", 2015–16), thread 245276 post #3, article 582, emu42.htm.
- **Verdict: practical.**

## HP 28C

- **Method 1: `ROMDMP.TXT` (Giesselink, 2002-11-11), "for experts only".**
  1. In HEX mode, type helper programs: a checksum checker, a POKE-like loader and a PEEK generator. A typo can cause Memory Lost.
  2. Build a PEEK routine that returns 16 nibbles per address.
  3. Print its output over IR with PR1.
  4. Capture it with `INPRT` on an HP 48 in sessions of about 2 KB, which is slow.
  5. Merge on the PC and check with `CHAMPCHK`.
  - ROM is 128 KB.
  - Sources: <https://hp.giesselink.com/Emu28/ROMDMP.TXT> and <https://www.hpcalc.org/hp28/programming/romdmp.txt>
- **Method 2: CPROMUPL**, which the Emu28 page describes as "uploading a HP28C ROM image very quickly". Needs "A HP28C and a HP48 as receiver."
  - Source: <https://hp.giesselink.com/emu28.htm>
- The Emu28 page also offers an entry-point list for 28C rev 1CC.
- **Verdict: practical.** Needs an HP 48.

## HP 19BII (and 19B)

- **No owner-level method is documented.** Our current statement ("cannot be read out") is **still accurate**. No newer method was found up to 2026.
- The 19B/19BII has **no memory scanner/test-mode browser**. Mark Hardman: "there is no way to get into the ROM scanner on the HP-19B/BII". Eric Smith (brouhaha): "Because there isn't one."
- Without a scanner there is no known way to enter machine code, so neither IR method applies.
- Christoph Giesselink, 2014-11-20, thread 2447:
  - "Some of the ROM images haven't been extracted. About 10 years ago some at HP had been interested into an emulator … To make them I got access to some images."
  - "I only know a theoretical method": the Bert/Lewis/Sacajawea CPUs have the Saturn bus only inside the chip. HP technical documents mention a "debug mode" set by pins at CPU startup that would expose bus data during the ROM self-test, "but these technical documents tell nothing about the pin layout, so it's a theoretical method only."
  - In other words, the 19BII image in Emu42 came from HP, not from a dump.
- Eric Smith, 2014: he had ideas to dump the 18C/19B/19BII that destroy the calculator (chip decapping with fuming nitric acid). His lab was not yet complete. A 2016 follow-up ("Did something happen?") got no answer, and no published result was found (WEAK/unconfirmed).
- Original **19B**: Emu42 has no 19B skin (only SKN19B2) and the ImageArt page lists no 19B. There is no method and no known image.
- Source: <https://www.hpmuseum.org/forum/thread-2447.html> (read via Wayback snapshot).
- **Verdict: not owner-dumpable** with any documented, non-destructive method.
  - The theoretical routes are a CPU debug-mode bus capture (pinout undocumented) or decapping and optical ROM extraction (destructive, specialist equipment).
  - Garnier's Saturn-bus approach for the 22S/32S/32SII ("external hardware and knowledge", per the Emu42 page) is the closest precedent. No write-up of it was found in this pass.

---

## What the Emu42 documentation says about ROM images

- **Emu42 page** (<https://hp.giesselink.com/emu42.htm>):
  > "Like all emulators Emu42 needs a ROM image for each emulated calculator to work. I have no license to publish them, so they aren't included in the emulator package. Sorry I will not answer any ROM image related questions. For further information how to extract the ROM images from the High End Pioneers and the HP28S read the documentation of the ROM upload packet [Emu28/CPROMUPL.ZIP] and the Emu42 manual please. For all other calculators you need external hardware and knowledge to extract the data from the Saturn bus system, a way which was gone by Jean-François Garnier to extract the HP22S, the HP32S and the HP32SII rev. B ROM image."
  - The page also lists the tools `LEWISCRC` (CRC check), `MKE42`, `MAC2TXT`, `MK17MAC` and `SUPROM28`. None of their descriptions were opened.
- **Emu42 manual** (<https://hp.giesselink.com/Emu42/Emu42.htm>), sections 3–4:
  - "The necessary ROM images are copyrighted by Hewlett Packard and I have no license to distribute them. Please don't ask me, I will not send you mine."
  - Images may be packed or unpacked and can be checked with `LEWISCRC.EXE`.
  - The image name must match the KML script.
  - The manual has **no extraction instructions** and points to nothing beyond that.
- **Emu28** (HP 28C):
  - Same "no license" wording. It offers `ROMDMP.TXT` ("A theoretical discussion about the abilities how to upload the necessary HP28C ROM image") and `CPROMUPL` ("Necessary hardware: A HP28C and a HP48 as receiver").
  - Sources: <https://hp.giesselink.com/emu28.htm> and the Emu28 manual <https://hp.giesselink.com/Emu28/Emu28.htm> (validation with CHAMPCHK).

## Links to the ImageArt page

- **hp.giesselink.com links to it from exactly one place:** the home page (<https://hp.giesselink.com/index.htm>), under "Links".
  - The link text is **"Pictures at an exhibition of essentials"**, with no description around it.
  - It sits between "Emu48+ Beta Source code"/"Nonpareil" and "Free42".
- No other Giesselink page checked links to it: emu42.htm, Emu42 manual, emu28.htm, Emu28 manual, emu48.htm, emu48faq.htm, pcuti.htm, 25ann48.htm, hp82240b.htm, emu71.htm, v41.htm, hpil.htm, AllschwilMeeting.htm.
- **The ImageArt page itself:**
  - Title "Calculator Image Art", heading "Pictures at an exhibition of essentials", footer "Christoph Gießelink, December 2021".
  - Groups: Low End Pioneer (10B, 20S, 21S), Mid Range Pioneer (14B, 22S, 32S, 32SII), High End Pioneer (17B, 17BII, 27S, 42S), Clamshell (19BII, 28C, 28S). Each entry has a codename and a PNG.
  - The page says **nothing** about ROMs.
- **Community reading:** in MoHPC thread 11462 (Sept 2018), Eric Rechlin (hpcalc.org) answered a user who lacked an HP 48 for a 42S/27S dump with: "The images in this page might be of interest to you. Don't ask questions; just think and experiment... <https://hp.giesselink.com/ImageArt/ImageArt.htm>".
  - Replies: "Finally I've them all." (M. Gnerucci); "Alas, no 32S there" (2018; the page now lists a 32S); Olivier De Smet: "thinking is done … now time to code new emulators".
  - So the forum treats the PNGs as carrying the ROM images, but nobody states it outright. I did not verify it and did not download the images, as instructed.
  - Source: <https://www.hpmuseum.org/forum/thread-11462.html> (pages 1–2).
- **Consequence:** the 19BII, 28C and 28S appear there. If the community reading is right, those images are a copy of Giesselink's HP-sourced set. That is not an owner dump, so it does not change the finding that a 19BII cannot be dumped by its owner.

## Summary table

| Model | Practical owner method | Hardware | Effort | Confirmed |
|---|---|---|---|---|
| 42S | Test-mode memory scanner → IR (manual INPRT, or CPROMUPL machine code + BINPRT) | HP 48 (G+/GX or 128 KB merged for CPROMUPL), cable, PC | ~45 min (CPROMUPL) / ~4 h (manual) | Yes, many users |
| 27S | Same as 42S | Same | Same | Yes (jebem 2014/2017) |
| 17BII | Same as 42S | Same; 103 KB free for the 96 KB international ROM | Same | Yes (Giesselink, Sloyer) |
| 17B | Same (rev B entry known) | Same | Same | Not directly reported (WEAK) |
| 28S | CPROMUPL `CLAMSHEL.TXT` machine-code uploader → IR | HP 48 (stock 48G/GX works with upload in parts), cable, PC | Not stated (likely under 1 h) | Yes (Van Nieuwenhove) |
| 28C | ROMDMP.TXT (PEEK + PR1 + INPRT, slow) or CPROMUPL | HP 48, cable, PC | Hours (ROMDMP) / fast (CPROMUPL) | Tools published by Giesselink; no user report found |
| 19BII / 19B | **None.** No memory scanner; only theoretical CPU debug-mode bus capture or destructive decap | — | — | No; the Emu42 19BII image came from HP |
