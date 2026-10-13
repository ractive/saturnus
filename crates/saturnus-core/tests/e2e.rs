//! End-to-end tests against real ROM images, gated by `SATURNUS_ROM_DIR`
//! (see the test policy). Without the variable every test is skipped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use saturnus_core::ADDR_MASK;
use saturnus_core::internals::{self, Chip, Select, decode};
use saturnus_core::io::Key;
use saturnus_core::{Halt, Machine, Model};

/// The ROM image `name` from `$SATURNUS_ROM_DIR`, or `None` when the
/// variable is unset.
fn rom(name: &str) -> Option<Vec<u8>> {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping ROM test ({name})");
        return None;
    };
    let path = std::path::Path::new(&dir).join(name);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read ROM {}: {e}", path.display()));
    Some(bytes)
}

/// Disassembly of the instruction at `pc` through the current mapping.
fn disasm_at(m: &Machine, pc: u32) -> String {
    let d = decode(|a| m.peek(a & ADDR_MASK), pc);
    internals::disassemble(&d.instr)
}

/// Run `m` for `cycles`, panicking with the disassembly on a halt.
fn run(m: &mut Machine, cycles: u64) {
    if let Err(h) = m.run_cycles(cycles) {
        let Halt::InvalidOpcode { pc, .. } = h;
        panic!(
            "halt after {} cycles: {h}\npc #{pc:05X}: {}",
            m.cycles(),
            disasm_at(m, pc)
        );
    }
}

/// Path of the golden screen `name` (131x64 text, see `Lcd::to_text`).
fn golden_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"))
}

/// Run in 1M-cycle slices until the LCD equals the golden screen `name` or
/// `limit` cycles have passed, then assert it. With `SATURNUS_BLESS` set,
/// run the full `limit` and (re)write the golden file instead.
fn run_until_screen(m: &mut Machine, name: &str, limit: u64) {
    const SLICE: u64 = 1_000_000;
    let path = golden_path(name);
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        run(m, limit);
        std::fs::write(&path, m.lcd().to_text()).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read golden screen {}: {e}", path.display()));
    let end = m.cycles() + limit;
    while m.cycles() < end && m.lcd().to_text() != want {
        run(m, SLICE);
    }
    let got = m.lcd().to_text();
    assert!(
        got == want,
        "screen {name} not reached after {limit} cycles (pc #{:05X}); got:\n{got}",
        m.cpu.regs.pc
    );
}

/// Press `key` long enough for the ROM's debounce (> 10 ms), release it.
fn tap(m: &mut Machine, key: Key) {
    m.key_down(key).unwrap();
    run(m, 400_000);
    m.key_up(key).unwrap();
}

/// Cold boot of `model` with zeroed RAM: the ROM finds no valid memory
/// and asks "Try To Recover Memory?"; NO (the sixth menu key, F) clears
/// memory and shows "Memory Clear" over the empty stack (on the 49G a box
/// that OK, F again, closes: golden `49g-stack`). The golden
/// screens `<model>-try-to-recover-memory` and `<model>-memory-clear` are
/// RAM dumps of the display area through `Lcd`. Cycle limits scale with
/// the clock (the GX runs at twice the SX's rate).
fn boot_to_stack_model(model: Model, rom: &[u8]) -> Machine {
    let mut m = Machine::new(model, rom).unwrap();
    let scale = u64::from(model.clock_hz() / 2_000_000);
    let name = model.name();
    run_until_screen(
        &mut m,
        &format!("{name}-try-to-recover-memory"),
        40_000_000 * scale,
    );
    tap(&mut m, Key::F);
    run_until_screen(&mut m, &format!("{name}-memory-clear"), 20_000_000 * scale);
    if model == Model::Hp49g {
        // The 49G's "Memory Clear" is a box with an OK softkey (F); OK
        // gives the empty stack in algebraic mode.
        tap(&mut m, Key::F);
        run_until_screen(&mut m, &format!("{name}-stack"), 20_000_000 * scale);
    }
    m
}

/// Run `keys` one instruction at a time and check the display at every
/// instant at which any row of the main or menu area does not decode to
/// the built-in RAM (the ROMs unconfigure and resize it while busy: SX
/// ROM J at #0C0B2, GX ROM R at #72386): the whole `lcd()` must show the
/// bitmaps in RAM, never the ROM data the mapping now answers with.
/// Returns how many such instants were seen. Assumes the RAM window's
/// base, taken at the start, stays the same through `keys` (the ROMs
/// only change its size).
fn check_display_through_remaps(m: &mut Machine, keys: &[Key]) -> u64 {
    let (base, _) = m.hw.mc.window(Chip::Nce2).unwrap();
    let in_ram = |m: &Machine, a: u32| {
        matches!(
            m.hw.mc.select(a),
            Select::Chip {
                chip: Chip::Nce2,
                ..
            }
        )
    };
    let mut gaps = 0;
    for &k in keys {
        for (down, cycles) in [(true, 400_000), (false, 1_500_000)] {
            if down {
                m.key_down(k).unwrap();
            } else {
                m.key_up(k).unwrap();
            }
            let end = m.cycles() + cycles;
            while m.cycles() < end {
                m.step().unwrap();
                if !m.display_on()
                    || internals::lcd_row_spans(m).all(|(a, b)| in_ram(m, a) && in_ram(m, b))
                {
                    continue;
                }
                gaps += 1;
                let want = internals::render_lcd(m, |a| m.hw.ram.read(a.wrapping_sub(base)));
                assert!(
                    m.lcd() == want,
                    "noise frame at pc #{:05X}, cycle {}:\n{}",
                    m.cpu.regs.pc,
                    m.cycles(),
                    m.lcd().to_text()
                );
            }
        }
    }
    gaps
}

/// The owner's report: on the 48SX the whole LCD flashed to noise for a
/// moment while the hourglass was on. Arithmetic and the PLOT menu after
/// the boot pass through the ROM's RAM resizing with the display on.
#[test]
fn hp48sx_display_holds_through_ram_remap() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    let mut m = boot_to_stack_model(Model::Hp48sx, &rom);
    use Key::*;
    let keys = [One, Three, Enter, Four, Power, LeftShift, Eight];
    let gaps = check_display_through_remaps(&mut m, &keys);
    assert!(gaps > 0, "the sequence no longer reaches a RAM remap");
}

