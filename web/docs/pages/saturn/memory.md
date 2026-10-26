---
title: Memory
description: How the Saturn reaches ROM, RAM, the hardware registers and the card ports: the memory controllers, their configuration at start-up, the maps of each model and the bank switching of the 48GX and the 49G.
section: saturn
order: 2
---

The Saturn addresses 1 M nibbles, 512 KB. Nothing in that space is fixed: at start-up the ROM tells each memory controller where its chip goes and how big it is. The 48G series and the 49G have more memory than fits, so they switch banks.

## The bus and the controllers

The CPU sends a 20-bit nibble address and reads or writes one nibble at a time. Which chip answers is decided by six memory controllers, inside the Clarke chip of the 48SX and the Yorke chip of the G series and the 49G. Each controller drives the chip-select line of one device:

| Controller | 48SX | 48G, 48GX | 49G |
|---|---|---|---|
| NCE1 | ROM | ROM | flash |
| HDW | the hardware registers | the hardware registers | the hardware registers |
| NCE2 | RAM | RAM | RAM, 256 KB |
| CE1 | card port 1 | the bank switcher | the bank switcher |
| CE2 | card port 2 | card port 1 | RAM, 128 KB |
| NCE3 | not used | card port 2 | RAM, 128 KB, and the flash's write path |

The ROM controller needs no configuration and answers any address no other device claims. The hardware registers are a block of 64 nibbles, which the ROM places at address 00100 on every model.

## Configuring the controllers

Four instructions talk to the controllers:

- **RESET** unconfigures all of them except the ROM's.
- **CONFIG** configures the next controller that is not yet configured, with the value in register C. Most controllers take two CONFIGs: first the size, then the base address. The hardware registers take only the address.
- **UNCNFG** unconfigures the controller of the device at the address in C.
- **C=ID** reads which controller comes next and whether it expects a size or an address.

The controllers form a daisy chain: a CONFIG goes to the first one in the chain that is not fully configured. The order of the chain is HDW, NCE2, CE1, CE2, NCE3, and the ROM last. Every controller must be configured, whether its device is fitted or not; the ROM parks the unused ones at a small window of 2 KB.

A size is a mask, not a length. Each controller only sees the top eight address lines, so sizes and addresses come in steps of 2 KB, and a window always starts on a multiple of its size. A chip that is smaller than its window repeats in it: a 32 KB card declared as 128 KB shows up four times.

When two windows overlap, one device wins. The hardware registers come first, then the RAM, the cards and the ROM last. Sources disagree on whether CE1 or CE2 wins when those two overlap; the stock ROMs never let them overlap.

It also means that a program can move memory around: copy RAM to address 0 and map it there, and the CPU runs its own interrupt handler instead of the ROM's. A few programs do this, so an emulator cannot hard-wire the map.

## The maps

What the ROM sets up after a start, by model. Addresses here are nibble addresses in hexadecimal, as HP's documents write them; sizes are in bytes.

| Model | ROM | RAM | Card ports |
|---|---|---|---|
| 48SX | 256 KB from 00000, its top 32 KB covered by RAM | 32 KB at 70000 | slot 1 at 80000, slot 2 at C0000 |
| 48G | 512 KB, the whole space, where nothing covers it | 32 KB at 80000 | none |
| 48GX | 512 KB, as the 48G | 128 KB at 80000 | slot 1 at C0000, slot 2 behind it in banks of 128 KB |
| 38G | 512 KB, as the 48G | 32 KB at F0000 | none |
| 39G, 40G | 1 MB, in banks | 256 KB at 80000 | none |
| 49G | 2 MB of flash, in banks | 256 KB at 80000, 256 KB more as port 1 | none |

The hardware registers sit at 00100 to 0013F on all of them, over the ROM. The 42S and the other Lewis models have a fixed map without controllers.

## Bank switching on the 48GX

The G series has 512 KB of ROM, as much as the whole address space. HP solved it with one more address line and a bank switcher.

- The ROM's top address line shares a pin of the Yorke chip with NCE3, the select line of card port 2. A bit in the hardware registers chooses which one the pin carries. So the upper half of the ROM and port 2 cannot be seen at the same time.
- CE1 drives an external latch, a 74HC174. Reading any address in CE1's window stores the low address bits in the latch. Those bits become the upper address lines of port 2 and a bank-enable bit, so a read at the right address picks one of 32 banks of 128 KB, up to 4 MB.
- [Christoph Gießelink found](https://www.hpcalc.org/details/1693) that a read of the latch reads one nibble too many, which latches the next bank. Data still comes back as written, since reads and writes are off by the same amount. But the last bank of a full 4 MB card latches an address with the enable bit clear, which is why a full 4 MB card always gives "Invalid Card Data": the firmware does not work around it.
- On the 48SX, a card's write-protect switch blocks writes in the Clarke chip. On the GX, slot 2's write-protect line goes to the bank switcher instead.

Since every access to the upper ROM or to port 2 may need a bank switch first, the G series runs more slowly than its clock suggests (see [The CPU](cpu.md)). The [48GX page](../48gx.md) tells how the ports appear to the user.

## Flash banking on the 49G

The 49G has 2 MB of flash, an Intel 28F160S5, and 512 KB of RAM. It uses the same latch as the 48GX to choose flash banks of 128 KB, with two windows:

- 00000 to 3FFFF shows one of banks 0 to 3.
- 40000 to 7FFFF shows any of the 16 banks, or half of the extra RAM when the system needs it.
- 80000 to FFFFF holds the first 256 KB of RAM: HOME and port 0.

The pin the 48GX shares between ROM and port 2 is always NCE3 on the 49G, so no device can be bigger than 256 KB. CE2 and NCE3 each select 128 KB of the second 256 KB of RAM, which the 49G calls port 1. Unlike the 48GX, the latch can also be written, and the 49G configures CE1 only while it switches a bank.

The flash holds the operating system and port 2, the user's flash memory. The first 64 KB of bank 0 are a boot sector, write-protected in hardware. To write the flash, the system maps NCE3 over the flash window and sets the bit that drives the infrared LED on the 48, which the 49G, having no infrared, uses as the flash's write enable. Then it programs the chip with Intel's command set. Because the system is in flash, the 49G's ROM can be updated.

The 39G and 40G come from the 49G's design, with 1 MB of ROM and 256 KB of RAM.

## Sources

- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin, chapters 66 and 67 by Christoph Gießelink: the six controllers, the chain, the priority, sizes as masks, the default maps, the 48GX's bank switcher and its extra nibble, the 49G's controllers, banks and flash writes.
- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): RESET, CONFIG, UNCNFG and C=ID, the 48's devices, programs that move RAM to address 0.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the managers of the 48G, the parking window, repeated small modules, the bank switcher's window.
- [Guide to the HP48G/GX Hardware, by Teuwen (hphard.zip)](https://www.hpcalc.org/hp48/docs/misc/hphard.zip): the 74HC174 latch and which address bits it stores.
- [HP 49G memory map, by Smith (memmap.zip)](https://www.hpcalc.org/hp49/docs/programming/memmap.zip): the 49G's flash and RAM, the boot sector, the map.
- [HP49 Memory Explained, by Sousa (memory49.zip)](https://www.hpcalc.org/hp49/docs/programming/memory49.zip): the 49G's two bank windows.
- [Intel 28F160S5 datasheet](https://web.archive.org/web/20000819001922id_/http://developer.intel.com:80/design/flcomp/datashts/29060904.pdf): the flash chip's blocks and commands.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the 49G's memory and its upgradable ROM.
