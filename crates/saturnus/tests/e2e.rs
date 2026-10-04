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