/// The 48GX's ROM R resizes its RAM the same way while it clears memory
/// after NO at "Try To Recover Memory?".
#[test]
fn hp48gx_display_holds_through_ram_remap() {
    let Some(rom) = rom("gxrom-r") else {
        return;
    };
    let mut m = Machine::new(Model::Hp48gx, &rom).unwrap();
    run_until_screen(&mut m, "48gx-try-to-recover-memory", 80_000_000);
    use Key::*;
    let gaps = check_display_through_remaps(&mut m, &[F, One, Three, Enter]);
    assert!(gaps > 0, "the sequence no longer reaches a RAM remap");
}

#[test]
fn hp48sx_boot_to_memory_prompt() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    boot_to_stack_model(Model::Hp48sx, &rom);
}

/// The 48GX ROM R boots the same way as the SX's ROM J: "Try To Recover
/// Memory?", NO, "Memory Clear" over the empty stack.
#[test]
fn hp48gx_boot_to_memory_prompt() {
    let Some(rom) = rom("gxrom-r") else {
        return;
    };
    let m = boot_to_stack_model(Model::Hp48gx, &rom);
    // The OS records the RAM base nibble in #11F: 8 on the G/GX (wiki:
    // hardware/io-ram "Base nibble"), and leaves upper ROM selected.
    assert_eq!(m.peek(0x11F), 8);
}

/// The 49G ROM 2.15 (the saturnng oracle's image): "Try To Recover
/// Memory?", NO, the "Memory Clear" box, OK, the empty stack. The golden
/// screens match saturnng pixel for pixel (scenario 49g-boot).
#[test]
fn hp49g_boot_to_stack() {
    let Some(rom) = rom("rom.49g") else {
        return;
    };
    let m = boot_to_stack_model(Model::Hp49g, &rom);
    // The OS left a flash bank switched in: the latch survives SHUTDN on
    // the 49G (wiki: hardware/hp49g).
    eprintln!(
        "49G at the stack: latch {:#04X}, #11F = {:X}",
        m.hw.bank_latch(),
        m.peek(0x11F)
    );
    assert!(!m.hw.nce1.flash().is_some_and(|f| f.write_enabled()));
}

/// Port 2 of the 49G is user flash. `42 STO :2:A` makes the ROM program
/// the chip (write-to-buffer commands through NCE3 with #11C bit 3 set),
/// and `RCL(:2:A)` reads 42 back (scenario 49g-port2 matches saturnng).
/// Only bank 8 onwards (port 2) may change, and only 1 bits may clear.
#[test]
fn hp49g_store_to_port2_programs_flash() {
    let Some(rom) = rom("rom.49g") else {
        return;
    };
    let mut m = boot_to_stack_model(Model::Hp49g, &rom);
    let before = m.hw.nce1.flash().unwrap().to_packed();
    let tag = [
        Key::LeftShift,
        Key::Point,
        Key::Two,
        Key::Right,
        Key::Alpha,
        Key::A,
    ];
    let mut keys = vec![Key::Four, Key::Two, Key::Sto];
    keys.extend(tag);
    keys.push(Key::Enter);
    for key in keys {
        tap(&mut m, key);
        run(&mut m, 4_000_000);
    }
    run(&mut m, 8_000_000);
    let flash = m.hw.nce1.flash().unwrap();
    assert!(!flash.write_enabled(), "the ROM closes the write gate");
    let after = flash.to_packed();
    let changed: Vec<usize> = (0..after.len())
        .filter(|&i| after[i] != before[i])
        .collect();
    assert!(!changed.is_empty(), "nothing was programmed");
    for &i in &changed {
        assert!(i >= 8 * 0x2_0000, "byte {i:#X} outside port 2 changed");
        assert_eq!(after[i] & !before[i], 0, "byte {i:#X} gained 1 bits");
    }
    eprintln!(
        "port 2: {} bytes programmed from {:#X}",
        changed.len(),
        changed[0]
    );
}

/// The 38G ROM A1.67 has no oracle, so the acceptance is boot and key
/// input against golden screens recorded from saturnus: a cold boot shows
/// a "Memory Clear" box, OK (softkey F) gives HOME, and the algebraic
/// entry `6 * 7 ENTER` puts `6*7` and 42 in the history.
#[test]
fn hp38g_boot_to_home_and_compute() {
    let Some(rom) = rom("38G_A167.ROM") else {
        return;
    };
    let mut m = Machine::new(Model::Hp38g, &rom).unwrap();
    run_until_screen(&mut m, "38g-memory-clear", 80_000_000);
    tap(&mut m, Key::F);
    run_until_screen(&mut m, "38g-home", 40_000_000);
    let fb = m.framebuffer();
    eprintln!(
        "38G at HOME: #11F = {:X}, contrast {}, DA19 {}",
        m.peek(0x11F),
        fb.contrast,
        m.hw.io.da19()
    );
    // The ROM puts its 32 KB of RAM on NCE2 at #F0000 (wiki: hardware/hp38g).
    assert_eq!(m.hw.mc.window(Chip::Nce2), Some((0xF0000, 0xF0000)));
    for key in [Key::Six, Key::Multiply, Key::Seven, Key::Enter] {
        tap(&mut m, key);
        run(&mut m, 800_000);
    }
    run_until_screen(&mut m, "38g-six-times-seven", 40_000_000);
    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp38g, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
}

/// Hold every key of `chord` at once (ON first, as a user would), long
/// enough for the ROM to see the chord, then release them.
fn chord(m: &mut Machine, chord: &[Key]) {
    for &k in chord {
        m.key_down(k).unwrap();
        run(m, 40_000);
    }
    run(m, 400_000);
    for &k in chord.iter().rev() {
        m.key_up(k).unwrap();
    }
}

/// Cold boot of the 39G or 40G ROM to HOME: a "Memory Clear" box with an
/// OK softkey (menu key 6), then HOME. There is no oracle; the golden
/// screens are recorded from saturnus.
fn aplet49_boot_to_home(model: Model, rom: &[u8]) -> Machine {
    let mut m = Machine::new(model, rom).unwrap();
    run_until_screen(&mut m, "39g-memory-clear", 80_000_000);
    tap(&mut m, Key::F);
    run_until_screen(&mut m, &format!("{}-home", model.name()), 40_000_000);
    m
}

