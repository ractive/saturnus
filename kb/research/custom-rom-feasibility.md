---
title: "A free ROM of our own: feasibility"
type: research
date: 2026-10-12
status: active
tags:
  - research
  - saturnus
  - rom
---

# A free ROM of our own: feasibility

Owner's question: has anyone written their own OS or ROM for the Saturn
calculators, and what would it take to write a small free (MIT) ROM that
boots in saturnus, and perhaps on a real 49G? The facts, with citations,
are on the wiki page `questions/custom-rom-for-saturn-calculators`
(calculator-knowledgebase PR 7, with a second pass in PR 8). PR 8 read
the whole comp.sys.hp48 archive and the HP Museum forum, which the first
pass could not reach. This note gives the findings and a recommendation.

## Findings

- **No free standalone Saturn ROM exists.** Nothing found boots from
  address #00000 without HP code. The nearest is **ShellOS**
  (1998-1999): a replacement OS for the 48GX on a RAM card in slot 1. Its
  authors say it uses no HP ROM code, but it is started from the RPL OS
  and has no source or licence. Its sources were lost; the author
  promised a public-domain release in 2001 that never came. A user
  describes it mapping its kernel over #00000 from a card. Three other
  OS projects from 2001 (SOS, WinSOS, and one by Tsiros and Busby) never
  got past their kernels. Nobody in the newsgroup ran their own code from
  the reset address. In 2000 Emu48's maintainer said this could hardly be
  tested without replacing the ROM. **Meta Kernel** replaces only the
  48GX shell. **newRPL** and **HPGCC** target only the ARM models (49g+,
  50g, 39gs, 40gs, 48gII, Prime G1) and have no Saturn port.
- **Games and demos take the machine over at run time and are not
  ROMs.** They include two Doom-style games from 1997-1998 and many
  greyscale games. The documented method is to disable interrupts, write
  the display registers directly, and scan the keyboard with OUT and
  C=IN. For a program's own interrupt handler, map RAM or a card
  at address #00000, because every interrupt calls #0000F. Brébisson's
  ray-casting Doom ran at 26, then 50, frames a second in greyscale
  (his posts, 1995-1996).
- **On a 48SX/GX the ROM cannot be replaced in the field.** It is a
  byte-wide mask ROM in production (HP posters). Only prototypes had OTP
  chips, which HP swapped by soldering. The CPU always starts in NCE1
  (ROM), and a card can only take over after HP's ROM has mapped it.
- **The CPU's register state after reset is undocumented**, even to
  Emu48's maintainer. A ROM of our own must set everything it relies on.
