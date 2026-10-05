//! End-to-end tests against real ROM images, gated by `SATURNUS_ROM_DIR`
//! (see the test policy). Without the variable every test is skipped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use saturnus::cpu::{ADDR_MASK, decode};
use saturnus::io::Key;
use saturnus::{Halt, Machine, Model};

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
    saturnus::cpu::disassemble(&d.instr)
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
    assert_eq!(
        m.hw.mc.window(saturnus::bus::Chip::Nce2),
        Some((0xF0000, 0xF0000))
    );
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
    assert_eq!(
        m.hw.mc.window(saturnus::bus::Chip::Nce2),
        Some((0x80000, 0x80000))
    );
    for chip in [saturnus::bus::Chip::Ce2, saturnus::bus::Chip::Nce3] {
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
    // A generic-command packet: CD UP DROP, the request hptx's `cd ..`
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
/// reads #0001C-#1FFFB through it and expects #FFFF; this image gives
/// #1BE8, which the step prints ("ROM 01BE8"), and the summary reads FAIL. Recorded as found: either the dump
/// has bad bits or the Lewis CRC differs from the Clarke's (wiki:
/// questions/hp42s-rom-crc).
#[test]
fn hp42s_self_test_rom_crc() {
    let Some(rom) = rom("hp42s-c.rom") else {
        return;
    };
    let mut m = Machine::new(Model::Hp42s, &rom).unwrap();
    run_until_screen(&mut m, "42s-memory-clear", 5_000_000);
    chord(&mut m, &[Key::On, Key::Ln]);
    // The ROM step shows for about half a second: poll in short slices.
    let path = golden_path("42s-self-test-rom");
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        run(&mut m, 5_600_000);
        std::fs::write(&path, m.lcd().to_text()).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let end = m.cycles() + 8_000_000;
    while m.cycles() < end && m.lcd().to_text() != want {
        run(&mut m, 50_000);
    }
    // The screen reads "ROM 01BE8": the step prints the CRC it got.
    assert_eq!(m.lcd().to_text(), want, "self-test ROM step not reached");
}
