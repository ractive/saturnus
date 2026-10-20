# Audit of calculator-knowledgebase before republishing on ractive.ch/saturnus

Scope: CLAUDE.md (the clean-room rule is the yardstick), README.md, sources/manifest.json and all 167 files under wiki/ (index, log, overview, 1 emulator page, 29 hardware pages, 10 protocol pages, 40 question pages, 84 source pages), read in full at commit 9db1840. The repository was not modified.

The raw/ documents are not in the repository, so no text could be compared with its source. Every finding is a judgement from the wiki text: what the page says it took, how it is worded and laid out, and what the manifest says about the source's terms. The confidence levels reflect this. "High" means the text itself shows the problem (an instruction listing, a named private photograph, a sentence contradicting CLAUDE.md). "Medium" and "low" mean the wording, structure or attribution suggests copying or a terms conflict that someone with the source in hand must confirm.

## Summary

| Category | Findings | high / medium / low |
| --- | --- | --- |
| 1. ROM or program code | 17 | 2 / 7 / 8 |
| 2. Quotations and copied prose | 4 | 0 / 2 / 2 |
| 3. Tables, code or page layouts copied from a source | 19 | 0 / 11 / 8 |
| 4. Signs that GPL or other restricted source code was read | 12 | 1 / 6 / 5 |
| 5. Private material | 32 | 10 / 12 / 10 |
| 6. Republishing problems (terms, attribution, media) | 13 | 0 / 6 / 7 |
| **Total** | **97** | 13 / 44 / 40 |

**Verdict: not ready for republishing.** Most of the wiki is facts in its own words with citations, and the bulk of it can go out after targeted edits. Five groups of problems block publication as it stands:

1. **ROM code on the pages.** Two pages carry Saturn instruction listings disassembled from HP ROMs (timers.md:115, hp39g-40g-model-detection.md:35). memory-controller.md:280 has an 8-byte ROM dump, and several pages walk through ROM routine bodies by address (uart.md, card-ports.md, hp28s.md, hp42s-rom-crc.md). The ROM routine name `OUTBYT` comes from a personal-use-only listing.
2. **Private material.** Photographs of the owner's own 38G, 39g+ and 42S and of the owner's printed HP 49G Pocket Guide are cited as sources. The manifest lists the four booklet photographs. About twenty places name the owner (@ractive) as holder of unreleased Pioneer, 19BII and 28S ROM images.
3. **Clean-room statements that contradict CLAUDE.md.** overview.md:21 says GPL emulator sources are "read for facts only". Other pages mention an Emu48 "source tree" or "reference tree" and propose reading GPL connectivity-kit code. The Conn4x pages cite the restricted source routine by routine, name a routine ("CRC16") and quote its code comments, which goes beyond the documented exception (facts only, no routine names).
4. **Copied tables.** These include the three system-flag pages, which transcribe HP appendices entry by entry (the 49G one from a non-public booklet), the Intel datasheet tables, and Mastracci's tables reproduced against his "no changes" terms. Further tables come from the Saturn tutorial and Horn's Kermit notes.
5. **Unchecked terms.** 25 manifest entries say "not checked", and more say "not stated", "none found" or "-". These include the Saturn tutorial and Voyage, the two sources the most reproduced tables come from.

**Media: none.** The repository contains no images, PDFs, audio or other media files (git ls-files, checked by extension), and no page embeds or links an image file. The Commons photographs are cited by URL and not stored; the Pocket Guide photographs are listed in the manifest but not in git.

Not found: no e-mail addresses (apart from Usenet Message-IDs that resemble them, reported under 5), no local file-system paths (only repository-relative `raw/...` paths), no personal data beyond names and the owner's handle. No sign was found that x48ng, saturnng or Emu48 source code itself was quoted; the saturnng material is consistently described as black-box runs.

Suggested order of work: remove the listings, the dumps and the private-photo references (categories 1 and 5, high); correct overview.md:21 and the Conn4x citations (category 4); decide what to do with the system-flag pages and the copied tables (category 3); check the terms (category 6). Leave log.md and the question pages' working notes out of the encyclopedia: they hold most of the process detail (disassembly, private images, reference trees) and add little for readers.

## 1. ROM or program code (17)

### `wiki/hardware/timers.md:115`