- **On a 49G, HP's boot sector always runs first.** It is the first 64 KB
  of flash, write-protected in hardware and mapped at #00000 at reset. It
  provides the "No System" menu (+ ENTER ON) and "Download System" over
  serial with HP's Connectivity Kit 3.0. Recovery after a failed upgrade
  is the reset hole and the same menu. On saturnus, HP's boot code stops
  in "No System" when its bank scan finds nothing valid.
  - **Now known** (PR 8, from posts by HP's developers):
    - Every bank starts with a header at byte #100. It holds a type
      (System, ROM, RAM or Boot), the bank number and an erase count that
      the boot code fills in, the offset of a CRC, an init hook called at
      every boot, and a list of "flash pointers".
    - "Download Pack" checks that the system is valid and complete, and
      reports bad CRCs. Nobody mentions a signature.
    - A `.flash` file is a count followed by header and data for each
      part (FS, System, Part1-Part7).
    - In terminal mode the calculator waits for a short handshake from
      HP's flashing program. The menu then offers Download System,
      Download Packs, Upload Bank and Erase Bank. The boot loader holds
      the XModem and flash routines.
    - Bank 0's second half ("FS") can replace the boot sector. HP's ROMs
      from 1.19 use it to patch the boot. A broken FS bank makes
      reflashing hard.
    - Third parties have flashed self-built HP systems (casrelease 1.24,
      Parisse's geometry ROM). Nobody flashed a non-HP system.
    - The only bricked 49Gs on record had a damaged boot sector (HP's
      developers, on early units without the write protection) or a
      damaged FS bank.
  - **Still unknown:** the exact handshake bytes, the per-bank framing,
    and the CRC algorithm. Whether a system with no FS bank boots under
    the original boot sector once an FS bank from 1.19 or later is
    present is also unknown.
- **The 38G, 39G and 40G cannot be flashed.** The 38G's ROM is OTP and
  the 39G/40G's is mask ROM.
- **Toolchains.** HP's SASM is a free-to-redistribute DOS binary. HP
  Tools 3.0.8 is GPL with source. Areuh is CeCILL-B (BSD-like) but built
  for the HP-71 and lacks some 48 opcodes. KASM (GPL-2) and HP48XGCC (GCC
  2.7.2, GPL-2) were last updated in 2013. None of them is a good fit for
  an MIT repository's build.
- **What a ROM must do at reset** (from documentation, not from HP's
  code):
  - Execution starts at #00000. HP's documents imply this, and every HP
    ROM boots from there on saturnus.
  - The interrupt vector is #0000F, so the code must jump over it and
    put a handler there.
  - CONFIG HDW (I/O at #00100), then NCE2 (RAM), then CE1, CE2 and NCE3.
    Nothing but ROM is mapped until then.
  - Set the display start (#120), the line count (#128), the menu
    bitmap (#130), the contrast and the display enable.
  - TIMER2 and SHUTDN are enough for a clock and for sleep.

## Recommendation

Build it, for saturnus's sake rather than for real hardware. A free ROM
fixes two standing gaps: the decision log's "No ROMs in CI" (full-machine
e2e tests could run on every push) and the web app's empty first visit
(it could boot something without a download). It also exercises
documented hardware behaviour that HP's ROMs barely touch.

**Scope of a minimal demo ROM** (MIT, 48GX layout first):

- Reset code at #00000 that jumps over the vector, with an interrupt
  handler at #0000F. The handler acknowledges keyboard, timer and ON
  interrupts and returns with RTI.
- Configure the memory controllers in daisy-chain order, with RAM
  at address #80000.
- Set up the display: clear the bitmap, draw a text banner with **our
  own** 5x7 font, and show a running clock from TIMER2.
- Echo key names from the keyboard matrix on screen. ON+key makes the
  machine sleep with SHUTDN and ON wakes it.
- Optional: a serial "hello" through the UART at 9600 bps, so that satx
  and the serial bridge can be tested against it.
- Variants after that: 48SX (RAM at #70000) and 38G/39G (other key
  layout and RAM base). For the 49G, saturnus needs our own boot-sector
  stand-in in bank 0. That variant runs in the emulator only, because
  HP's boot sector stays on real hardware.

**Toolchain:** our own small Saturn assembler in Rust. It encodes from
the opcode facts in the wiki and SASM docs and checks itself by
round-trip against `saturnus-core`'s existing decoder and disassembler,
which is a strong free test oracle. It could live as a tool crate
(`saturnus-asm`), with the ROM source as `.s` files in the repo and the
binary built in CI. This avoids a GPL or DOS dependency in the build and
any question about where the encodings came from. Areuh (CeCILL-B) is
the fallback if writing an assembler turns out to cost more than an
iteration. Its opcode table would need the 48 additions.

**Estimate:** three iterations for the emulator side.

1. The assembler: SASM-like syntax, labels, the expressions the ROM
   needs, and round-trip tests against the decoder.
2. The 48GX demo ROM, plus a ROM-free e2e test in CI that runs it and
   compares the screen to a golden image.
3. The 48SX, 38G/39G and emulated-49G variants, and making it the web
   app's default when no HP ROM is loaded.

A real 49G is still a separate decision, but no longer blocked. PR 8
documents the bank header and the boot menu from public posts by HP's
developers. What is left is the serial side of "Download System" and the
bank CRC. One research iteration could close it: a black-box run of
HP's flashing program or of ROMUPLOAD against saturnus's emulated 49G,
watching the serial bytes and the flash writes, needs none of HP's code.
After that, a 49G system of our own in banks 1-7 behind HP's boot sector
is a fourth iteration.

## Risks

- **Clean-room.** The ROM must contain no HP code or data:
  - **Font:** draw our own; never copy the HP font bitmaps.
  - **Entry layout:** no HP entry-point layout or RPL structures.
  - **Our own boot sector for the emulated 49G:** do not imitate HP's
    bank headers. The emulator side needs none of HP's code.
  - **The real 49G is now less of a grey area.** The bank header that
    HP's boot code expects is documented in a public post by an HP
    developer, and a real system needs it, written in our own words and
    code. The download framing should be learned by a black-box serial
    run, not by tracing HP's boot code. **The owner decides** whether that
    run is acceptable, or whether to read his own HPComm sources instead.
  - **casrelease is off limits.** Parisse's package that rebuilds the
    49G ROM 1.24 contains HP's ROM as object files. It is HP code, so it
    is not to be opened.
- **Bricking a real 49G.**
  - **Low, but not zero.** The boot sector is hardware write-protected
    (a strap on the board), and a bad system image lands in "No System".
    The documented recovery is the reset hole plus + ENTER ON. The whole
    comp.sys.hp48 archive and the HP Museum forum report no 49G bricked
    by a bad system image. The units that died had a damaged boot sector
    (HP's developers, protection lifted) or a damaged FS bank.
  - **What does carry risk:**
    - **Bank 0's second half (FS).** It can take over from the boot
      sector, so a custom download must never write or erase it.
    - A custom system that leaves the flash in a command state or
      programs the wrong bank. The reset hole should clear that.
    - A boot code that rejects the image, which is recoverable.
    - Flashing tools that need a Windows 95-2000 era Connectivity Kit.
  - **Before any real flash:** read back and keep the owner's 49G system
    banks (port 2 and the system) with HP's own tools.
  - **A zero-risk path to real hardware:** load the demo as a takeover
    code object under HP's OS via satx (ShellOS and the games work this
    way). It needs no flashing and also works on the 48SX and GX.
- **Value risk.** A demo ROM is a test fixture and a showcase, not a
  calculator. Do not let it grow into an OS: newRPL took years on far
  better hardware.