/// The 39G/40G ROM (`rom.39g`, unpacked) has no oracle, so the acceptance
/// is boot, key input and the user's guide reset chords against golden
/// screens recorded from saturnus: the cold boot shows a "Memory Clear"
/// box, OK gives HOME, `6 * 7 ENTER` puts `6*7` and 42 in the history,
/// ON + menu key 3 resets to HOME, and ON + menu keys 1 and 6 clears
/// memory, back to the "Memory Clear" box (wiki: hardware/hp39g-40g
/// "First boot").
#[test]
fn hp39g_boot_compute_and_reset_chords() {
    let Some(rom) = rom("rom.39g") else {
        return;
    };
    let mut m = aplet49_boot_to_home(Model::Hp39g, &rom);
    // The ROM configures only HDW and NCE2, its 256 KB of RAM at #80000;
    // CE1 (the bank latch) is configured only around each bank switch,
    // and CE2 and NCE3 are left alone (wiki: hardware/hp39g-40g).
    assert_eq!(m.hw.mc.window(Chip::Nce2), Some((0x80000, 0x80000)));
    for chip in [Chip::Ce2, Chip::Nce3] {
        assert!(!m.hw.mc.is_configured(chip), "{chip:?} configured");
    }
    let fb = m.framebuffer();
    eprintln!(
        "39G at HOME: latch {:#04X}, #11F = {:X}, contrast {}",
        m.hw.bank_latch(),
        m.peek(0x11F),
        fb.contrast
    );
    assert!(Model::Hp39g.contrast_range().contains(&fb.contrast));
    for key in [Key::Six, Key::Multiply, Key::Seven, Key::Enter] {
        tap(&mut m, key);
        run(&mut m, 800_000);
    }
    run_until_screen(&mut m, "39g-six-times-seven", 40_000_000);

    // State round trip into a freshly built machine.
    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp39g, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
    run(&mut m, 1_000_000);
    run(&mut fresh, 1_000_000);
    assert_eq!(fresh.save_state(), m.save_state());

    // ON + menu key 3: reset, back at HOME.
    chord(&mut m, &[Key::On, Key::C]);
    run_until_screen(&mut m, "39g-warm-reset", 40_000_000);
    // ON + menu keys 1 and 6: memory clear.
    chord(&mut m, &[Key::On, Key::A, Key::F]);
    run_until_screen(&mut m, "39g-memory-clear", 80_000_000);
}

/// The 40G runs the 39G ROM. The ROM reads #11A bit 3 (on the 39G the IR
/// receive sample) and treats a set bit as a 40G: HOME then shows a CAS
/// label on menu key 6 (wiki: questions/hp39g-40g-model-detection).
#[test]
fn hp40g_boot_shows_the_cas() {
    let Some(rom) = rom("rom.39g") else {
        return;
    };
    let mut m = aplet49_boot_to_home(Model::Hp40g, &rom);
    assert_eq!(m.peek(0x11A) & 0x8, 0x8, "the 40G strap");
    let home_39 = std::fs::read_to_string(golden_path("39g-home")).unwrap();
    assert_ne!(m.lcd().to_text(), home_39, "40G HOME looks like the 39G's");
    for key in [Key::Six, Key::Multiply, Key::Seven, Key::Enter] {
        tap(&mut m, key);
        run(&mut m, 800_000);
    }
    run_until_screen(&mut m, "40g-six-times-seven", 40_000_000);
}

#[test]
fn hp48sx_state_round_trip() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    state_round_trip(Model::Hp48sx, &rom);
}

#[test]
fn hp49g_state_round_trip() {
    let Some(rom) = rom("rom.49g") else {
        return;
    };
    state_round_trip(Model::Hp49g, &rom);
}

#[test]
fn hp48gx_state_round_trip() {
    let Some(rom) = rom("gxrom-r") else {
        return;
    };
    state_round_trip(Model::Hp48gx, &rom);
}

/// Save/load round trip on a booted ROM: boot to "Memory Clear", save, run
/// 1 M cycles, load, run the same 1 M cycles again; screen, registers and
/// the whole state must match. Also checks the framebuffer extras the ROM
/// sets up (contrast, annunciators).
fn state_round_trip(model: Model, rom: &[u8]) {
    let mut m = boot_to_stack_model(model, rom);
    let fb = m.framebuffer();
    eprintln!(
        "after boot: {} cycles, contrast {}, annunciators {}",
        m.cycles(),
        fb.contrast,
        fb.annunciator_line()
    );
    assert!(
        model.contrast_range().contains(&fb.contrast),
        "ROM contrast {} outside the keyboard range",
        fb.contrast
    );

    let saved = m.save_state();
    // Something to do in the next million cycles: a key press.
    m.key_down(Key::Seven).unwrap();
    run(&mut m, 1_000_000);
    let screen = m.lcd().to_text();
    let regs = m.cpu.regs.clone();
    let after = m.save_state();

    m.load_state(&saved).unwrap();
    assert_eq!(m.save_state(), saved);
    m.key_down(Key::Seven).unwrap();
    run(&mut m, 1_000_000);
    assert_eq!(m.lcd().to_text(), screen);
    assert_eq!(m.cpu.regs, regs);
    assert_eq!(m.save_state(), after);

    // The state also loads into a freshly built machine.
    let mut fresh = Machine::new(model, rom).unwrap();
    fresh.load_state(&after).unwrap();
    assert_eq!(fresh.lcd().to_text(), screen);
    assert_eq!(fresh.save_state(), after);
}

/// Boot ROM J to the empty stack ("Memory Clear").
fn boot_to_stack(rom: &[u8]) -> Machine {
    boot_to_stack_model(Model::Hp48sx, rom)
}

/// Run `cycles`, then on until the CPU is idle in SHUTDN, so the screen
/// is not caught half redrawn.
fn run_to_idle(m: &mut Machine, cycles: u64) {
    run(m, cycles);
    let cap = m.cycles() + 2_000_000;
    while !m.is_shutdown() && m.cycles() < cap {
        if let Err(h) = m.step() {
            panic!("halt: {h}");
        }
    }
    assert!(m.is_shutdown(), "CPU not idle at pc #{:05X}", m.cpu.regs.pc);
}

