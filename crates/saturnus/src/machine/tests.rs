use super::*;
use crate::bus::Chip;
use crate::cpu::{Bus, INTERRUPT_VECTOR};
use crate::io::timers::{CTRL_INT, CTRL_WAKE, CTRL_XTRA_OR_RUN};
use crate::modules::Nce1;

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
    m.key_down(Key::Enter).unwrap();
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(m.cpu.regs.in_interrupt);
    assert_eq!(m.hw.io.peek(0x19), 0x8, "KDN set by the poll");
}

#[test]
fn key_press_masked_by_intoff() {
    let mut m = key_setup(false, true);
    m.key_down(Key::Enter).unwrap();
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
    m.key_down(Key::Enter).unwrap();
    m.run_cycles(cycles_for_ticks(10)).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
}

#[test]
fn on_key_is_non_maskable() {
    let mut m = key_setup(false, false);
    m.key_down(Key::On).unwrap();
    m.step().unwrap();
    assert!(m.cpu.regs.in_interrupt);
}

#[test]
fn rti_with_on_held_and_pending_enters_once() {
    // Handler is a bare RTI.
    let mut m = machine(&[(INTERRUPT_VECTOR, "0F")]);
    m.hw.keyboard.press(Key::On);
    m.cpu.interrupt(); // enter the handler
    m.cpu.interrupt(); // latch a second one
    assert!(m.cpu.regs.interrupt_pending);
    m.step().unwrap(); // RTI: the CPU re-vectors for the latched one
    assert_eq!(m.cpu.regs.pc, INTERRUPT_VECTOR);
    assert!(m.cpu.regs.in_interrupt);
    // The machine must not latch yet another entry on top.
    assert!(!m.cpu.regs.interrupt_pending);
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
    m.key_down(Key::Enter).unwrap();
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

/// HDW, RAM and port 1 (CE1, 128 KB at #80000) configured; CE2 next.
fn card_machine() -> Machine {
    let mut m = with_hdw(LOOP);
    for v in [0xF0000, 0x70000, 0xE0000, 0x80000] {
        m.hw.config(v);
    }
    m
}

#[test]
fn card_size_checked() {
    let mut m = card_machine();
    for n in [0, 512, 1000, 3072, 256 * 1024] {
        assert_eq!(
            m.insert_card(Port::One, &vec![0; n]),
            Err(Error::CardSize {
                actual: n,
                max: 128 * 1024
            })
        );
    }
    assert!(m.hw.card(Port::One).is_none());
    assert!(!m.remove_card(Port::Two));
}

#[test]
fn card_reads_writes_and_mirrors() {
    let mut m = card_machine();
    assert_eq!(m.peek(0x80000), 0, "empty port reads open bus");
    let mut image = vec![0u8; 1024];
    image[0] = 0x21;
    m.insert_card(Port::One, &image).unwrap();
    assert_eq!(m.peek(0x80000), 1);
    assert_eq!(m.peek(0x80001), 2);
    // 1 KB = 2048 nibbles, mirrored through the 128 KB window.
    assert_eq!(m.peek(0x80800), 1);
    m.hw.write_nibble(0x80003, 0xC);
    assert_eq!(m.card_image(Port::One).unwrap()[1], 0xC0);
    assert_eq!(m.card_image(Port::Two), None);
    assert!(m.remove_card(Port::One));
    assert_eq!(m.peek(0x80000), 0);
}

#[test]
fn card_detect_interrupts_and_sets_mp() {
    let mut m = card_machine();
    // Detection off: insertion is silent and #10F reads 0.
    m.insert_card(Port::Two, &[0; 1024]).unwrap();
    m.step().unwrap();
    assert!(!m.cpu.regs.in_interrupt);
    assert_eq!(m.peek(0x10F), 0);
    // Detection on: #10F shows port 2 present and writable.
    m.hw.write_nibble(0x10E, 0xC);
    assert_eq!(m.peek(0x10F), 0xA);
    // Removal raises the non-maskable card-detect interrupt and MP.
    m.cpu.regs.interrupts_enabled = false;
    assert!(m.remove_card(Port::Two));
    m.step().unwrap();
    assert!(m.cpu.regs.in_interrupt);
    assert_eq!(m.cpu.regs.hst & crate::cpu::regs::HST_MP, 0x8);
    assert_eq!(m.peek(0x10E), 0xE);
    // The handler clears SMP; MP can then be cleared and stays clear.
    m.hw.write_nibble(0x10E, 0xC);
    m.cpu.regs.hst = 0;
    m.step().unwrap();
    assert_eq!(m.cpu.regs.hst, 0);
}

#[test]
fn reset_keeps_cards() {
    let mut m = card_machine();
    m.insert_card(Port::One, &[0x33; 2048]).unwrap();
    m.reset();
    assert!(m.hw.card(Port::One).is_some());
    m.hw.config(0x100);
    m.hw.write_nibble(0x10E, 0x8);
    assert_eq!(m.peek(0x10F), 0x5);
}

#[test]
fn card_status_bits_pair_by_chip_select() {
    // ROM J pairing: CE1 (port 1) on bits 0 and 2, CE2 (port 2) on 1 and 3.
    let mut m = card_machine();
    m.hw.write_nibble(0x10E, 0x8);
    m.insert_card(Port::One, &[0; 1024]).unwrap();
    assert_eq!(m.peek(0x10F), 0x1 | 0x4);
    m.remove_card(Port::One);
    m.insert_card(Port::Two, &[0; 1024]).unwrap();
    assert_eq!(m.peek(0x10F), 0x2 | 0x8);
    m.insert_card(Port::One, &[0; 1024]).unwrap();
    assert_eq!(m.peek(0x10F), 0xF);
}

#[test]
fn display_refresh_stalls_the_cpu() {
    // Time per instruction = table cycles x calibration (48SX: 1.267)
    // x 1.13 while DON is set, the fraction carried between steps, so
    // each sum is exact to within one cycle.
    let mut m = lcd_machine();
    m.step().unwrap();
    let mut cpu = m.clone();
    let table: u64 = (0..1000)
        .map(|_| u64::from(cpu.cpu.step(&mut cpu.hw).cycles))
        .sum();
    let off: u64 = (0..1000).map(|_| u64::from(m.step().unwrap())).sum();
    assert!(off.abs_diff(table * 1267 / 1000) <= 1, "{off} vs {table}");
    set_io(&mut m, 0x00, 0x8, 1);
    let on: u64 = (0..1000).map(|_| u64::from(m.step().unwrap())).sum();
    assert!(
        on.abs_diff(table * 1267 * 113 / 100_000) <= 1,
        "{on} vs {table}"
    );
}

#[test]
fn framebuffer_annunciators_and_contrast() {
    let mut m = with_hdw(LOOP);
    set_io(&mut m, 0x01, 0x1C, 2); // contrast 12 + bit 4
    set_io(&mut m, 0x0B, 0x85, 2); // left shift, alpha, AON
    let fb = m.framebuffer();
    assert_eq!(fb.contrast, 0x1C);
    assert_eq!(fb.annunciators, Annunciators::default(), "TIMER2 stopped");
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN);
    let fb = m.framebuffer();
    assert!(fb.annunciators.left_shift && fb.annunciators.alpha);
    assert_eq!(fb.annunciator_line(), "leftshift alpha");
    assert_eq!(fb.to_text(), m.lcd().to_text());
    // AON clear: nothing lit.
    m.hw.write_nibble(0x10C, 0x0);
    assert_eq!(m.framebuffer().annunciator_line(), "-");
}

