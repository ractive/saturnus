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
    m.key_down(key);
    run(m, 400_000);
    m.key_up(key);
}

/// Cold boot of the 48SX ROM J with zeroed RAM: the ROM finds no valid
/// memory and asks "Try To Recover Memory?"; NO (the sixth menu key, F)
/// clears memory and shows "Memory Clear" over the empty stack. The golden
/// screens are RAM dumps of the display area through `Lcd`.
#[test]
fn hp48sx_boot_to_memory_prompt() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    let mut m = Machine::new(Model::Hp48sx, &rom).unwrap();
    run_until_screen(&mut m, "48sx-try-to-recover-memory", 40_000_000);
    tap(&mut m, Key::F);
    run_until_screen(&mut m, "48sx-memory-clear", 20_000_000);
}

/// Save/load round trip on a booted ROM: boot to "Memory Clear", save, run
/// 1 M cycles, load, run the same 1 M cycles again; screen, registers and
/// the whole state must match. Also checks the framebuffer extras the ROM
/// sets up (contrast, annunciators).
#[test]
fn hp48sx_state_round_trip() {
    let Some(rom) = rom("sxrom-j") else {
        return;
    };
    let mut m = Machine::new(Model::Hp48sx, &rom).unwrap();
    run_until_screen(&mut m, "48sx-try-to-recover-memory", 40_000_000);
    tap(&mut m, Key::F);
    run_until_screen(&mut m, "48sx-memory-clear", 20_000_000);
    let fb = m.framebuffer();
    eprintln!(
        "after boot: {} cycles, contrast {}, annunciators {}",
        m.cycles(),
        fb.contrast,
        fb.annunciator_line()
    );
    assert!(
        Model::Hp48sx.contrast_range().contains(&fb.contrast),
        "ROM contrast {} outside the keyboard range",
        fb.contrast
    );

    let saved = m.save_state();
    // Something to do in the next million cycles: a key press.
    m.key_down(Key::Seven);
    run(&mut m, 1_000_000);
    let screen = m.lcd().to_text();
    let regs = m.cpu.regs.clone();
    let after = m.save_state();

    m.load_state(&saved).unwrap();
    assert_eq!(m.save_state(), saved);
    m.key_down(Key::Seven);
    run(&mut m, 1_000_000);
    assert_eq!(m.lcd().to_text(), screen);
    assert_eq!(m.cpu.regs, regs);
    assert_eq!(m.save_state(), after);

    // The state also loads into a freshly built machine.
    let mut fresh = Machine::new(Model::Hp48sx, &rom).unwrap();
    fresh.load_state(&after).unwrap();
    assert_eq!(fresh.lcd().to_text(), screen);
    assert_eq!(fresh.save_state(), after);
}

/// Boot ROM J to the empty stack ("Memory Clear").
fn boot_to_stack(rom: &[u8]) -> Machine {
    let mut m = Machine::new(Model::Hp48sx, rom).unwrap();
    run_until_screen(&mut m, "48sx-try-to-recover-memory", 40_000_000);
    tap(&mut m, Key::F);
    run_until_screen(&mut m, "48sx-memory-clear", 20_000_000);
    m
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

/// Push `packet` and collect the reply up to its CR (at most 4 s).
fn kermit_exchange(m: &mut Machine, packet: &[u8]) -> Vec<u8> {
    m.serial_push(packet);
    let mut reply = Vec::new();
    for _ in 0..40 {
        run(m, 200_000);
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
    let mut m = boot_to_stack(&rom);
    // SERVER: alpha alpha S E R V E R, ENTER.
    for k in [
        Key::Alpha,
        Key::Alpha,
        Key::Sin,
        Key::E,
        Key::Right,
        Key::Sqrt,
        Key::E,
        Key::Right,
        Key::Enter,
    ] {
        tap(&mut m, k);
        run(&mut m, 400_000);
    }
    run(&mut m, 4_000_000);
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
        let reply = kermit_exchange(&mut m, &kermit_packet(seq, kind, data));
        eprintln!("{} -> {:?}", kind as char, String::from_utf8_lossy(&reply));
        assert!(
            reply.len() >= 6 && reply[0] == 1 && reply[2] == seq + 32 && reply[3] == b'Y',
            "packet {} not acknowledged: {reply:?}",
            kind as char
        );
    }
    // Leave the server (ON is ATTN), then VAR and the first menu key
    // evaluate A. The stack must look as if 42 had been typed.
    tap(&mut m, Key::On);
    run(&mut m, 4_000_000);
    tap(&mut m, Key::Var);
    run(&mut m, 2_000_000);
    tap(&mut m, Key::A);
    run(&mut m, 2_000_000);
    let mut typed = boot_to_stack(&rom);
    for k in [Key::Four, Key::Two, Key::Enter] {
        tap(&mut typed, k);
        run(&mut typed, 400_000);
    }
    run(&mut typed, 2_000_000);
    // Rows above the menu labels (the bottom 8 rows differ: VAR menu).
    let stack = |m: &Machine| {
        m.lcd()
            .to_text()
            .lines()
            .take(56)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(stack(&m), stack(&typed), "A does not hold 42");
}