/// Glyphs of the status line's right half (columns 64-130, the rows above
/// the separator line), split at blank columns; each glyph is its column
/// bit patterns. On ROM J with flag -40 set this ends in the clock,
/// `HH:MM:SSA`.
fn status_glyphs(m: &Machine) -> Vec<Vec<u16>> {
    let text = m.lcd().to_text();
    let rows: Vec<&[u8]> = text.lines().map(str::as_bytes).collect();
    let sep = rows
        .iter()
        .position(|r| r.iter().all(|&c| c == b'#'))
        .expect("status separator line");
    let mut glyphs = Vec::new();
    let mut cur: Vec<u16> = Vec::new();
    let width = rows[0].len();
    for x in 64..width {
        let col = rows[..sep]
            .iter()
            .enumerate()
            .fold(0u16, |acc, (y, row)| acc | (u16::from(row[x] == b'#') << y));
        if col == 0 {
            if !cur.is_empty() {
                glyphs.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(col);
        }
    }
    if !cur.is_empty() {
        glyphs.push(cur);
    }
    glyphs
}

/// Digit glyphs learned from the running clock: the seconds digit steps
/// through 0-9, and it shows 0 whenever the tens digit changes. Returns
/// the glyphs for 0-9 in order. Runs about 25 emulated seconds.
fn learn_digits(m: &mut Machine) -> Vec<Vec<u16>> {
    const QUARTER_SECOND: u64 = 500_000;
    let mut order: Vec<Vec<u16>> = Vec::new();
    let mut zero_at = None;
    let mut last_tens: Option<Vec<u16>> = None;
    for _ in 0..100 {
        run_to_idle(m, QUARTER_SECOND);
        let g = status_glyphs(m);
        let n = g.len();
        assert!(n >= 9, "clock not shown: {} status glyphs", n);
        let units = g[n - 2].clone();
        if order.last() != Some(&units) {
            order.push(units);
        }
        let tens = g[n - 3].clone();
        if zero_at.is_none() && last_tens.as_ref().is_some_and(|t| *t != tens) {
            zero_at = Some(order.len() - 1);
        }
        last_tens = Some(tens);
        if let Some(z) = zero_at
            && order.len() >= z + 10
        {
            return order[z..z + 10].to_vec();
        }
    }
    panic!(
        "could not learn the clock digits; saw {} glyphs",
        order.len()
    );
}

/// Minutes and seconds of the clock on the status line, in seconds. The
/// hour is not read: after "Memory Clear" the clock starts at 12:00 and
/// the drift test stays within that hour.
fn shown_seconds(m: &Machine, digits: &[Vec<u16>]) -> u32 {
    let g = status_glyphs(m);
    let n = g.len();
    let digit = |i: usize| {
        let pos = digits.iter().position(|d| *d == g[i]);
        pos.map(|p| p as u32).unwrap_or_else(|| {
            panic!(
                "glyph {i} of the clock is not a digit: {g:?}\n{}",
                m.lcd().to_text()
            )
        })
    };
    // The glyphs end in ... H : M M : S S A.
    let seconds = digit(n - 3) * 10 + digit(n - 2);
    let minutes = digit(n - 6) * 10 + digit(n - 5);
    minutes * 60 + seconds
}

/// Run in 5 ms slices until the shown clock changes; returns the cycle
/// count at the first sample showing the new value, and that value.
fn next_clock_change(m: &mut Machine, digits: &[Vec<u16>]) -> (u64, u32) {
    const SLICE: u64 = 10_000;
    let start = shown_seconds(m, digits);
    for _ in 0..400 {
        run_to_idle(m, SLICE);
        let now = shown_seconds(m, digits);
        if now != start {
            return (m.cycles(), now);
        }
    }
    panic!("clock stuck at {start} s");
}

/// Timer drift against the ROM's own clock (iteration 4): set flag -40 so
/// the status line shows the time, then compare the shown time over ten
/// emulated minutes with the emulated time (`Machine::cycles` at 2 MHz).
/// Both ends are taken at the moment the shown second changes, sampled
/// every 5 ms, so the measurement resolves about 10 ms.
#[test]
fn hp48sx_clock_drift() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    let mut m = boot_to_stack(&rom);
    // -40 SF: 4 0 +/- SPC, alpha alpha S (SIN) F, ENTER.
    for k in [
        Key::Four,
        Key::Zero,
        Key::Neg,
        Key::Space,
        Key::Alpha,
        Key::Alpha,
        Key::Sin,
        Key::F,
        Key::Enter,
    ] {
        tap(&mut m, k);
        run(&mut m, 400_000);
    }
    let digits = learn_digits(&mut m);
    let hz = u64::from(Model::Hp48sx.clock_hz());
    let (c0, t0) = next_clock_change(&mut m, &digits);
    // The ROM turns the calculator off after ten minutes without a key;
    // two taps of left shift (shift on, shift off) every three minutes keep
    // it on without touching the clock.
    let end = c0 + 600 * hz - hz / 2;
    for _ in 0..3 {
        run(&mut m, 180 * hz);
        tap(&mut m, Key::LeftShift);
        run(&mut m, hz / 5);
        tap(&mut m, Key::LeftShift);
    }
    let rest = end - m.cycles();
    run(&mut m, rest);
    let (c1, t1) = next_clock_change(&mut m, &digits);
    let emulated = (c1 - c0) as f64 / hz as f64;
    let shown = f64::from(t1 - t0);
    let drift = shown - emulated;
    eprintln!(
        "clock drift: ROM clock {shown} s over {emulated:.4} s emulated, \
         {:+.1} ms ({:+.1} ppm)",
        drift * 1000.0,
        drift / emulated * 1e6
    );
    assert!(
        drift.abs() < 1.0,
        "ROM clock drifted {drift:.3} s in {emulated:.1} s"
    );
}

/// A Kermit packet with block check type 1 (wiki: protocols/kermit):
/// SOH, LEN, SEQ, TYPE, DATA, CHECK, CR.
fn kermit_packet(seq: u8, kind: u8, data: &[u8]) -> Vec<u8> {
    let tochar = |x: u8| x + 32;
    let len = u8::try_from(data.len() + 3).unwrap();
    let mut p = vec![1, tochar(len), tochar(seq), kind];
    p.extend_from_slice(data);
    let sum: u32 = p[1..].iter().map(|&b| u32::from(b)).sum();
    p.push(tochar(((sum + ((sum & 192) / 64)) & 63) as u8));
    p.push(b'\r');
    p
}

/// Push `packet` and collect the reply up to its CR (at most 4 s; `k` is
/// the clock in units of 2 MHz).
fn kermit_exchange(m: &mut Machine, packet: &[u8], k: u64) -> Vec<u8> {
    m.serial_push(packet);
    let mut reply = Vec::new();
    for _ in 0..40 {
        run(m, 200_000 * k);
        reply.extend(m.serial_drain());
        if reply.ends_with(b"\r") {
            break;
        }
    }
    assert_eq!(m.serial_pending(), 0);
    reply
}

/// ROM J's own Kermit server over the emulated wire: start SERVER from
/// the keyboard, then at the default 9600 baud send a server init ("I")
/// and a one-packet ASCII file `A` containing `42`; every packet must be
/// acknowledged ("Y"), and `A` must then evaluate to 42 on the stack.
#[test]
fn hp48sx_kermit_server_receives_a_file() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    kermit_server_receives_a_file(Model::Hp48sx, &rom);
}