- Excerpt: "`D0=(5) #00139`, `C=DAT0 S`, then `A=DAT0 S` and"
- Problem: Instruction listing of HP ROM code (ROM J/R #009A9-#009BD), disassembled from the ROM image and reproduced instruction by instruction, continuing on line 116 with `?A=C S GOYES`.
- Confidence: high
- Fix: Delete the mnemonics. Say only what the handler does: it waits until TIMER2's second nibble changes, i.e. it syncs to a 16-tick boundary. Drop the addresses.

### `wiki/questions/hp39g-40g-model-detection.md:35`

- Excerpt: "nibble #66F39 in ROM bank 1 (`D0=(5) #0011A`, `A=DAT0 B`, `?ABIT=1 3`,"
- Problem: Disassembly of a 39G/40G ROM primitive. Lines 35-38 give its instruction sequence and the ROM addresses of the routine and its caller (#66F16, a ROMPTR target).
- Confidence: high
- Fix: Replace with a prose fact: the ROM reads #11A bit 3 once during the cold start, and a set bit selects the 40G behaviour. Remove the mnemonics and the routine and caller addresses.

### `wiki/hardware/memory-controller.md:280`

- Excerpt: "`2369B108DADF1008`, matching the ROM image, so the reads are real"
- Problem: A 16-nibble (8-byte) dump of HP 48SX ROM J content at #00000. It is longer than a few bytes and is copied from the copyrighted ROM image.
- Confidence: medium
- Fix: Replace the dump with 'a control read of #00000 returned the ROM's first nibbles, so the reads are real'.

### `wiki/hardware/hp39g-40g.md:197`

- Excerpt: "(`0 F D F 3 0 0 0 2 0 0 3 0 0 4 ...`) where"
- Problem: A 15-nibble dump of the contents of the hpcalc rom.39g image (its I/O window). Short, but still raw content copied from the ROM file.
- Confidence: low
- Fix: Drop the nibble string and say only that the window holds non-zero I/O register values from the upload.

### `wiki/hardware/uart.md:242`

- Excerpt: "Read from the 48SX ROM J itself (disassembly with saturnus's disassembler,"
- Problem: Lines 242-275 walk through the bodies of ROM J's serial routines step by step (#003C8, #0283D, #004AE, #31416, #30FAA, #310CA, #317A9, #31638), worked out by disassembling the HP ROM. Line 255 also reproduces the 8-entry ROM data table at #0048B (300, 188, 150 ...).
- Confidence: medium
- Fix: Keep the hardware conclusions (TBF is checked before a write, TCS bit 3 is writable, port open writes #B to IOC, the receive wait is about 4 frames). Remove the routine addresses, the instruction-order walk-through and the verbatim table values.

### `wiki/hardware/card-ports.md:88`

- Excerpt: "How ROM J finds RAM cards (its own code, #09A18-#09A63 and #01DA1;"
- Problem: Lines 88-96 describe the body of a disassembled ROM J routine in execution order: the card-detect and RAM-test steps, its probe addresses and its result string.
- Confidence: medium
- Fix: Reduce to the observable fact (#10F bits 1/3 belong to the CE2 window, bits 0/2 to CE1, and the ROM size-tests the card) and drop the routine addresses and step order.

### `wiki/hardware/hp28s.md:450`

- Excerpt: "A loop at #10938 skips leading zeros; it counts the exponent down"
- Problem: Lines 441-458 trace the 28S ROM's number parser branch by branch (#108E7, #10938, #109A5-#109C1, #1099A) and say the 28C ROM holds 'the same nibble sequence'. This is a routine body worked out from the ROM.
- Confidence: medium
- Fix: Keep the user-visible quirk (0.00042 is stored unnormalised, so MANT and ENG show it). Drop the code addresses, the branch walk-through and the nibble-sequence comparison.

### `wiki/hardware/hp28s.md:406`

- Excerpt: "In saturnus (28S ROM) the pass ends in "OK-28S". Its ROM step (#0169B,"
- Problem: Describes the 28S self-test routine's body (entry addresses, read ranges, interleave) as worked out from the ROM.
- Confidence: low
- Fix: Keep 'the ROM step checks the hardware CRC over the ROM and expects #FFFF'. Move the read pattern into the general Lewis CRC description without the routine addresses.

### `wiki/hardware/hp28s.md:469`

- Excerpt: "In address order the ROM starts with a GOSBVL to its reset"
- Problem: Describes the ROM image's content by instruction and string offsets (GOSBVL to #004BD, copyright string at #3FFB7, OK-28S at #01C9F). This is ROM-listing detail.
- Confidence: low
- Fix: Keep the image-order fact (dump order versus address order). Drop the instruction and string offsets.

### `wiki/questions/hp42s-rom-crc.md:21`

- Excerpt: "The 42S self-test (EXIT + LN) has a ROM step at #01C44 (revision C). It"
- Problem: Lines 21-36 describe the self-test routine's body from a disassembly ('28S and 19BII ROMs, disassembled with saturnus', line 34): routine addresses #01C44, #01CB3, #044C9, the read order and the carry result.
- Confidence: medium
- Fix: State the behaviour (two CRC passes over the ROM, each must give #FFFF) without the routine addresses or the 'disassembled' wording.

### `wiki/hardware/hp42s.md:90`

- Excerpt: "The keyboard interrupt's handler debounces the keys itself: it scans the"
- Problem: Lines 88-98 trace the 42S ROM key handler (scan order, RAM bitmap at #500A6, main loop around #009A3). This is derived from tracing the ROM code.
- Confidence: low
- Fix: Keep the emulator requirement (the key interrupt must come from a whole-keyboard scan). Drop the internal RAM and code addresses.

### `wiki/hardware/lewis.md:165`

- Excerpt: "the ROM's byte routine is `OUTBYT`"
- Problem: ROM routine name and address (#031D6) taken from Gießelink's PIONEER.TXT listing. The manifest marks that listing 'free for personal use only'. It is a ROM name copied from a listing, not from published HP documentation.
- Confidence: medium
- Fix: Remove the name and address: 'the ROM has an IR byte-output routine used by the dump procedure'.

### `wiki/sources/emu42-pioneer-dump.md:30`

- Excerpt: "reads from address 0 with `A=DAT1 B` and sends each byte through the"
- Problem: Lines 29-31 repeat the dump program's instruction and the ROM routine name `OUTBYT` with its addresses for revisions A-C, copied from the personal-use PIONEER.TXT listing.
- Confidence: medium
- Fix: Describe the procedure in prose (a short program reads the ROM and sends it byte by byte over IR). Drop the mnemonic, the routine name and the addresses.

### `wiki/hardware/hp48-system-ram.md:48`

- Excerpt: "and the 49G: at #067F1-#067F4 it points D0 at #70579 (#806F8 on the 49G),"
- Problem: Describes a short ROM code sequence (its address and what each instruction loads), traced from the ROM.
- Confidence: low
- Fix: Keep 'confirmed by tracing the ROM's wake-up path' and drop the code address and the per-instruction description.

### `wiki/sources/hp48-internals-address-list.md:21`

- Excerpt: "`DynamicStart` #7056A, `HeapStart` #7056F, `FramePtr` (saved B, return"
- Problem: Lines 20-31 reproduce internal entry names and addresses from a 1991 community entry-point listing. The names may originate in HP's internal listings rather than in published documentation. hp48-system-ram.md lines 34-36 repeat them.
- Confidence: low
- Fix: Describe the locations in plain words (data stack pointer, HOME pointer) and cite the list. Do not reproduce its identifier names.

### `wiki/questions/da19-polarity.md:16`

- Excerpt: ""CBIT=1 7 % DA19=1, enable ROM" to switch to upper"
- Problem: Lines 16-17 quote two lines of assembly code, with their comments, from the Saturn tutorial.
- Confidence: low
- Fix: Paraphrase: 'the tutorial's example sets bit 7 of the LINECOUNT byte (DA19 = 1) to select upper ROM'.

### `wiki/log.md:227`

- Excerpt: "Disassembled and traced ROM J's serial code while building the saturnus"
- Problem: The log records that HP ROM code was disassembled and names a ROM table address (#0048B). This publishes the disassembly-based method behind the uart.md routine walk-through.
- Confidence: low
- Fix: If the log is republished, reword it to 'observed ROM J's serial behaviour in saturnus' and drop the table address. Better, leave log.md out of the encyclopedia.

## 2. Quotations and copied prose (4)

### `wiki/protocols/kermit.md:92`

- Excerpt: "| 1 | MAXL | tochar | longest packet I want to receive (LEN value, max 94) | 80 |"
- Problem: The Send-Init table (lines 90-103) keeps the Kermit manual's first-person wording ('I want to receive', 'you should time me out', 'padding characters I need', 'terminator I need'). That reads as text copied from the manual, without quote marks.
- Confidence: medium
- Fix: Rewrite the Meaning column in neutral third person ('maximum packet length the sender of this packet accepts'). Do not keep the manual's column set and order verbatim.

### `wiki/questions/custom-rom-for-saturn-calculators.md:102`

- Excerpt: "ended with "HP has been there and done that, and called it the HP49G" and"
- Problem: A quotation attributed only to message numbers (C1, C2), with no named speaker.
- Confidence: low
- Fix: Name the poster (from the C2 entry, John H Meyers) or paraphrase.

### `wiki/hardware/interrupts.md:199`

- Excerpt: "[[sources/hp48-faq]] 8.13): 0 log cleared (ON SPC ON); 1 low battery, deep"
- Problem: Lines 198-205 list all 16 WSLOG codes in the FAQ's order with its terse labels. They read like a near-verbatim copy of the FAQ list, without quote marks.
- Confidence: medium
- Fix: Keep only the codes the page uses (3, 4, 7, 8, B) in our own words, or describe the categories, and cite FAQ 8.13 for the full list.

### `wiki/hardware/system-flags-48gx.md:85`

- Excerpt: "| -54 | Math | Tiny array elements | RANK-type commands zero singular values below 1E-14"
- Problem: The Clear/Set cells follow the manual's flag appendix entry by entry and may be close paraphrases of HP's wording. Without the manual this cannot be checked; the same applies to system-flags-48sx.md and system-flags-49g.md.
- Confidence: low
- Fix: Spot-check a sample of cells against the manuals and reword any that track HP's sentences closely.

## 3. Tables, code or page layouts copied from a source (19)

### `wiki/hardware/saturn-cpu.md:43`

- Excerpt: "Mastracci's table leaves out the low-end Pioneers and the companion chips"
- Problem: The chip-version table at lines 32-38 is Mastracci's table from 1.5, reproduced row for row (the page itself calls it 'Mastracci's table'). Mastracci's terms forbid changes or additions.
- Confidence: medium
- Fix: Merge it into a single table in our own arrangement with saturn-models.md, which already covers the family from several sources.

### `wiki/hardware/saturn-cpu.md:222`

- Excerpt: "Encodings as given by Mastracci 3.4; the full opcode map is in"
- Problem: The chip-interface opcode table (lines 225-258) is Mastracci 3.4's opcode list with its encodings, copied as a table. overview.md line 36 claims the opcode table is 'cited, not copied'.
- Confidence: medium
- Fix: Keep the encodings the page needs inline in prose, or rebuild the table from several sources (SASM, tutorial) in our own grouping. Remove the 'as given by Mastracci' framing.

### `wiki/hardware/saturn-cpu.md:54`

- Excerpt: "Nineteen registers (src: [[sources/mastracci-saturn-guide]] 2.2):"
- Problem: The register table (lines 56-68) and the field-selector table (lines 108-120) mirror Mastracci 2.2 and 3.1 one to one.
- Confidence: low
- Fix: Reorder and reword, and cite several sources per row (SASM, tutorial). These facts are standard; only the layout copy is the issue.

### `wiki/hardware/memory-controller.md:133`

- Excerpt: "| C(B) | Next CONFIG is |"
- Problem: The C=ID code table (lines 133-144) reproduces the tutorial's p. 154 table, and the module tables at lines 17-26 and 48-54 reproduce Mastracci 2.4.
- Confidence: low
- Fix: Express the codes as a rule in prose ('#01-#07 and #19 ask for a size or the HDW address; #F2-#F8 ask for the address of NCE3 ... CE2') or combine the tables into one page-specific table.

### `wiki/hardware/uart.md:19`

- Excerpt: "| Addr | Bits | Meaning |"
- Problem: The UART register table (lines 19-38), the baud table (42-46) and the IR table (146-158) are Mastracci 4.5 and 4.7 reproduced as tables, each attributed to one section.
- Confidence: medium
- Fix: Fold them into one register table cross-sourced with Voyage, the I/O guide and Emu48 names, in our own column layout.

### `wiki/hardware/io-ram.md:23`

- Excerpt: "| Addr | Bits | Function | Page |"
- Problem: The full #100-#13F register map (lines 23-60) is built from Mastracci 4.2-4.11 and Voyage's overview tables p. 192 and 200 (line 62), and follows their order and grouping.
- Confidence: low
- Fix: Acceptable as a fact index, but rephrase the cells and add the per-row source so that it is a compilation, not a copy of either overview table.

### `wiki/hardware/display.md:129`

- Excerpt: "| Reg | Name | Bits |"
- Problem: The bit-name table (lines 129-140) reproduces the tutorial's register tables p. 169-171, including its quoted warnings. The ghost-register table (lines 151-156) reproduces the tutorial's p. 166 table.
- Confidence: medium
- Fix: Turn the bit names into prose per register, or merge them with the Mastracci and Emu48 names in one table in our own layout.

### `wiki/hardware/timers.md:37`

- Excerpt: "| Bit | #12E (TIMER1) | #12F (TIMER2) |"
- Problem: The control-nibble tables (lines 37-44 from Mastracci 4.10, lines 48-53 from Voyage p. 203) each mirror one source's table.
- Confidence: low
- Fix: Merge into one table with both sources per row, in our wording.

### `wiki/hardware/interrupts.md:153`

- Excerpt: "1. Set ST bit 14 ("inside the handler"), then branch on ST bit 15 without"
- Problem: The 15-step list (lines 153-184) follows the structure and order of Duchesne's commented ROM disassembly step by step. It mirrors the source's layout and, in turn, the ROM routine's structure.
- Confidence: medium
- Fix: Condense to the emulator-relevant checks (TIMER2 running, RAM check word, 48S C=ID check, OUT shadow, RSTK budget) as an unordered summary in our own words.

### `wiki/protocols/kermit.md:42`

- Excerpt: "| Type | Meaning |"
- Problem: The packet-type table (lines 42-59), the generic-command table (lines 179-197) and the packet diagram (lines 23-25) reproduce the Kermit Protocol Manual's tables and figure.
- Confidence: low
- Fix: Keep the letters needed for HP (S, Y, N, F, D, Z, B, E, R, I, C, G and G D/F/L) in prose and point to the manual for the full lists.

### `wiki/protocols/server-commands.md:33`

- Excerpt: "| Packet | HP 48 server response |"
- Problem: Lines 33-45 reproduce Joe Horn's table from the Columbia 'HP-48 Kermit hints' page (Columbia University copyright), including his wording ('useless').
- Confidence: medium
- Fix: Rewrite as prose in our own words, merged with the saturnng observations below it, and attribute Horn.

### `wiki/hardware/system-flags-49g.md:55`

- Excerpt: "| Flags | Topic | Name | Clear | Set | Status | Source |"
- Problem: Lines 55-161 transcribe the whole 'System Flags' list of the HP 49G Pocket Guide (p. 76-79), row for row in the booklet's order. The booklet is copyrighted, not published online, and was read from private photographs.
- Confidence: medium
- Fix: Keep only the facts needed (meanings that differ from the 48G, defaults, contradictions), or obtain HP's permission. At minimum, reword every cell and drop the one-row-per-booklet-entry structure.

### `wiki/hardware/system-flags-48gx.md:42`

- Excerpt: "| Flags | Topic | Name | Clear | Set | Status | Source |"
- Problem: Lines 42-95 transcribe the 48G User's Guide appendix D flag table entry for entry (page 13: 'transcribed ... from the system flag appendix').
- Confidence: medium
- Fix: Reduce to a summary of flags that matter to an emulator or encyclopedia reader, or reword each cell and give a different arrangement (by topic) so that it is not the appendix's table.

### `wiki/hardware/system-flags-48sx.md:44`

- Excerpt: "| Flags | Topic | Name | Clear | Set | Status | Source |"
- Problem: Lines 44-97 transcribe appendix E of the 48SX Owner's Manual entry for entry.
- Confidence: medium
- Fix: As for the 48GX page: summarise or rearrange and reword. Consider one combined page listing only the differences between models.

### `wiki/sources/intel-28f160s5.md:37`

- Excerpt: "## Command set (Table 3, p. 14)"
- Problem: The command table (lines 42-57) and the status-register table (lines 86-95) reproduce Intel's datasheet Tables 3 and 15. The manifest records Intel copyright.
- Confidence: medium
- Fix: Describe in prose the commands the 49G uses (#FF, #70, #50, #E8 ... #D0, #40, #20/#D0) and the status bits that matter, and cite the tables instead of copying them.

### `wiki/sources/emu42-problems.md:18`

- Excerpt: "| Offset | Name | Bits not emulated |"
- Problem: Lines 18-25 reproduce the PROBLEMS.TXT register list of the GPL Emu42 package as a table, apparently verbatim.
- Confidence: medium
- Fix: Keep the register names in prose on hardware/lewis.md, which already holds them, and drop the copied table.

### `wiki/protocols/iopar.md:25`

- Excerpt: "| # | Field (command) | Values | Default |"
- Problem: The IOPAR field table follows the AUR's D-5/D-6 field list in order and content.
- Confidence: low
- Fix: Acceptable as facts. Reword the cells and keep citations, or merge them with the Kermit-hints values in a layout of our own.

### `wiki/hardware/hp49g.md:104`

- Excerpt: "| ROM | Bank 0 | System banks | User banks | RAM port 0 | RAM port 1 |"
- Problem: The flash-use table (lines 104-108) reproduces the tutorial's table on p. 161-162.
- Confidence: low
- Fix: Express it in a sentence per ROM range, or combine it with Smith's and the FAQ's figures.

### `wiki/hardware/hp48sx.md:26`

- Excerpt: "| Range | Contents |"
- Problem: The default memory map (lines 26-33) is the tutorial's p. 156 map in table form.
- Confidence: low
- Fix: Acceptable as a fact table. Merge it with the saturnus boot observation on the same page and the CONFIG values so that it is our own compilation.

## 4. Signs that GPL or other restricted source code was read (12)

### `wiki/overview.md:21`

- Excerpt: "GPL emulator sources are not part of this"
- Problem: Lines 21-22 say GPL emulator sources are 'read for facts only'. That contradicts CLAUDE.md (GPL emulator source is never opened, not even to look up a fact) and, published, states that GPL source was read.
- Confidence: high
- Fix: Correct the sentence to 'GPL emulator source code is never opened; emulator facts come from their documentation and black-box runs.' Confirm with the owner that this is true.

### `wiki/questions/hp38g-39g-transfer-protocol.md:36`

- Excerpt: "(GPL, so facts only) or the "aplet disk drive" documentation."
- Problem: Lines 35-36 propose reading the connectivity kit's code '(GPL, so facts only)', i.e. reading GPL source for facts, which the rule forbids beyond the HPGComm exception.
- Confidence: medium
- Fix: Reword to name only the documented exception (the owner's HPGComm source), or delete the suggestion. The question is answered anyway.

### `wiki/sources/emu48-changes.md:17`

- Excerpt: "changelog, not code, and was the only file read in the Emu48 source tree."
- Problem: States that the Emu48 source tree was opened to read CHANGES.TXT. The 'reference tree' notes at log.md:262 and hp39g-40g-model-detection.md:21 point the same way. The wording invites doubt that the clean-room boundary held.
- Confidence: medium
- Fix: Cite the change log from its public URL (hp.giesselink.com/Emu48/CHANGES.TXT, as the raw field already does) and remove 'in the Emu48 source tree'.

### `wiki/questions/hp39g-40g-model-detection.md:21`

- Excerpt: "CHANGES.TXT lies in the reference tree this work may not open; ask someone"
- Problem: Mentions a local 'reference tree' holding Emu48 files that 'this work may not open'. This shows a GPL source tree sat alongside the work, and it contradicts emulators/emu48.md, which cites CHANGES.TXT throughout.
- Confidence: low
- Fix: Delete the paragraph (the question is answered) or replace it with 'Emu48's public change log was not checked for this'.

### `wiki/log.md:261`

- Excerpt: "Emu48's CHANGES.TXT was not re-read (it"
- Problem: Line 262 says CHANGES.TXT 'sits in the reference tree', the same local GPL source tree reference as above.
- Confidence: low
- Fix: Reword or leave log.md out of the encyclopedia.

### `wiki/sources/mastracci-saturn-guide.md:37`

- Excerpt: "Secondary compilation from Gariepy, Nickel, Brittenson, Ervin, Teuwen, Duchesne,"
- Problem: Line 38 records that Mastracci compiled his guide partly from 'x48 source', a GPL emulator. Every fact taken only from Mastracci may therefore come indirectly from GPL emulator code.
- Confidence: low
- Fix: Flag Mastracci-only facts in the encyclopedia and prefer an independent source (Voyage, the tutorial, SASM) where one exists. Say this on the source page.

### `wiki/protocols/xserv.md:30`

- Excerpt: "quiet [command sender]. Official letters (src: [[sources/hp49-rom118-notes]]):"
- Problem: Throughout lines 30-58 every fact carries a bracketed tag naming the Conn4x code unit it was read from ([command sender], [put command], [command packet sender], [reply packet receiver], [directory listing]). This is a routine-level map of the restricted source. The exception allows facts only, 'no routine names'.
- Confidence: medium
- Fix: Replace the bracket tags with a single citation '(src: [[sources/conn4x-ymodem-pas]])'. Keep the facts.

### `wiki/protocols/xmodem-hp.md:38`

- Excerpt: "From [[sources/conn4x-ymodem-pas]] (the part of the code in brackets):"
- Problem: Lines 38-60 cite Conn4x code parts in brackets ([YModem sender], [YModem receiver], [header check]) and name source files ([DownLd.pas]). Line 41 notes that a CRC is 'computed by a routine not in the source'. These are code-structure details from the non-commercial source.
- Confidence: medium
- Fix: Cite '(src: Conn4x sources)' once, drop the bracketed code-part names and the file name DownLd.pas.

### `wiki/questions/xmodem-hp-crc-mode.md:13`

- Excerpt: "[[sources/conn4x-ymodem-pas]] YModem sender). Its "CRC16" routine is a"
- Problem: Lines 13-15 name a Conn4x routine ('CRC16') and quote Graves's source-code comments ('obviously wrong', 'not detected in use with HP49'). That is a routine name plus quoted code text from the restricted source.
- Confidence: medium
- Fix: Say 'the Conn4x client's CRC-16 code is marked by its author as wrong and unused with the 49G', without the routine name or the quoted comments.

### `wiki/sources/conn4x-ymodem-pas.md:36`

- Excerpt: ""CRC16" routine is a reflected table labelled by Graves as "obviously wrong""
- Problem: Lines 35-39 repeat the routine name and the quoted code comments. Line 22 names the source file FixObj.pas and 'its object-fixing step'.
- Confidence: medium
- Fix: Remove the routine name and the comment quotations. Describe the reliability point in prose.

### `wiki/protocols/hp-object-format.md:127`

- Excerpt: "[[sources/conn4x-ymodem-pas]], FixObj.pas)."
- Problem: Cites a specific Conn4x source file (FixObj.pas) as the location of a fact. binary-odd-nibble-padding.md lines 15 and 20 do the same.
- Confidence: low
- Fix: Cite the source page only.

### `wiki/overview.md:109`

- Excerpt: "SATURN.TXT from the 1993 SDK, and x48ng."
- Problem: Lists x48ng, a GPL emulator, among sources 'not yet used' without saying documentation only. Republished, this reads as a plan to use its code.
- Confidence: low
- Fix: Write 'x48ng's manual and change log (documentation only)' or drop the item.

## 5. Private material (32)

### `wiki/hardware/keyboard.md:207`

- Excerpt: "On @ractive's unit the"
- Problem: Refers to the owner's own 49G (identified by GitHub handle) as evidence for label colours.
- Confidence: medium
- Fix: Cite the user manual only, or write 'on a 49G checked by the author' without the handle. Better, cite a public photograph.

### `wiki/hardware/keyboard.md:210`

- Excerpt: "(@ractive's photographs of a 38G, saturnus skin)."
- Problem: Cites private photographs of the owner's own 38G, which CLAUDE.md forbids.
- Confidence: high
- Fix: Remove the reference and cite the 38G user's guide (inside cover, turquoise shift) or a public Commons photograph.

### `wiki/hardware/keyboard.md:212`

- Excerpt: "(@ractive's photo of an HP 39g+, whose keyboard is taken to be the"
- Problem: Cites a private photograph of the owner's own calculator.
- Confidence: high
- Fix: Remove it. Cite the 39G/40G user's guide or a public photograph, or mark the colours unverified.

### `wiki/hardware/keyboard.md:215`

- Excerpt: "(@ractive's photographs of a 42S)."
- Problem: Cites private photographs of the owner's own 42S.
- Confidence: high
- Fix: Remove it. Cite the 42S owner's manual or a Commons photograph.

### `wiki/hardware/hp38g.md:226`

- Excerpt: "above the 2 key on the unit (@ractive's photographs of a 38G, which"
- Problem: Cites private photographs of the owner's own 38G as confirmation of every label.
- Confidence: high
- Fix: Remove the photograph reference and cite the 38G user's guide figure (inside cover).

### `wiki/hardware/hp38g.md:243`

- Excerpt: "on the owner's real 38G in DEG mode (2026-10-19). Not checked on the"
- Problem: Refers to the owner's own calculator as a private test bench. Not a photograph, but private material by the rule's spirit.
- Confidence: low
- Fix: Write 'confirmed on a real 38G' without 'the owner's'.

### `wiki/hardware/hp48sx.md:56`

- Excerpt: "saturnus with ROM J and confirmed on the owner's real 48SX (2026-10-19), so"
- Problem: Refers to the owner's own calculator.
- Confidence: low
- Fix: Write 'confirmed on a real 48SX'.

### `wiki/hardware/timers.md:128`

- Excerpt: "Inferred from the ROM's code and from @ractive's real 48SX, which does"
- Problem: Names the owner's own calculator by handle.
- Confidence: low
- Fix: Write 'a real 48SX'.

### `wiki/hardware/system-flags-49g.md:14`

- Excerpt: "read from @ractive's photographs of a"
- Problem: The whole 49G flag table rests on private photographs of the owner's printed manual, which CLAUDE.md forbids as private material.
- Confidence: high
- Fix: Remove the photograph reference. Cite the booklet by title and page only, and see the category 3 and 6 findings on whether the transcription may be published at all.

### `wiki/hardware/system-flags-49g.md:171`

- Excerpt: "read, iteration 12d; not checked on @ractive's 49G). Against the"
- Problem: Refers to the owner's own 49G.
- Confidence: low
- Fix: Delete the parenthesis or write 'not checked on a real 49G'.

### `wiki/sources/hp49g-pocket-guide.md:20`

- Excerpt: "What the library has: four of @ractive's photographs of a printed copy, taken"
- Problem: The source page is built on private photographs of the owner's manual. Lines 25-29 list their local file names.
- Confidence: high
- Fix: Rewrite the page to cite the printed booklet itself (title, page numbers). Remove the photograph description and file list.

### `wiki/index.md:60`

- Excerpt: "(@ractive's photographs) (digested)"
- Problem: The index entry names the private photographs.
- Confidence: high
- Fix: Drop '(@ractive's photographs)'.

### `wiki/log.md:459`

- Excerpt: "@ractive photographed the booklet's "System Flags" pages (p. 76-79), now"
- Problem: The log records the private photographs and where they are kept (raw/manuals/hp49g-pocket-guide/).
- Confidence: high
- Fix: Leave log.md out of the encyclopedia, or reword it to 'ingested the Pocket Guide's System Flags pages'.

### `wiki/questions/hp49g-system-flags.md:32`

- Excerpt: "describes, and a check on @ractive's 49G, remain open."
- Problem: Refers to the owner's own 49G.
- Confidence: low
- Fix: Write 'a check on a real 49G'.

### `wiki/questions/hp49g-system-flags.md:37`

- Excerpt: "No photograph was hard to read; nothing here waits on @ractive's"
- Problem: Refers to the private photographs of the owner's booklet.
- Confidence: high
- Fix: Delete the sentence.

### `sources/manifest.json:970`

- Excerpt: ""The HP 49G Pocket Guide, System Flags pages 76-79 (photographs of a printed copy)""
- Problem: Four manifest entries (lines 968-1030) list private photographs of a printed manual, with sizes and hashes.
- Confidence: high
- Fix: Remove the four entries, or replace them with one entry describing the booklet as a printed source with no file.

### `README.md:78`

- Excerpt: "private photographs that are not distributed, and text conversions that"
- Problem: The README mentions private photographs among the sources.
- Confidence: medium
- Fix: Remove 'a few private photographs that are not distributed'.

### `wiki/hardware/hp42s.md:108`

- Excerpt: "## @ractive's ROM image"
- Problem: A section about the owner's privately held 42S ROM image (with its SHA-256 on line 111). HP never published this ROM; it publicly ties a named person to an unreleased ROM dump.
- Confidence: medium
- Fix: Rename the section 'A revision C ROM image' and drop the handle. Consider dropping the hash too.

### `wiki/hardware/hp17bii.md:264`

- Excerpt: "@ractive's images, run in saturnus (iteration 42); both are international"
- Problem: Names the owner as holder of unreleased 17B/17BII ROM images. Lines 270-271 give their SHA-256 hashes.
- Confidence: medium
- Fix: Write 'international images of both models' without the handle. Keep or drop the hashes as policy decides.

### `wiki/hardware/hp27s.md:231`

- Excerpt: "@ractive's image, run in saturnus (iteration 42): 65536 bytes, SHA-256"
- Problem: Names the owner as holder of an unreleased 27S ROM image.
- Confidence: medium
- Fix: Remove the handle.

### `wiki/hardware/hp19bii.md:27`

- Excerpt: "Facts marked "(19BII ROM)" were found by running @ractive's image in"
- Problem: Names the owner as holder of a 19BII ROM image. The page itself says such images only came from people at HP (line 220).
- Confidence: medium
- Fix: Remove the handle: 'by running a 19BII ROM image in saturnus'.

### `wiki/hardware/hp19bii.md:222`

- Excerpt: "F3). @ractive's image is 131072 bytes in dump order (256 units of 512"
- Problem: Second reference on this page to the owner's 19BII image.
- Confidence: medium
- Fix: Remove the handle.

### `wiki/hardware/saturn-models.md:173`

- Excerpt: "in saturnus on @ractive's images, iteration 42):"
- Problem: Names the owner as holder of Pioneer ROM images.
- Confidence: medium
- Fix: Remove the handle.

### `wiki/hardware/saturn-models.md:194`

- Excerpt: "address order. @ractive's 128 KB 19BII and 28S images are in dump order"
- Problem: Names the owner as holder of 19BII and 28S ROM images.
- Confidence: medium
- Fix: Remove the handle.

### `wiki/hardware/clocks.md:107`

- Excerpt: "searched in @ractive's 42S image (revision C), `DATE` and `APPT` do not"
- Problem: Names the owner's private ROM image.
- Confidence: medium
- Fix: Write 'searched in a revision C 42S image'.

### `wiki/hardware/clocks.md:126`

- Excerpt: "- Seen in saturnus (owner's ROMs, 2026-10-13): in 12-hour format the"
- Problem: Refers to the owner's private ROM images.
- Confidence: low
- Fix: Drop 'owner's ROMs'.

### `wiki/questions/hp42s-rom-crc.md:2`

- Excerpt: "title: Does @ractive's 42S ROM image pass the ROM's own CRC test?"
- Problem: The page title (repeated in the H1 on line 10) and lines 41, 96 and 115 name the owner's private ROM images.
- Confidence: medium
- Fix: Retitle 'Does a 42S ROM image pass the ROM's own CRC test?' and remove the handle on lines 10, 41, 96 and 115.

### `wiki/index.md:177`

- Excerpt: "Does @ractive's 42S ROM image pass the ROM's own CRC test? (answered)"
- Problem: The index repeats the handle in the page title.
- Confidence: medium
- Fix: Update it together with the page title.

### `wiki/hardware/hp28s.md:45`

- Excerpt: "Facts marked "(28S ROM)" were found by running the owner's revision 2BB"
- Problem: Refers to the owner's private 28S ROM image (also on lines 412 and 465, with its SHA-256).
- Confidence: low
- Fix: Write 'a revision 2BB image' on lines 45, 412 and 465.

### `wiki/log.md:374`

- Excerpt: "([[sources/kml20]]). Traced @ractive's revision C ROM in saturnus."
- Problem: The log names the owner's private ROM images (also on lines 385, 724 and 737).
- Confidence: low
- Fix: Leave log.md out of the encyclopedia, or remove the handle.

### `wiki/sources/hpgcomm.md:19`

- Excerpt: "source are in the owner's private repository; **the source is not"
- Problem: Refers to the owner's private repository. With the authors field (line 4) and CLAUDE.md's 'mainly written by the owner', it also ties the owner's real name to the @ractive handle used elsewhere.
- Confidence: low
- Fix: Write 'the 2.01 source, which is not public'. Decide consciously whether the encyclopedia should identify the owner.

### `wiki/sources/comp-sys-hp48-archive.md:36`

- Excerpt: "| # | Date | Author | Subject | Message-ID |"
- Problem: Lines 36-80 list 43 named Usenet posters with Message-IDs, several of which look like e-mail addresses (e.g. 361BA4B3.3FF5@hp.com). These are public posts, but an encyclopedia does not need personal identifiers.
- Confidence: low
- Fix: Keep author names and dates. Drop the Message-ID column or keep it only in the repository, not on the public site.

## 6. Republishing problems (terms, attribution, media) (13)

### `sources/manifest.json:4616`

- Excerpt: "free to copy and distribute provided no changes or additions are made"
- Problem: Mastracci's guide may only be passed on unchanged. The wiki copies several of its tables (saturn-cpu, uart, memory-controller, timers, card-ports) in modified form. Facts are free, but copied tables are an adaptation his terms do not allow. The same terms are on lines 4630 and 4645.
- Confidence: medium
- Fix: Rebuild every Mastracci-derived table in our own layout from several sources (see category 3), or ask Mastracci for permission.

### `sources/manifest.json:2895`

- Excerpt: "no commercial use without permission"
- Problem: Conn4x source terms (also the help pages, line 1206, and XSrvr48, line 3028): no commercial use without permission, keep notice. If the encyclopedia on ractive.ch carries ads or is part of a commercial offering, facts drawn from the source and the routine-level citations are a risk.
- Confidence: low
- Fix: Confirm the site is non-commercial, keep only facts (see category 4) and credit Graves and de Brébisson on the XSERV page.

### `sources/manifest.json:110`

- Excerpt: ""terms": "free for personal use only (the listing)""
- Problem: PIONEER.TXT (and CLAMSHEL.TXT, line 49) are free for personal use only. The wiki republishes the listing's routine name, addresses and dump procedure (lewis.md:165, emu42-pioneer-dump.md:29-31, saturn-models.md ROM table).
- Confidence: medium
- Fix: Reduce what is taken from these files to facts in our own words, without the listing's names and addresses.

### `sources/manifest.json:975`

- Excerpt: ""terms": "HP copyright; photographs not distributed""
- Problem: The Pocket Guide is HP copyright and not public. The encyclopedia would publish a complete transcription of its flag list (system-flags-49g.md). The photographs also cannot back the citations for readers.
- Confidence: medium
- Fix: Publish only selected facts with page citations, or get HP's permission. Remove the photograph entries (category 5).

### `sources/manifest.json:3186`

- Excerpt: ""terms": "Intel copyright, standard datasheet disclaimer""
- Problem: Intel datasheet: its tables are reproduced on sources/intel-28f160s5.md (category 3).
- Confidence: medium
- Fix: Replace the copied tables with prose and citations.

### `sources/manifest.json:1130`

- Excerpt: ""terms": "Columbia University copyright""
- Problem: The Kermit hints page is Columbia copyright. Horn's server table is reproduced on server-commands.md (category 3).
- Confidence: low
- Fix: Paraphrase the table and attribute Horn and Columbia.

### `sources/manifest.json:18`

- Excerpt: ""terms": "not checked","
- Problem: 25 manifest entries have terms 'not checked' (e.g. KML 2.0 here and lines 177 and 191, Duchesne 3120, Cannon 3157/3172, Voyage 4374/4389, buf49 4406, keyb49 4438, serial49 4534, hphard 3539). The encyclopedia would publish material derived from them, including tables from Voyage and the tutorial, without knowing whether that is allowed.
- Confidence: medium
- Fix: Check and record the actual terms of every source the encyclopedia draws tables or quotations from, starting with Voyage, the Saturn tutorial, Duchesne and KML 2.0.

### `sources/manifest.json:210`

- Excerpt: ""terms": "not stated","
- Problem: Garnier's 42S page (also line 226) and Hosoda's notes (lines 245 and 259) have no stated terms. The wiki quotes both.
- Confidence: low
- Fix: Record the terms (or the absence of a licence) and keep quotations to a sentence, attributed.

### `sources/manifest.json:3138`

- Excerpt: ""terms": "none found","
- Problem: Saturn tutorial: no terms found, yet it is the most-used source and several of its tables are reproduced (C=ID codes, display bit names, ghost registers, flash use).
- Confidence: medium
- Fix: Find the tutorial's licence (hpcalc.org listing, authors) or rewrite the reproduced tables.

### `sources/manifest.json:577`

- Excerpt: ""terms": "-","
- Problem: FAQ index entry has '-' as terms, and 20 FAQ entries say only 'usual FAQ redistribution'. The WSLOG list is near-verbatim from the FAQ (category 2).
- Confidence: low
- Fix: Record the FAQ's actual redistribution notice and paraphrase the copied list.

### `sources/manifest.json:814`

- Excerpt: ""terms": "community scan","
- Problem: 22 HP manuals are recorded only as 'community scan'. The underlying copyright is HP's, and the system-flag pages transcribe their appendices (category 3).
- Confidence: low
- Fix: Record 'HP copyright; community scan hosted by hpcalc.org' and limit the flag pages to facts in our own arrangement.

### `wiki/sources/commons-hp28s-photos.md:26`

- Excerpt: "- **P2**, open, front; Ralf Pfeifer, CC BY-SA 3.0 de, taken 2009-11-02."
- Problem: Web sources used by the wiki are not in the manifest, so no terms are recorded centrally: Commons photos under CC BY-SA (here, and Joe Haupt's 17B photo in commons-pioneer-photos.md:26), Wikipedia (CC BY-SA), comp.sys.hp48, the HP Museum forum, Finseth and Gießelink's pages. Only facts are drawn from them, so the risk is low, but attribution should be visible on the published pages that use them.
- Confidence: low
- Fix: Add manifest entries or a terms line for web sources, and credit photographers and Wikipedia on the case-layout and model pages that use them.

### `wiki/protocols/aplet-transfer.md:70`

- Excerpt: "[[sources/hpgcomm]] source)."
- Problem: Facts marked '(src: hpgcomm source)' rest on an unpublished GPL-2 source with co-authors (Davis, Croft). Encyclopedia readers cannot check them, and the source page says it is not public.
- Confidence: low
- Fix: Keep only facts confirmed by black-box runs or public HPGComm documents, or publish the source.