/// Echo program: SON on, then forever: wait for RBF, copy RBR to TBR.
/// D0=(5) #00110, LC(1) #8, DAT0=C 1; L: D0=(2) #11, A=DAT0 1,
/// ?ABIT=0 0 GOYES L, D0=(2) #14, A=DAT0 B, D0=(2) #16, DAT0=A B, GOTO L.
const ECHO: &str = "1B0110030815C0191115A0808603F194114A196114862EF";

/// Cycles for `frames` byte frames at 9600 baud (182 sixteenths each at
/// 153_600 sixteenths per second), rounded up.
fn frame_cycles(frames: u64) -> u64 {
    (frames * 182 * u64::from(Model::Hp48sx.clock_hz())).div_ceil(16 * 9600)
}

fn echo_machine() -> Machine {
    let mut m = with_hdw(ECHO);
    m.hw.write_nibble(0x10D, 6);
    m.run_cycles(200).unwrap();
    assert!(m.hw.io.uart.is_on());
    m
}

#[test]
fn echo_program_disassembles() {
    let m = echo_machine();
    let mut pc = MAIN;
    let mut text = Vec::new();
    for _ in 0..11 {
        let d = crate::cpu::decode(|a| m.peek(a), pc);
        text.push(crate::cpu::disassemble(&d.instr));
        pc += u32::from(d.len);
    }
    let text = text.join("; ");
    assert!(text.contains("?ABIT=0 0  GOYES #0002E"), "{text}");
    assert!(text.ends_with("GOTO    #0002E"), "{text}");
}

