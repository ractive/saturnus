use super::*;
use crate::bus::Chip;
use crate::cpu::{Bus, INTERRUPT_VECTOR};
use crate::io::timers::{CTRL_INT, CTRL_WAKE, CTRL_XTRA_OR_RUN};

const MAIN: u32 = 0x20;
/// `GOTO` to itself.
const LOOP: &str = "6FFF";

/// A packed ROM image with `code` (hex nibble strings) placed at the given
/// nibble addresses. #00000 jumps to MAIN; the interrupt vector loops.
fn rom_image(code: &[(u32, &str)]) -> Vec<u8> {
    let mut nibbles = vec![0u8; Model::Hp48sx.rom_bytes() * 2];
    let mut place = |addr: u32, s: &str| {
        for (i, c) in s.chars().enumerate() {
            nibbles[addr as usize + i] = c.to_digit(16).unwrap() as u8;
        }
    };
    place(0, "6F10"); // GOTO #00020
    place(INTERRUPT_VECTOR, LOOP);
    for &(a, s) in code {
        place(a, s);
    }
    nibbles.chunks(2).map(|p| p[0] | (p[1] << 4)).collect()
}

fn machine(code: &[(u32, &str)]) -> Machine {
    Machine::new(Model::Hp48sx, &rom_image(code)).unwrap()
}

/// Machine with HDW at #100 and the main program `main` at MAIN.
fn with_hdw(main: &str) -> Machine {
    let mut m = machine(&[(MAIN, main)]);
    m.hw.config(0x100);
    m
}

/// Cycles covering `ticks` timer ticks, with a little slack.
fn cycles_for_ticks(ticks: u64) -> u64 {
    ticks * u64::from(Model::Hp48sx.clock_hz()) / TICKS_PER_SECOND + 100
}

#[test]
fn rom_size_checked() {
    let err = Machine::new(Model::Hp48sx, &[0; 10]).unwrap_err();
    assert_eq!(
        err,
        Error::RomSize {
            expected: 262_144,
            actual: 10
        }
    );
}

#[test]
fn rom_answers_when_nothing_configured() {
    let mut m = machine(&[]);
    assert_eq!(m.peek(0), 6);
    assert_eq!(m.peek(1), 0xF);
    assert_eq!(m.hw.read_nibble(2), 1);
    m.step().unwrap();
    assert_eq!(m.cpu.regs.pc, MAIN);
}

#[test]
fn config_program_maps_ram_over_rom() {
    // LC(5) #00100, CONFIG, LC(5) #F0000, CONFIG, LC(5) #70000, CONFIG.
    let prog = "3400100805340000F80534000078056FFF";
    let mut m = machine(&[(MAIN, prog), (0x70000, "A")]);
    assert_eq!(m.peek(0x70000), 0xA);
    m.run_cycles(500).unwrap();
    assert!(m.hw.mc.is_configured(Chip::Hdw));
    assert_eq!(m.hw.mc.window(Chip::Nce2), Some((0x70000, 0xF0000)));
    assert_eq!(m.peek(0x70000), 0);
    m.hw.write_nibble(0x70000, 9);
    m.hw.write_nibble(0x7FFFF, 3);
    assert_eq!(m.peek(0x70000), 9);
    assert_eq!(m.hw.read_nibble(0x7FFFF), 3);
    assert_eq!(m.hw.ram.read(0), 9);
    // ROM ignores writes.
    m.hw.write_nibble(0x50, 5);
    assert_eq!(m.peek(0x50), 0);
}

#[test]
fn hdw_reaches_io_registers() {
    let mut m = with_hdw(LOOP);
    m.hw.write_nibble(0x137, 5);
    assert_eq!(m.hw.io.timers.t1, 5);
    assert_eq!(m.hw.read_nibble(0x137), 5);
    assert_eq!(m.peek(0x137), 5);
    // Moved window: #100 is ROM again.
    let mut m = machine(&[]);
    m.hw.config(0x200);
    m.hw.write_nibble(0x237, 7);
    assert_eq!(m.hw.io.timers.t1, 7);
}