/// The same Kermit exchange with the 48GX ROM R's server.
#[test]
fn hp48gx_kermit_server_receives_a_file() {
    let Some(rom) = rom("gxrom-r") else {
        return;
    };
    kermit_server_receives_a_file(Model::Hp48gx, &rom);
}

/// The same Kermit exchange with the 49G ROM 2.15's server. Its letters
/// sit on other keys (S = SIN, E = softkey E, R = square root, V = EEX)
/// and it computes in algebraic mode, so `A` is checked by evaluating it
/// in the command line against typing `42`.
#[test]
fn hp49g_kermit_server_receives_a_file() {
    let Some(rom) = rom("rom.49g") else {
        return;
    };
    kermit_server_receives_a_file(Model::Hp49g, &rom);
}

fn kermit_server_receives_a_file(model: Model, rom: &[u8]) {
    // Cycle counts below are for 2 MHz; scale them to the model's clock.
    let k = u64::from(model.clock_hz() / 2_000_000);
    let mut m = boot_to_stack_model(model, rom);
    // SERVER: alpha alpha S E R V E R, ENTER.
    let (r, v) = if model == Model::Hp49g {
        (Key::Sqrt, Key::Eex)
    } else {
        (Key::Right, Key::Sqrt)
    };
    for key in [
        Key::Alpha,
        Key::Alpha,
        Key::Sin,
        Key::E,
        r,
        v,
        Key::E,
        r,
        Key::Enter,
    ] {
        tap(&mut m, key);
        run(&mut m, 400_000 * k);
    }
    run(&mut m, 4_000_000 * k);
    assert_eq!(m.serial_baud(), 9600);
    let packets = [
        (b'I', &b""[..]),
        (b'S', b""),
        (b'F', b"A"),
        (b'D', b"42"),
        (b'Z', b""),
        (b'B', b""),
    ];
    for (seq, (kind, data)) in packets.into_iter().enumerate() {
        // The server init does not advance the sequence.
        let seq = u8::try_from(seq.saturating_sub(1)).unwrap();
        let reply = kermit_exchange(&mut m, &kermit_packet(seq, kind, data), k);
        eprintln!("{} -> {:?}", kind as char, String::from_utf8_lossy(&reply));
        assert!(
            reply.len() >= 6 && reply[0] == 1 && reply[2] == seq + 32 && reply[3] == b'Y',
            "packet {} not acknowledged: {reply:?}",
            kind as char
        );
    }
    // Leave the server (ON is ATTN), then VAR and the first menu key
    // evaluate A (on the 49G the softkey types A into the command line and
    // ENTER evaluates it). The stack must look as if 42 had been typed.
    tap(&mut m, Key::On);
    run(&mut m, 4_000_000 * k);
    tap(&mut m, Key::Var);
    run(&mut m, 2_000_000 * k);
    tap(&mut m, Key::A);
    run(&mut m, 2_000_000 * k);
    if model == Model::Hp49g {
        tap(&mut m, Key::Enter);
        run(&mut m, 2_000_000 * k);
    }
    let mut typed = boot_to_stack_model(model, rom);
    for key in [Key::Four, Key::Two, Key::Enter] {
        tap(&mut typed, key);
        run(&mut typed, 400_000 * k);
    }
    run(&mut typed, 2_000_000 * k);
    // Rows above the menu labels (the bottom 8 rows differ: VAR menu). The
    // 49G's algebraic history also lists SERVER and A, so there only the
    // last result line (the 8 rows above the menu) is compared.
    let skip = if model == Model::Hp49g { 48 } else { 0 };
    let stack = |m: &Machine| {
        m.lcd()
            .to_text()
            .lines()
            .take(56)
            .skip(skip)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(stack(&m), stack(&typed), "A does not hold 42");
}

/// Run until the LCD has not changed for 300 ms while the CPU sits in
/// SHUTDN (at most 10 s), like the CLI's `wait-idle`.
fn wait_idle(m: &mut Machine) {
    let cap = m.cycles() + 20_000_000;
    let mut last = m.lcd();
    let mut since = m.cycles();
    while m.cycles() < cap {
        run(m, 4_000);
        let lcd = m.lcd();
        if lcd != last {
            last = lcd;
            since = m.cycles();
        } else if m.cycles() - since >= 600_000 && m.is_shutdown() {
            return;
        }
    }
}

/// A key-script `press`: hold 60 ms, release, wait for idle.
fn press(m: &mut Machine, key: Key) {
    m.key_down(key).unwrap();
    run(m, 120_000);
    m.key_up(key).unwrap();
    wait_idle(m);
}

/// Regression (iteration 6): after `6 ENTER 7 * ENTER` the 48SX Kermit
/// server went deaf when a packet's first byte arrived about 1-3 ms after
/// its NAK. The start bit's interrupt vectored while ST bit 15 was clear,
/// so ROM J's handler returned at once without RTI; the ROM later
/// re-enabled interrupts with RSI and RTI, but the UART request was
/// still held and gave no new edge, so RBR was never read and every later
/// byte overran (RCS = RBF | RER). RTI now re-enters while USRQ is held.
/// The test sweeps the push time over the first 8 ms after the NAK; each
/// push must get the server's S (send-init) reply, at worst after one NAK
/// for a packet whose first byte overran.
#[test]
fn hp48sx_kermit_server_hears_packets_right_after_a_nak() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    let mut m = boot_to_stack(&rom);
    for key in [Key::Six, Key::Enter, Key::Seven, Key::Multiply, Key::Enter] {
        press(&mut m, key);
    }
    for key in [
        Key::Alpha,
        Key::Alpha,
        Key::Sin,
        Key::E,
        Key::Right,
        Key::Sqrt,
        Key::E,
        Key::Right,
    ] {
        press(&mut m, key);
    }
    m.key_down(Key::Enter).unwrap();
    run(&mut m, 120_000);
    m.key_up(Key::Enter).unwrap();
    // The server NAKs while it waits for a command; stop at the end of the
    // first NAK.
    let mut nak = Vec::new();
    while !nak.ends_with(b"\r") {
        assert!(m.cycles() < 200_000_000, "no NAK from the server");
        run(&mut m, 1_000);
        nak.extend(m.serial_drain());
    }
    assert_eq!(nak.get(3), Some(&b'N'), "{nak:?}");
    let at_nak = m.save_state();
    // A generic-command packet: CD UP DROP, the request satx's `cd ..`
    // sends.
    let packet = b"\x01+ CDUP DROP/\r";
    for half_ms in 0..16u64 {
        m.load_state(&at_nak).unwrap();
        run(&mut m, half_ms * 1_000);
        let mut reply = Vec::new();
        for _attempt in 0..2 {
            m.serial_push(packet);
            run(&mut m, 3_000_000);
            reply = m.serial_drain();
            if reply.get(3) == Some(&b'S') {
                break;
            }
        }
        assert_eq!(
            reply.get(3),
            Some(&b'S'),
            "push {} ms after the NAK: reply {:?}, RCS {:X}",
            half_ms as f64 / 2.0,
            String::from_utf8_lossy(&reply),
            m.peek(0x111)
        );
    }
}