#[test]
fn serial_round_trip_through_cpu_at_line_rate() {
    let mut m = echo_machine();
    assert_eq!(m.serial_baud(), 9600);
    m.serial_push(b"hello");
    assert_eq!(m.serial_pending(), 5);
    // After two frames two bytes are in and the first is on its way back.
    m.run_cycles(frame_cycles(2)).unwrap();
    assert_eq!(m.serial_pending(), 3);
    assert_eq!(m.serial_drain(), b"h");
    m.run_cycles(frame_cycles(4)).unwrap();
    assert_eq!(m.serial_pending(), 0);
    assert_eq!(m.serial_drain(), b"ello");
    assert!(m.serial_drain().is_empty());
}

#[test]
fn serial_loop_back_without_cpu() {
    let mut m = with_hdw(LOOP);
    m.hw.write_nibble(0x10D, 6);
    m.hw.write_nibble(0x110, 0x8);
    m.hw.write_nibble(0x112, 0x4); // LPB
    m.serial_push(b"ignored");
    m.hw.write_nibble(0x116, 0xA);
    m.hw.write_nibble(0x117, 0x5);
    m.run_cycles(frame_cycles(1)).unwrap();
    assert_eq!(m.hw.read_nibble(0x111), 0x1, "RBF");
    assert_eq!(m.hw.read_nibble(0x114), 0xA);
    assert_eq!(m.hw.read_nibble(0x115), 0x5);
    assert_eq!(m.hw.read_nibble(0x111), 0x0, "the read cleared RBF");
    m.run_cycles(frame_cycles(8)).unwrap();
    assert!(m.serial_drain().is_empty());
    assert_eq!(m.serial_pending(), 0, "wire bytes passed unseen");
    assert_eq!(m.hw.read_nibble(0x111), 0x0);
}

#[test]
fn uart_interrupt_vectors_and_ignores_intoff() {
    let mut m = with_hdw(LOOP);
    m.cpu.regs.interrupts_enabled = false;
    m.hw.write_nibble(0x10D, 6);
    m.hw.write_nibble(0x110, 0x8 | 0x2); // SON, rx full
    m.serial_push(&[0x42]);
    m.run_cycles(frame_cycles(1) / 2).unwrap();
    assert!(!m.cpu.regs.in_interrupt);
    m.run_cycles(frame_cycles(1) / 2).unwrap();
    assert!(m.cpu.regs.in_interrupt);
    assert!((INTERRUPT_VECTOR..INTERRUPT_VECTOR + 4).contains(&m.cpu.regs.pc));
}