#[test]
fn crc_follows_data_reads_not_fetches() {
    // D0=(5) #00040, A=DAT0 A, loop.
    let mut m = machine(&[(MAIN, "1B04000142"), (MAIN + 10, LOOP), (0x40, "12345")]);
    m.hw.config(0x100);
    m.step().unwrap(); // GOTO
    m.step().unwrap(); // D0=
    assert_eq!(m.hw.io.crc(), 0);
    m.step().unwrap(); // A=DAT0
    let crc = m.hw.io.crc();
    assert_ne!(crc, 0);
    m.step().unwrap(); // loop
    assert_eq!(m.hw.io.crc(), crc);
    // Reads of the I/O window leave the CRC alone.
    m.hw.read_data(0x137);
    assert_eq!(m.hw.io.crc(), crc);
    m.hw.read_data(0x41);
    assert_ne!(m.hw.io.crc(), crc);
}

#[test]
fn timer2_interrupt_vectors() {
    let mut m = with_hdw(LOOP);
    m.hw.io.timers.t2 = 10;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_INT);
    m.run_cycles(cycles_for_ticks(5)).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(m.cpu.regs.in_interrupt);
    assert!((INTERRUPT_VECTOR..INTERRUPT_VECTOR + 4).contains(&m.cpu.regs.pc));
}

#[test]
fn timer_interrupt_ignores_intoff() {
    let mut m = with_hdw(LOOP);
    m.cpu.regs.interrupts_enabled = false;
    m.hw.io.timers.t2 = 3;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_INT);
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(m.cpu.regs.in_interrupt);
}

fn key_setup(enabled: bool, t2_run: bool) -> Machine {
    let mut m = with_hdw(LOOP);
    m.cpu.regs.interrupts_enabled = enabled;
    m.hw.io.timers.t2 = 0x7000_0000;
    if t2_run {
        m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN);
    }
    m.hw.write_out(0x1FF);
    m.run_cycles(cycles_for_ticks(16)).unwrap();
    m
}

#[test]
fn key_press_interrupts() {
    let mut m = key_setup(true, true);
    assert!(!m.cpu.regs.in_interrupt);
    m.key_down(Key::Enter);
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(m.cpu.regs.in_interrupt);
    assert_eq!(m.hw.io.peek(0x19), 0x8, "KDN set by the poll");
}

#[test]
fn key_press_masked_by_intoff() {
    let mut m = key_setup(false, true);
    m.key_down(Key::Enter);
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
    // Level already high: enabling later gives no edge.
    m.cpu.regs.interrupts_enabled = true;
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
}

#[test]
fn key_poll_stops_with_timer2() {
    let mut m = key_setup(true, false);
    m.key_down(Key::Enter);
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
}

#[test]
fn on_key_is_non_maskable() {
    let mut m = key_setup(false, false);
    m.key_down(Key::On);
    m.step().unwrap();
    assert!(m.cpu.regs.in_interrupt);
}

#[test]
fn shutdn_wakes_on_timer_with_wake_bit() {
    // SHUTDN, loop.
    let mut m = with_hdw("8076FFF");
    m.hw.io.timers.t2 = 100;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_WAKE);
    m.step().unwrap(); // GOTO
    m.step().unwrap(); // SHUTDN
    assert!(m.is_shutdown());
    m.run_cycles(cycles_for_ticks(50)).unwrap();
    assert!(m.is_shutdown());
    assert_eq!(m.cpu.regs.pc, MAIN + 3);
    m.run_cycles(cycles_for_ticks(60)).unwrap();
    assert!(!m.is_shutdown());
    assert!(!m.cpu.regs.in_interrupt);
    assert_eq!(m.cpu.regs.pc, MAIN + 3);
}

#[test]
fn shutdn_without_wake_bit_sleeps_on() {
    let mut m = with_hdw("8076FFF");
    m.hw.io.timers.t2 = 100;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN);
    m.run_cycles(cycles_for_ticks(500)).unwrap();
    assert!(m.is_shutdown());
}

#[test]
fn shutdn_with_timer_interrupt_wakes_into_handler() {
    let mut m = with_hdw("8076FFF");
    m.hw.io.timers.t2 = 100;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_INT);
    m.run_cycles(cycles_for_ticks(200)).unwrap();
    assert!(!m.is_shutdown());
    assert!(m.cpu.regs.in_interrupt);
}

#[test]
fn hour_long_shutdn_is_fast() {
    let mut m = with_hdw("8076FFF");
    m.hw.io.timers.t2 = 0x01C2_0000; // 3600 s at 8192 Hz
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_WAKE);
    m.run_cycles(100).unwrap();
    assert!(m.is_shutdown());
    let start = std::time::Instant::now();
    m.run_cycles(3599 * u64::from(Model::Hp48sx.clock_hz()))
        .unwrap();
    assert!(m.is_shutdown());
    m.run_cycles(2 * u64::from(Model::Hp48sx.clock_hz()))
        .unwrap();
    assert!(!m.is_shutdown());
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn shutdn_with_key_held_does_not_stop() {
    let mut m = with_hdw("8076FFF");
    m.hw.write_out(0x1FF);
    m.key_down(Key::Enter);
    m.step().unwrap();
    m.step().unwrap();
    assert!(!m.is_shutdown());
}

