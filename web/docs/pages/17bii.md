---
title: HP 17BII
description: HP's business calculator of 1990 with RPN or algebraic entry, its history, what makes it special and how saturnus runs it.
section: models
model: 17bii
---

The HP 17BII is a business and financial calculator that Hewlett-Packard introduced in January 1990. It replaced the [HP 17B](17b.md) and kept its menus, applications and solver, but lets you choose between algebraic entry and [RPN](https://en.wikipedia.org/wiki/Reverse_Polish_notation). With the [HP 19BII](19bii.md) of the same month, it was the first HP calculator to offer both.

![HP 17BII](../img/17bii-640.webp)
The 17BII as saturnus draws it, with its main menu.

## At a glance

| | |
|---|---|
| Released | January 1990, at $110 |
| Discontinued | Before 2003; the 17bII+ followed in September 2003 |
| Code name | Trader II |
| Product number | F1638A |
| Processor | Saturn core in HP's Lewis chip, about 1 MHz |
| Memory | 8 KB RAM (6.5 KB for you), 64 KB ROM |
| Display | Two lines of 22 characters, 131 × 16 pixels |
| Keyboard | 37 keys, one shift key, six menu keys |
| Entry | Algebraic in chain order, or RPN |
| Input and output | Infrared output to HP's 82240 printer, beeper |
| Clock | Time, date and appointments |
| Power | Three button cells |

## History

### The 17B with a choice

HP launched the 17B in January 1988 as part of its Pioneer series: slim calculators built around Lewis, a custom chip with an improved Saturn processor core. The 17B worked in algebraic chain order only. Two years later, in January 1990, the 17BII took its place at the same price, $110.

[The new model's manual still opens](https://literature.hpcalc.org/community/hp17bii-om-en.pdf) with the 17B's welcome page and adds one point: "You can choose either ALG (Algebraic) or RPN (Reverse Polish Notation) entry logic for your calculations." [Finseth's data lists](https://www.finseth.com/hpdata/hp17bii.php) the 17BII and the 19BII as the first dual-logic HP calculators. Their predecessors, the [18C](https://en.wikipedia.org/wiki/HP-18C) Business Consultant of 1986 and the 17B and 19B of 1988, were algebraic only.

[The manual's RPN appendix explains](https://literature.hpcalc.org/community/hp17bii-om-en.pdf) the name: Polish notation comes from the Polish logician [Jan Łukasiewicz](https://en.wikipedia.org/wiki/Jan_%C5%81ukasiewicz) (1878–1956), who put operators before the numbers; HP puts them after.

### Early bugs

[Finseth's data keeps](https://www.finseth.com/hpdata/hp17bii.php) two RPN oddities. [Wlodek Mier-Jedrzejowicz found](https://www.finseth.com/hpdata/hp17bii.php) that the stack is not always four levels: it grows from one to four as you enter numbers. Type 1, ENTER, 2, then roll down twice, and you see 2, not 0.

The second is a real bug in early 17BII and 19BII calculators. In RPN mode, change the sign of a number with +/−, swap with x≷y, and type a digit: a minus sign appears in front of it that is not really there. Backspacing over that minus can freeze the calculator or reset it with "Machine Reset". The advice was to press CLR at once. HP removed the bug early in 1991.

### The 17bII+

The 17BII was sold until the early 2000s. Its successor, the 17bII+, came out on 23 September 2003 at $120. It was made by Kinpo; a redesigned version of 2007 uses a Sunplus processor. [Finseth notes](https://www.finseth.com/hpdata/hp17biiplus.php) that it lacks the 17BII's hidden solver functions L() and G().

## What makes it special

### ALG and RPN

The MODES menu switches between ALG and RPN, and the setting stays until you change it. In RPN mode the = key becomes ENTER: 2 ENTER 3 + gives 5. In ALG mode the 17BII calculates as the 17B did, in the order you type: 2 + 3 = gives 5, and 2 + 3 × 4 = gives 20, not 14.

The solver uses algebraic equations in both modes.

### Menus and applications

The bottom line of the display shows a menu, usually the main menu with FIN, BUS, SUM, TIME and SOLVE. Menus that calculate work the same way throughout: key in a number and press a variable's menu key to store it, or press the key alone to calculate it.

- Time value of money for loans, savings, leasing and amortization.
- Interest conversions between nominal and effective rates.
- Cash flows: net present value and internal rate of return.
- Bonds: price or yield on any date.
- Depreciation: straight line, declining balance, sum of the years' digits and ACRS.
- Business percentages: percent change, percent of total, markup.
- Statistics: mean, correlation, linear estimates.
- The clock: time, date and appointments.

### The solver and its hidden functions

For anything not built in, type an equation and solve it for any of its variables. "It's easier than programming!", [the manual says](https://literature.hpcalc.org/community/hp17bii-om-en.pdf). [Finseth's data adds](https://www.finseth.com/hpdata/hp17bii.php) that the 17BII's solver is the same as the 19BII's and has all its functions, documented or not, among them Let and Get.

### Printing

The infrared port prints to HP's 82240 printer. [Finseth notes](https://www.finseth.com/hpdata/hp17bii.php) that the 17BII added a double-spaced printing mode, set in the MODES menu.

## In saturnus

saturnus runs the international 17BII, main ROM revision B with language ROM revision A:

- On a first start it asks for the language. It boots, computes, and the ROM checks of its self-test pass. Its ROM is 96 KB: the 64 KB main ROM and a 32 KB language ROM. The English-only 64 KB ROM has not been tested.
- Its clock runs with emulated time. "Set the clock" sets it to your computer's time, by pressing the keys of its TIME menu as you would; not while it asks for the language. Appointment alarms have not been tested.
- On your computer's keyboard, F1 to F6 press the menu keys and Escape is CLR.
- The CPU runs at 1 MHz, the figure the sources give for its chip. No timing of a real 17BII has been measured to check it against, so run times are an estimate. 2×, 4× and Max speed it up.
- Save and load states work as on every model.

What it does not do: the 17BII has no serial port, so Files has nothing for it, and the infrared printer output is not emulated. The memory view and the RPL editor are for the HP 48 and 49 only.

The [command reference](commands.md#17bii) lists the 17BII's 260 functions with the keys that run them, and examples for 58 of them, run in saturnus.

[Open the HP 17BII in saturnus](../app/#/calculator/17bii)

## Sources

- [HP 17BII Owner's Manual](https://literature.hpcalc.org/community/hp17bii-om-en.pdf): the welcome page and the choice of ALG or RPN, the applications, the solver, 6.5 KB of memory, the 82240 printer, the RPN appendix (the mode stays until changed, the solver is algebraic, Łukasiewicz), the edition dates.
- [Finseth's HP calculator data: HP 17BII](https://www.finseth.com/hpdata/hp17bii.php): date and price, the first dual-logic calculators, the RPN stack oddity, the +/− bug and its fix, the solver shared with the 19BII, double-spaced printing.
- [Finseth's HP calculator data: HP 17B](https://www.finseth.com/hpdata/hp17b.php): the 17B's code name, processor, power, display, RAM and ROM.
- [Finseth's HP calculator data: HP 17bII+](https://www.finseth.com/hpdata/hp17biiplus.php): the 17bII+'s date, price, maker and missing L and G.
- [Finseth's HP calculator data: HP 18C](https://www.finseth.com/hpdata/hp18c.php) and [HP 19B](https://www.finseth.com/hpdata/hp19b.php): their entry logic and years.
- [Wikipedia: HP-17B](https://en.wikipedia.org/wiki/HP-17B): the 17BII's code name, product number and date, the 17bII+ of 2007 and its Sunplus processor.
- [Wikipedia: HP-28 series](https://en.wikipedia.org/wiki/HP-28_series): Lewis as a custom chip with an improved Saturn core.
- [Wikipedia: HP-27S](https://en.wikipedia.org/wiki/HP-27S): the keyboard of the Pioneer series.