#[test]
fn shutdn_wakes_on_receive_start() {
    // SHUTDN, loop.
    let mut m = with_hdw("8076FFF");
    m.hw.write_nibble(0x10D, 6);
    m.hw.write_nibble(0x110, 0x8 | 0x1); // SON, rx start
    m.step().unwrap(); // GOTO
    m.step().unwrap(); // SHUTDN
    assert!(m.is_shutdown());
    // A long sleep is skipped in bulk while the line is idle.
    m.run_cycles(2_000_000).unwrap();
    assert!(m.is_shutdown());
    let before = m.cycles();
    m.serial_push(&[1]);
    m.run_cycles(100).unwrap();
    assert!(!m.is_shutdown());
    assert!(m.cpu.regs.in_interrupt);
    assert!(m.cycles() - before < 200, "woke at the start bit");
}

#[test]
fn reset_keeps_the_wire() {
    let mut m = echo_machine();
    m.serial_push(b"abc");
    m.run_cycles(frame_cycles(2)).unwrap();
    m.reset();
    assert_eq!(m.serial_drain(), b"a");
    assert_eq!(m.serial_pending(), 1, "c waits, b was in flight");
    assert!(!m.hw.io.uart.is_on());
}

#[test]
fn serial_clear_inbound_drops_the_queue() {
    let mut m = machine(&[]);
    m.serial_push(b"abc");
    assert_eq!(m.serial_pending(), 3);
    assert_eq!(m.serial_clear_inbound(), 3);
    assert_eq!(m.serial_pending(), 0);
    assert_eq!(m.serial_clear_inbound(), 0);
}

/// A 48GX with a 512 KB ROM: MAIN holds `main`, #41234 holds 4 (lower
/// ROM) and #C1234 holds #B (upper ROM).
fn gx_machine(main: &str) -> Machine {
    let mut nibbles = vec![0u8; Model::Hp48gx.rom_bytes() * 2];
    let mut place = |addr: u32, s: &str| {
        for (i, c) in s.chars().enumerate() {
            nibbles[addr as usize + i] = c.to_digit(16).unwrap() as u8;
        }
    };
    place(0, "6F10");
    place(INTERRUPT_VECTOR, LOOP);
    place(MAIN, main);
    place(0x41234, "4");
    place(0xC1234, "B");
    let image: Vec<u8> = nibbles.chunks(2).map(|p| p[0] | (p[1] << 4)).collect();
    Machine::new(Model::Hp48gx, &image).unwrap()
}

/// The GX map with both slots empty (wiki: hardware/memory-controller
/// "Default maps"): HDW #100, RAM 128 KB at #80000, bank latch at #7F000,
/// CE2 and NCE3 parked at #7E000 (2 KB) unless `ports_at_c0000`, which
/// configures both as 128 KB at #C0000 like the tutorial's bring-up.
fn gx_bring_up(m: &mut Machine, ports_at_c0000: bool) {
    let ports: [u32; 4] = if ports_at_c0000 {
        [0xC0000, 0xC0000, 0xC0000, 0xC0000]
    } else {
        [0xFF000, 0x7E000, 0xFF000, 0x7E000]
    };
    for v in [0x100, 0xC0000, 0x80000, 0xFF000, 0x7F000] {
        m.hw.config(v);
    }
    for v in ports {
        m.hw.config(v);
    }
    assert!(m.hw.mc.all_configured());
}

/// Write #129 (DA19 is bit 3).
fn set_da19(m: &mut Machine, on: bool) {
    m.hw.write_nibble(0x129, if on { 0x8 } else { 0x0 });
}

#[test]
fn gx_da19_switches_upper_rom() {
    let mut m = gx_machine(LOOP);
    gx_bring_up(&mut m, false);
    // Power-on DA19 = 0: the lower 256 KB repeat at #80000 (RAM covers
    // #80000-#BFFFF, so look at #C0000-#FFFFF).
    assert_eq!(m.peek(0xC1234), 4);
    assert_eq!(m.peek(0x41234), 4);
    set_da19(&mut m, true);
    assert_eq!(m.peek(0xC1234), 0xB);
    assert_eq!(m.peek(0x41234), 4);
    set_da19(&mut m, false);
    assert_eq!(m.peek(0xC1234), 4);
}