#[test]
fn reset_keeps_ram() {
    let mut m = with_hdw(LOOP);
    m.hw.config(0xF0000);
    m.hw.config(0x70000);
    m.hw.write_nibble(0x70005, 7);
    m.run_cycles(100).unwrap();
    m.reset();
    assert_eq!(m.cpu.regs.pc, 0);
    assert!(!m.hw.mc.is_configured(Chip::Hdw));
    assert_eq!(m.hw.ram.read(5), 7);
}

#[test]
fn invalid_opcode_halts() {
    // 105 is undefined.
    let mut m = machine(&[(MAIN, "105")]);
    m.step().unwrap();
    let err = m.run_cycles(100).unwrap_err();
    assert!(matches!(err, Halt::InvalidOpcode { pc: MAIN, .. }));
    assert!(err.to_string().starts_with("invalid opcode at #00020"));
}

fn lcd_machine() -> Machine {
    let mut m = with_hdw(LOOP);
    m.hw.config(0xF0000);
    m.hw.config(0x70000);
    m
}

fn set_io(m: &mut Machine, off: u32, value: u32, nibbles: u32) {
    for i in 0..nibbles {
        m.hw.write_nibble(0x100 + off + i, ((value >> (4 * i)) & 0xF) as u8);
    }
}

#[test]
fn lcd_off_is_blank() {
    let mut m = lcd_machine();
    for a in 0x70000..0x70000 + 34 * 64 {
        m.hw.write_nibble(a, 0xF);
    }
    set_io(&mut m, 0x20, 0x70000, 5);
    assert!(m.lcd().pixels.iter().all(|r| r.iter().all(|&p| !p)));
    set_io(&mut m, 0x00, 0x8, 1);
    assert!(m.lcd().pixels.iter().all(|r| r.iter().all(|&p| p)));
}

#[test]
fn lcd_main_and_menu_bitmaps() {
    let mut m = lcd_machine();
    let main = 0x70000;
    let menu = 0x72000;
    let line_offset = 4; // 38 nibbles per main row
    // Main row r: pixel r lit (nibble r/4, bit r%4).
    for r in 0..56u32 {
        let a = main + r * 38 + r / 4;
        m.hw.write_nibble(a, 1 << (r % 4));
    }
    // Menu rows: rightmost pixel (130 = nibble 32, bit 2) and pixel 0.
    for r in 0..8u32 {
        m.hw.write_nibble(menu + r * 34, 1);
        m.hw.write_nibble(menu + r * 34 + 32, 4);
    }
    set_io(&mut m, 0x00, 0x8, 1);
    set_io(&mut m, 0x20, main, 5);
    set_io(&mut m, 0x25, line_offset, 3);
    set_io(&mut m, 0x28, 55, 2);
    set_io(&mut m, 0x30, menu, 5);
    let lcd = m.lcd();
    for (y, row) in lcd.pixels.iter().enumerate() {
        let lit: Vec<usize> = (0..LCD_WIDTH).filter(|&x| row[x]).collect();
        if y < 56 {
            assert_eq!(lit, vec![y], "row {y}");
        } else {
            assert_eq!(lit, vec![0, 130], "row {y}");
        }
    }
    let text = lcd.to_text();
    assert_eq!(text.lines().count(), 64);
    assert!(text.lines().all(|l| l.len() == 131));
    assert!(text.starts_with("#."));
}

#[test]
fn lcd_bit_offset_shifts_left() {
    let mut m = lcd_machine();
    let main = 0x70000;
    // Pixel 5 of each row in memory; with bit offset 5 (spans a nibble,
    // so stride 34 + 2) it shows at x = 0.
    for r in 0..64u32 {
        m.hw.write_nibble(main + r * 36 + 1, 0x2);
    }
    set_io(&mut m, 0x00, 0x8 | 5, 1);
    set_io(&mut m, 0x20, main, 5);
    let lcd = m.lcd();
    for row in &lcd.pixels {
        let lit: Vec<usize> = (0..LCD_WIDTH).filter(|&x| row[x]).collect();
        assert_eq!(lit, vec![0]);
    }
}