/// The 42S (Lewis chip) from the owner's ROM revision C dump: a cold
/// start shows "Memory Clear" over the X register with no key pressed;
/// 2 ENTER 3 + gives 5. There is no oracle; the golden screens are
/// recorded from saturnus (131x16 text) and match the real calculator's
/// documented behaviour (wiki: hardware/hp42s).
#[test]
fn hp42s_boot_and_add() {
    let Some(rom) = rom("hp42s-c.rom") else {
        return;
    };
    let mut m = Machine::new(Model::Hp42s, &rom).unwrap();
    run_until_screen(&mut m, "42s-memory-clear", 5_000_000);
    assert_eq!(m.lcd().height(), 16);
    let fb = m.framebuffer();
    assert_eq!(
        fb.contrast, 22,
        "the documented reset contrast (wiki: hardware/hp42s)"
    );
    assert_eq!(fb.annunciator_line(), "-");
    for key in [Key::Two, Key::Enter, Key::Three, Key::Plus] {
        tap(&mut m, key);
        run(&mut m, 200_000);
    }
    run_until_screen(&mut m, "42s-two-plus-three", 2_000_000);

    // State round trip into a freshly built machine.
    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp42s, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
    run(&mut m, 500_000);
    run(&mut fresh, 500_000);
    assert_eq!(fresh.save_state(), m.save_state());

    // Shift lights its annunciator.
    tap(&mut m, Key::Shift);
    run(&mut m, 200_000);
    assert!(m.framebuffer().annunciators.left_shift);
}

/// EXIT + LN starts the ROM's self-test (SPD, BEEP, DISP, ROM, DRAM,
/// URAM, then a summary). Its ROM step clears the hardware CRC at #40304,
/// reads #0001C-#1FFFB through it and expects #FFFF, then reads the whole
/// ROM; with the Lewis CRC rule (wiki: hardware/lewis "CRC") it passes:
/// "ROM OK".
#[test]
fn hp42s_self_test_rom_crc() {
    let Some(rom) = rom("hp42s-c.rom") else {
        return;
    };
    let mut m = Machine::new(Model::Hp42s, &rom).unwrap();
    run_until_screen(&mut m, "42s-memory-clear", 5_000_000);
    chord(&mut m, &[Key::On, Key::Ln]);
    self_test_rom_step(&mut m, "42s-self-test-rom");
}

/// Wait for the Pioneer self-test's ROM step result, "ROM" with its
/// verdict beside it, and compare it with the golden `name` (with
/// `SATURNUS_BLESS`, record the first such screen). The step shows for
/// about half a second: poll in short slices.
fn self_test_rom_step(m: &mut Machine, name: &str) {
    let path = golden_path(name);
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        let end = m.cycles() + 12_000_000;
        let mut seen_rom = false;
        while m.cycles() < end {
            run(m, 20_000);
            let t = m.lcd().to_text();
            let first: Vec<&str> = t.lines().take(7).collect();
            // "ROM" alone, then "ROM" with the verdict beside it.
            let wide = first.iter().map(|l| l.trim_end_matches('.').len()).max();
            if first[0].starts_with("####...###..#...#") {
                if wide < Some(25) {
                    seen_rom = true;
                } else if seen_rom {
                    std::fs::write(&path, &t).unwrap();
                    break;
                }
            }
        }
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let end = m.cycles() + 12_000_000;
    while m.cycles() < end && m.lcd().to_text() != want {
        run(m, 20_000);
    }
    assert_eq!(m.lcd().to_text(), want, "{name}: ROM step not reached");
}

/// The image `name` from `$SATURNUS_ROM_DIR`, `None` when the variable is
/// unset or the file is missing (the owner may not have every Pioneer).
fn rom_if_present(name: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    let path = std::path::Path::new(&dir).join(name);
    if !path.is_file() {
        eprintln!("{name} not in SATURNUS_ROM_DIR: skipped");
        return None;
    }
    Some(std::fs::read(&path).unwrap())
}

/// A cold-started Pioneer at its first screen (golden `<model>-cold`):
/// "MEMORY LOST" on the 27S; the language menu of an international 17B or
/// 17BII (its second ROM is mapped, see `pioneer_international_rom`),
/// left with the second menu key (ENGL) for MAIN.
///
/// The owner's 17B and 17BII images are international (96 KB); the
/// goldens are theirs, so a 64 KB English image is skipped
/// ([`pioneer_image`]).
fn pioneer_boot(model: Model, rom: &[u8]) -> Machine {
    let mut m = Machine::new(model, rom).unwrap();
    run_until_screen(&mut m, &format!("{}-cold", model.name()), 8_000_000);
    assert_eq!(m.lcd().height(), 16);
    assert_eq!(m.framebuffer().contrast, 22, "the KML 2.0 reset contrast");
    if model != Model::Hp27s {
        tap(&mut m, Key::B);
        run(&mut m, 2_000_000);
    }
    m
}

/// The images, as the owner gave them (in dump order; the machine puts
/// them in address order).
/// The Pioneer image `file`, `None` when it is missing or is a form the
/// goldens were not recorded from (a 64 KB English 17B or 17BII).
fn pioneer_image(model: Model, file: &str) -> Option<Vec<u8>> {
    let rom = rom_if_present(file)?;
    if model != Model::Hp27s && rom.len() != saturnus_core::INTL_17B_BYTES {
        eprintln!("{file}: a 64 KB image; the goldens are the international ROM's: skipped");
        return None;
    }
    Some(rom)
}

const PIONEERS: [(Model, &str); 3] = [
    (Model::Hp27s, "hp27s.rom"),
    (Model::Hp17bii, "hp17bii.rom"),
    (Model::Hp17b, "hp17b.rom"),
];