#[test]
fn gx_bank_latch_selects_port_2_bank() {
    let mut m = gx_machine(LOOP);
    gx_bring_up(&mut m, true);
    // A 256 KB card: bank 0 starts with 1, bank 1 with 2.
    let mut card = vec![0u8; 256 * 1024];
    card[0] = 0x01;
    card[128 * 1024] = 0x02;
    m.insert_card(Port::Two, &card).unwrap();
    // DA19 = 0, BEN = 0: NCE3 is off and CE2 (empty port 1) answers.
    assert_eq!(m.peek(0xC0000), 0);
    m.hw.config(0); // no-op: all configured
    m.hw.unconfig(0xC0000); // drop CE2, NCE3 stays at #C0000
    assert_eq!(m.peek(0xC0000), 0, "BEN = 0: ROM (mirrored lower half)");
    // Read #7F040 + 2n with a byte read: bank n, BEN set.
    m.hw.read_data(0x7F042);
    m.hw.read_data(0x7F043);
    assert_eq!(m.hw.bank_latch(), 0x21);
    assert_eq!(m.peek(0xC0000), 2);
    m.hw.read_data(0x7F040);
    assert_eq!(m.peek(0xC0000), 1);
    // Writes go to the selected bank.
    m.hw.write_nibble(0xC0001, 0x7);
    assert_eq!(m.card_image(Port::Two).unwrap()[0], 0x71);
    // Banks past the card's end mirror it (bank 3 of a 2-bank card).
    m.hw.read_data(0x7F046);
    assert_eq!(m.peek(0xC0000), 2);
    // DA19 = 1 hands the pin to the ROM: upper ROM, no port 2.
    set_da19(&mut m, true);
    assert_eq!(m.peek(0xC1234), 0xB);
    set_da19(&mut m, false);
    // BEN = 0 (read in the #7F000 half) turns port 2 off again.
    m.hw.read_data(0x7F000);
    assert_eq!(m.hw.bank_latch(), 0);
    assert_eq!(m.peek(0xC1234), 4);
    // Peeks and writes do not latch.
    m.peek(0x7F044);
    m.hw.write_nibble(0x7F044, 0);
    assert_eq!(m.hw.bank_latch(), 0);
}

#[test]
fn gx_latch_cleared_by_shutdn_and_reset() {
    // SHUTDN (807) then loop.
    let mut m = gx_machine("807");
    gx_bring_up(&mut m, true);
    m.hw.read_data(0x7F05E);
    assert_eq!(m.hw.bank_latch(), 0x2F);
    m.step().unwrap();
    m.step().unwrap();
    assert_eq!(m.hw.bank_latch(), 0);
    m.hw.read_data(0x7F05E);
    m.reset();
    assert_eq!(m.hw.bank_latch(), 0);
}

#[test]
fn gx_card_ports_and_status_bits() {
    let mut m = gx_machine(LOOP);
    gx_bring_up(&mut m, true);
    m.hw.write_nibble(0x10E, 0x8);
    // Port 1 is CE2 (bits 1 and 3), port 2 the other pair.
    m.insert_card(Port::One, &[0x21; 1024]).unwrap();
    assert_eq!(m.peek(0x10F), 0xA);
    assert_eq!(m.peek(0xC0000), 1, "port 1 card on CE2 at #C0000");
    m.insert_card(Port::Two, &[0; 1024]).unwrap();
    assert_eq!(m.peek(0x10F), 0xF);
    m.remove_card(Port::One);
    assert_eq!(m.peek(0x10F), 0x5);
    // Sizes: port 1 up to 128 KB, port 2 up to 4 MB.
    assert_eq!(
        m.insert_card(Port::One, &vec![0; 256 * 1024]),
        Err(Error::CardSize {
            actual: 256 * 1024,
            max: 128 * 1024
        })
    );
    m.insert_card(Port::Two, &vec![0; 4 * 1024 * 1024]).unwrap();
    assert!(m.insert_card(Port::Two, &vec![0; 8 * 1024 * 1024]).is_err());
}

#[test]
fn gx_runs_at_4_mhz() {
    let m = gx_machine(LOOP);
    assert_eq!(m.model().clock_hz(), 4_000_000);
    assert_eq!(m.hw.ram.len(), 2 * 128 * 1024);
}

#[test]
fn hp49g_builds_from_flash_images() {
    assert_eq!(
        Machine::new(Model::Hp49g, &[0; 16]).unwrap_err(),
        Error::RomSize {
            expected: 2 * 1024 * 1024,
            actual: 16
        }
    );
    let packed = vec![0x21u8; 2 * 1024 * 1024];
    let m = Machine::new(Model::Hp49g, &packed).unwrap();
    assert_eq!(m.peek(0), 1);
    assert_eq!(m.peek(1), 2);
    assert_eq!(m.hw.keyboard.layout(), crate::io::Layout::Hp49);
    assert_eq!(m.hw.ram.len(), 2 * 256 * 1024);
    // The unpacked form of the same image gives the same machine.
    let unpacked: Vec<u8> = packed.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect();
    let u = Machine::new(Model::Hp49g, &unpacked).unwrap();
    assert_eq!(u.hw.nce1, m.hw.nce1);
    assert_eq!(u.rom_sum, m.rom_sum);
}

#[test]
fn hp49g_flash_banks_and_write_path() {
    use crate::modules::Flash;
    // Bank n starts with nibble n (low view) at chip nibble n * #40000.
    let mut img = vec![0xFFu8; 2 * 1024 * 1024];
    for bank in 0..16 {
        img[bank * 0x2_0000] = bank as u8 | 0xF0;
    }
    let flash = Flash::from_packed(&img).unwrap();
    let mut hw = Hardware::new(Model::Hp49g, Nce1::Flash(Box::new(flash)));
    // Bank 0 in both views at reset.
    assert_eq!(hw.peek(0x00000), 0);
    assert_eq!(hw.peek(0x40000), 0);
    // HDW #100, NCE2 256 KB at #80000, CE1 2 KB at #3F000.
    for v in [0x100, 0xC0000, 0x80000, 0xFF000, 0x3F000] {
        hw.config(v);
    }
    // Sousa's assignment: base + 2*n picks high bank n, base + #20*n low
    // bank n; #58 = #40 + 2*12 gives low bank 2, high bank 12 (wiki:
    // questions/hp49g-bank-latch-bits).
    hw.read_nibble(0x3F058);
    assert_eq!(hw.peek(0x00000), 2);
    assert_eq!(hw.peek(0x40000), 12);
    // CE2 out of the way, NCE3 128 KB at #40000: still RAM until #11C
    // bit 3 opens the flash path.
    for v in [0xFF000, 0x7E000, 0xC0000, 0x40000] {
        hw.config(v);
    }
    assert!(hw.mc.all_configured());
    hw.write_nibble(0x40100, 0x3);
    assert_eq!(hw.peek(0x40100), 0x3, "NCE3 RAM");
    hw.write_nibble(0x11C, 0x8);
    // Through the flash now: array data of high bank 12.
    assert_eq!(hw.peek(0x40000), 12);
    // Program #5A at offset #200 of bank 12: #40 then the data byte,
    // low nibble first; then read status, then read array.
    for (a, v) in [
        (0x40000, 0x0),
        (0x40001, 0x4),
        (0x40200, 0xA),
        (0x40201, 0x5),
    ] {
        hw.write_nibble(a, v);
    }
    assert_eq!(hw.peek(0x40001), 0x8, "status: ready");
    for (a, v) in [(0x40000, 0xF), (0x40001, 0xF)] {
        hw.write_nibble(a, v);
    }
    assert_eq!(hw.peek(0x40200), 0xA);
    assert_eq!(hw.peek(0x40201), 0x5);
    // Closing #11C shuts the gate: writes no longer reach the chip and
    // NCE3 is RAM again.
    hw.write_nibble(0x11C, 0x0);
    assert_eq!(hw.peek(0x40100), 0x3);
    let f = hw.nce1.flash().unwrap();
    assert!(!f.write_enabled());
    assert_eq!(f.bank(12)[0x200], 0xA);
    // NCE3 unconfigured: the high view (NCE1) shows the programmed byte.
    hw.unconfig(0x40000);
    assert_eq!(hw.peek(0x40200), 0xA);
}