/// The 27S, 17BII and 17B boot, `2 + 3 =` shows 5.00 (5.0000 on the 27S)
/// in their algebraic mode, the state round-trips, and shift lights its
/// annunciator. No oracle: goldens recorded from saturnus, matching the
/// manuals (wiki: hardware/hp27s, hardware/hp17bii "Arithmetic").
#[test]
fn pioneers_boot_and_add() {
    for (model, file) in PIONEERS {
        let Some(rom) = pioneer_image(model, file) else {
            continue;
        };
        let mut m = pioneer_boot(model, &rom);
        for key in [Key::Two, Key::Plus, Key::Three, Key::Equals] {
            tap(&mut m, key);
            run(&mut m, 300_000);
        }
        run_until_screen(
            &mut m,
            &format!("{}-two-plus-three", model.name()),
            3_000_000,
        );

        let saved = m.save_state();
        let mut fresh = Machine::new(model, &rom).unwrap();
        fresh.load_state(&saved).unwrap();
        assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
        run(&mut m, 500_000);
        run(&mut fresh, 500_000);
        assert_eq!(fresh.save_state(), m.save_state(), "{}", model.name());

        m.key_down(Key::Shift).unwrap();
        run(&mut m, 300_000);
        m.key_up(Key::Shift).unwrap();
        run(&mut m, 300_000);
        assert!(m.framebuffer().annunciators.left_shift, "{}", model.name());
    }
}

/// CLR + the fifth menu key starts the self-test (wiki: hardware/hp27s,
/// hardware/hp17bii "Self-test"). Its ROM step clears the CRC at #40304,
/// reads #0001C-#1FFFB in three blocks, each as two interleaved halves of
/// 16-nibble words, and expects #FFFF; with the Lewis CRC rule (wiki:
/// hardware/lewis "CRC") every image passes.
#[test]
fn pioneers_self_test_rom_step() {
    for (model, file) in PIONEERS {
        let Some(rom) = pioneer_image(model, file) else {
            continue;
        };
        let mut m = pioneer_boot(model, &rom);
        chord(&mut m, &[Key::On, Key::E]);
        self_test_rom_step(&mut m, &format!("{}-self-test-rom", model.name()));
    }
}

/// An international 17B/17BII image maps its 32 KB language ROM at
/// #20000: the ROM checks its mark there and asks for a language first
/// (golden `17bii-cold`, menu DEUTS ENGL ESPAN FRANC ITAL PORT ...), and
/// the language ROM ends with its own self-test text, "OK-17B-L".
#[test]
fn pioneer_international_rom() {
    for model in [Model::Hp17bii, Model::Hp17b] {
        let file = if model == Model::Hp17b {
            "hp17b.rom"
        } else {
            "hp17bii.rom"
        };
        let Some(rom) = pioneer_image(model, file) else {
            continue;
        };
        let m = Machine::new(model, &rom).unwrap();
        // The language ROM's last bytes, read through the bus at #2FFE6.
        let text: Vec<u8> = (0..8)
            .map(|i| m.peek(0x2FFE6 + 2 * i) | (m.peek(0x2FFE7 + 2 * i) << 4))
            .collect();
        assert_eq!(&text, b"OK-17B-L", "{}", model.name());
        drop(pioneer_boot(model, &rom));
    }
}

/// The 17BII's clock runs on emulated time: the TIME menu's time line
/// changes once a second (wiki: hardware/hp17bii "Clock and alarms";
/// which timer keeps it is open: questions/pioneer-clock-alarm-hardware).
#[test]
fn hp17bii_clock_advances() {
    let Some(rom) = pioneer_image(Model::Hp17bii, "hp17bii.rom") else {
        return;
    };
    let mut m = pioneer_boot(Model::Hp17bii, &rom);
    tap(&mut m, Key::D); // MAIN's fourth label: TIME
    run(&mut m, 3_000_000);
    let mut changes = 0;
    let mut last = m.lcd().to_text();
    for _ in 0..1000 {
        run(&mut m, 10_000); // 10 s in all at 1 MHz
        let t = m.lcd().to_text();
        if t != last {
            changes += 1;
            last = t;
        }
    }
    assert!((9..=11).contains(&changes), "{changes} changes in 10 s");
}

/// A cold-started 28S (golden `28s-cold`): "Memory Lost" on the top line
/// above an empty stack, levels 3: to 1: (wiki: hardware/hp28s, the Owner's
/// Manual's cold start; recorded from saturnus, no oracle).
fn hp28s_boot(rom: &[u8]) -> Machine {
    let mut m = Machine::new(Model::Hp28s, rom).unwrap();
    run_until_screen(&mut m, "28s-cold", 8_000_000);
    assert_eq!((m.lcd().width(), m.lcd().height()), (137, 32));
    assert_eq!(m.framebuffer().contrast, 22, "the KML 2.0 reset contrast");
    m
}

/// The 28S (two Lewis chips, 137x32) boots, `2 ENTER 3 +` leaves 5 in
/// level 1, the state round-trips with both chips' blocks and the slave's
/// timers, shift lights its annunciator, and the command line's cursor
/// blinks on the slave chip's TIMER2 (wiki: hardware/hp28s).
#[test]
fn hp28s_boots_and_adds() {
    let Some(rom) = rom_if_present("hp28s.rom") else {
        return;
    };
    let mut m = hp28s_boot(&rom);
    for key in [Key::Two, Key::Enter, Key::Three, Key::Plus] {
        tap(&mut m, key);
        run(&mut m, 300_000);
    }
    run_until_screen(&mut m, "28s-two-enter-three-plus", 3_000_000);

    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp28s, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
    run(&mut m, 500_000);
    run(&mut fresh, 500_000);
    assert_eq!(fresh.save_state(), m.save_state());

    tap(&mut m, Key::Shift);
    run(&mut m, 300_000);
    assert!(m.framebuffer().annunciators.left_shift);
    tap(&mut m, Key::Shift);
    run(&mut m, 300_000);
    assert!(!m.framebuffer().annunciators.left_shift);

    // A letter opens the command line; its cursor blinks.
    tap(&mut m, Key::LetterP);
    run(&mut m, 300_000);
    let mut shown = std::collections::HashSet::new();
    for _ in 0..20 {
        run(&mut m, 100_000);
        shown.insert(m.lcd().to_text());
    }
    assert!(shown.len() >= 2, "the cursor blinks");
}

/// ON + the fifth menu key (◀) starts the 28S's repeating self-test (wiki:
/// hardware/hp28s "Self-test", from the Owner's Manual). Its ROM step
/// clears the master's CRC at #FFF04, reads #0001C-#1FFFB as on the
/// Pioneers (three blocks of two interleaved halves #5550 nibbles apart,
/// 16-nibble reads) and wants #FFFF, then the second ROM half; with the
/// Lewis CRC rule (wiki: hardware/lewis "CRC") the pass ends in "OK-28S".
#[test]
fn hp28s_self_test() {
    let Some(rom) = rom_if_present("hp28s.rom") else {
        return;
    };
    let mut m = hp28s_boot(&rom);
    chord(&mut m, &[Key::On, Key::E]);
    let path = golden_path("28s-self-test");
    // The result: "OK-28S" on the top line (its O and K).
    let result = |t: &str| t.starts_with(".###..#...#....");
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        let end = m.cycles() + 30_000_000;
        while m.cycles() < end {
            run(&mut m, 20_000);
            let t = m.lcd().to_text();
            if result(&t) {
                std::fs::write(&path, &t).unwrap();
                break;
            }
        }
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let end = m.cycles() + 30_000_000;
    while m.cycles() < end && m.lcd().to_text() != want {
        run(&mut m, 20_000);
    }
    assert_eq!(m.lcd().to_text(), want, "self-test result not reached");
}