#[test]
fn hp49g_profile_hooks() {
    // The 49G cannot boot yet, but its hardware profile can be exercised.
    let mut hw = Hardware::new(Model::Hp49g, Nce1::Rom(Rom::from_nibbles(vec![7; 16])));
    // HDW #100, NCE2 256 KB at #80000, CE1 2 KB at #3F000, CE2 128 KB at
    // #C0000, NCE3 128 KB at #40000.
    for v in [
        0x100, 0x80000, 0x80000, 0xFF000, 0x3F000, 0xC0000, 0xC0000, 0xC0000, 0x40000,
    ] {
        hw.config(v);
    }
    assert!(hw.mc.all_configured());
    // Writes latch on the 49G.
    hw.write_nibble(0x3F05E, 0);
    assert_eq!(hw.bank_latch(), 0x2F);
    // CE2 and NCE3 are built-in RAM.
    hw.write_nibble(0xC0003, 0x9);
    hw.write_nibble(0x40003, 0x6);
    assert_eq!(hw.peek(0xC0003), 0x9);
    assert_eq!(hw.peek(0x40003), 0x6);
    // #11C bit 3 opens the flash path; plain ROM (as in this test)
    // declines it, so NCE3's RAM still answers.
    hw.write_nibble(0x11C, 0x8);
    assert_eq!(hw.io.lcr(), 0x8);
    hw.write_nibble(0x40004, 0x5);
    assert_eq!(hw.peek(0x40004), 0x5);
    // No DA19 wiring: #129 bit 3 does not mask the ROM.
    assert_eq!(hw.peek(0x00200), 7);
}

#[test]
fn hp39g_builds_from_packed_and_unpacked_images() {
    assert_eq!(
        Machine::new(Model::Hp39g, &[0; 16]).unwrap_err(),
        Error::RomSize {
            expected: 1024 * 1024,
            actual: 16
        }
    );
    // Bank n starts with nibble n; the I/O window of an upload holds 5s.
    let mut unpacked = vec![0u8; 2 * 1024 * 1024];
    for bank in 0..8 {
        unpacked[bank * 0x4_0000] = bank as u8;
    }
    unpacked[0x100..0x140].fill(5);
    let u = Machine::new(Model::Hp39g, &unpacked).unwrap();
    let packed: Vec<u8> = unpacked.chunks(2).map(|p| p[0] | (p[1] << 4)).collect();
    let p = Machine::new(Model::Hp40g, &packed).unwrap();
    assert_eq!(u.hw.nce1, p.hw.nce1);
    // The upload's I/O window is zeroed.
    assert!(u.hw.nce1.nibbles()[0x100..0x140].iter().all(|&n| n == 0));
    assert_eq!(u.hw.keyboard.layout(), crate::io::Layout::Hp39);
    assert_eq!(u.hw.ram.len(), 2 * 256 * 1024);
    assert_eq!(u.model().clock_hz(), 4_000_000);
}

#[test]
fn hp39g_banks_through_the_latch_and_40g_strap() {
    let mut nibbles = vec![0u8; 2 * 1024 * 1024];
    for bank in 0..8 {
        nibbles[bank * 0x4_0000] = bank as u8;
    }
    for model in [Model::Hp39g, Model::Hp40g] {
        let rom = Nce1::BankedRom(Rom::from_nibbles(nibbles.clone()));
        let mut hw = Hardware::new(model, rom);
        // HDW #100, NCE2 256 KB at #80000, CE1 4 KB at #7E000: the ROM's
        // own sequence (wiki: hardware/hp39g-40g).
        for v in [0x100, 0x80000, 0x80000, 0xFF000, 0x7E000] {
            hw.config(v);
        }
        // #7E012 latches 9: high view bank 9, a mirror of bank 1.
        hw.read_nibble(0x7E012);
        assert_eq!(hw.bank_latch(), 9);
        assert_eq!(hw.peek(0x40000), 1);
        assert_eq!(hw.peek(0x00000), 0);
        // Writes latch too, as on the 49G; #7E00E gives bank 7.
        hw.write_nibble(0x7E00E, 0);
        assert_eq!(hw.peek(0x40000), 7);
        // No flash path: #11C bit 3 changes nothing.
        hw.write_nibble(0x11C, 0x8);
        assert_eq!(hw.peek(0x40000), 7);
        // The ROM writes #11A; only the 40G reads bit 3 back as 1.
        hw.write_nibble(0x11A, 0x0);
        let strap = if model == Model::Hp40g { 0x8 } else { 0 };
        assert_eq!(hw.peek(0x11A), strap, "{model:?}");
        assert_eq!(hw.read_nibble(0x11A), strap, "{model:?}");
        assert_eq!(hw.peek(0x11B), 0);
    }
}

#[test]
fn hp40g_state_round_trip_keeps_its_strap_and_keys() {
    let mut m = Machine::new(Model::Hp40g, &vec![0u8; Model::Hp40g.rom_bytes()]).unwrap();
    m.key_down(crate::io::Key::Aplet).unwrap();
    m.key_down(crate::io::Key::Views).unwrap();
    let saved = m.save_state();
    let mut n = Machine::new(Model::Hp40g, &vec![0u8; Model::Hp40g.rom_bytes()]).unwrap();
    n.load_state(&saved).unwrap();
    assert!(n.hw.keyboard.is_pressed(crate::io::Key::Views));
    assert_eq!(n.save_state(), saved);
    // A 40G state does not load into a 39G.
    let mut g = Machine::new(Model::Hp39g, &vec![0u8; Model::Hp39g.rom_bytes()]).unwrap();
    assert_eq!(g.load_state(&saved), Err(Error::StateModelMismatch));
}

#[test]
fn keys_off_the_model_are_refused() {
    let mut m = Machine::new(Model::Hp49g, &vec![0u8; Model::Hp49g.rom_bytes()]).unwrap();
    assert!(!m.has_key(Key::Prg));
    assert_eq!(
        m.key_down(Key::Prg),
        Err(Error::KeyNotOnModel {
            key: "prg",
            model: "49g"
        })
    );
    assert!(m.key_up(Key::Prg).is_err());
    assert_eq!(m.hw.read_in_lines(), 0, "nothing pressed");
    m.key_down(Key::Apps).unwrap();
    assert!(m.hw.keyboard.is_pressed(Key::Apps));
    let mut sx = machine(&[(MAIN, LOOP)]);
    assert_eq!(
        sx.key_down(Key::Apps).unwrap_err().to_string(),
        "key \"apps\" is not on the 48sx keyboard"
    );
    sx.key_down(Key::Prg).unwrap();
}

#[test]
fn rti_re_enters_while_the_uart_request_is_held() {
    // Handler: a bare RTI; main program loops.
    let mut m = machine(&[(MAIN, LOOP), (INTERRUPT_VECTOR, "0F")]);
    m.hw.config(0x100);
    m.hw.write_nibble(0x10D, 6);
    m.hw.write_nibble(0x110, 0x8 | 0x2); // SON, rx full
    m.serial_push(&[0x42]);
    m.run_cycles(frame_cycles(1)).unwrap();
    // RBF's edge vectored; each RTI re-enters while RBF is unread.
    for _ in 0..3 {
        assert!(m.cpu.regs.in_interrupt);
        assert_eq!(m.cpu.regs.pc, INTERRUPT_VECTOR);
        m.step().unwrap();
    }
    assert_eq!(m.hw.read_nibble(0x114), 0x2);
    m.step().unwrap(); // RTI with the request gone
    assert!(!m.cpu.regs.in_interrupt);
    assert_eq!(m.cpu.regs.pc, MAIN);
}