/// A cold-started 19BII: the language menu (golden `19bii-cold`, D E F I
/// P and ENGLISH on the menu keys, as on the international 17BII), then
/// ENGLISH (the sixth menu key) gives "MEMORY LOST" over 0.00 and the MAIN
/// menu (golden `19bii-memory-lost`; wiki: hardware/hp19bii, recorded
/// from saturnus, no oracle).
fn hp19bii_boot(rom: &[u8]) -> Machine {
    let mut m = Machine::new(Model::Hp19bii, rom).unwrap();
    run_until_screen(&mut m, "19bii-cold", 8_000_000);
    assert_eq!((m.lcd().width(), m.lcd().height()), (137, 32));
    assert_eq!(m.framebuffer().contrast, 22, "the KML 2.0 reset contrast");
    tap(&mut m, Key::F);
    run_until_screen(&mut m, "19bii-memory-lost", 3_000_000);
    m
}

/// The 19BII (the 28S's two Lewis chips, 137x32) boots, `2 + 3 =` shows
/// 5.00, the state round-trips, shift, alpha (a letter in TEXT) and RAD
/// (MODES, its first menu key) light their annunciators, and the clock
/// advances once a second in the TIME menu (wiki: hardware/hp19bii).
#[test]
fn hp19bii_boots_and_adds() {
    let Some(rom) = rom_if_present("hp19bii.rom") else {
        return;
    };
    let mut m = hp19bii_boot(&rom);
    for key in [Key::Two, Key::Plus, Key::Three, Key::Equals] {
        tap(&mut m, key);
        run(&mut m, 300_000);
    }
    run_until_screen(&mut m, "19bii-two-plus-three", 3_000_000);

    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp19bii, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.lcd().to_text(), m.lcd().to_text());
    run(&mut m, 500_000);
    run(&mut fresh, 500_000);
    assert_eq!(fresh.save_state(), m.save_state());

    let lit = |m: &mut Machine, keys: &[Key]| {
        for &k in keys {
            tap(m, k);
            run(m, 300_000);
        }
        m.framebuffer().annunciators
    };
    assert!(lit(&mut m, &[Key::Shift]).left_shift);
    assert!(!lit(&mut m, &[Key::Shift]).left_shift);
    assert!(lit(&mut m, &[Key::F]).alpha, "TEXT types letters");
    assert!(!lit(&mut m, &[Key::Exit, Key::Exit]).alpha);
    assert!(lit(&mut m, &[Key::Shift, Key::Dsp, Key::A]).rad);
    let a = lit(&mut m, &[Key::A, Key::Exit, Key::Shift, Key::Exit]);
    assert!(!a.rad && !a.busy);

    tap(&mut m, Key::D); // MAIN's fourth label: TIME
    run(&mut m, 3_000_000);
    let mut changes = 0;
    let mut last = m.lcd().to_text();
    for _ in 0..1000 {
        run(&mut m, 10_000); // 10 s in all at 1 MHz
        let t = m.lcd().to_text();
        if t != last {
            changes += 1;
            last = t;
        }
    }
    assert!((9..=11).contains(&changes), "{changes} changes in 10 s");
}

/// An appointment set a minute ahead comes due and lights the ((•))
/// annunciator, the slave's word 0, reported as `busy` (wiki:
/// hardware/hp19bii "Annunciators"): TIME, SET, 10.00 TIME; then APPT,
/// APPT1, 10.01 TIME.
#[test]
fn hp19bii_appointment_alarm() {
    let Some(rom) = rom_if_present("hp19bii.rom") else {
        return;
    };
    let mut m = hp19bii_boot(&rom);
    let keys = |m: &mut Machine, keys: &[Key]| {
        for &k in keys {
            tap(m, k);
            run(m, 300_000);
        }
    };
    let ten = [Key::One, Key::Zero, Key::Point, Key::Zero, Key::Zero];
    keys(&mut m, &[Key::D, Key::D]);
    keys(&mut m, &ten);
    keys(&mut m, &[Key::B, Key::Exit, Key::B, Key::A]);
    keys(
        &mut m,
        &[Key::One, Key::Zero, Key::Point, Key::Zero, Key::One, Key::B],
    );
    keys(&mut m, &[Key::Exit]);
    assert!(!m.framebuffer().annunciators.busy, "not due yet");
    let end = m.cycles() + 90_000_000;
    while m.cycles() < end && !m.framebuffer().annunciators.busy {
        run(&mut m, 1_000_000);
    }
    assert!(
        m.framebuffer().annunciators.busy,
        "the alarm did not come due"
    );
}

/// ON + the fifth menu key starts the 19BII's repeating self-test (the
/// Owner's Manual, "Confirming Calculator Operation"); a pass, its ROM
/// step included, ends in "OK-19B II" (golden `19bii-self-test`; wiki:
/// hardware/hp19bii "Self-test").
#[test]
fn hp19bii_self_test() {
    let Some(rom) = rom_if_present("hp19bii.rom") else {
        return;
    };
    let mut m = hp19bii_boot(&rom);
    chord(&mut m, &[Key::On, Key::E]);
    let path = golden_path("19bii-self-test");
    // The result: "OK-19B II" alone on the top line.
    let result = |t: &str| {
        let mut rows = t.lines();
        rows.next()
            .is_some_and(|r| r.starts_with(".###..#...#.") && r.matches('#').count() > 15)
            && rows.skip(6).all(|r| !r.contains('#'))
    };
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        let end = m.cycles() + 30_000_000;
        while m.cycles() < end {
            run(&mut m, 20_000);
            let t = m.lcd().to_text();
            if result(&t) {
                std::fs::write(&path, &t).unwrap();
                break;
            }
        }
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let end = m.cycles() + 30_000_000;
    while m.cycles() < end && m.lcd().to_text() != want {
        run(&mut m, 20_000);
    }
    assert_eq!(m.lcd().to_text(), want, "self-test result not reached");
}
