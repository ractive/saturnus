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
            Err(Error::CardSize { actual: n })
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
    let mut m = lcd_machine();
    m.step().unwrap();
    let off: u64 = (0..1000).map(|_| u64::from(m.step().unwrap())).sum();
    set_io(&mut m, 0x00, 0x8, 1);
    let on: u64 = (0..1000).map(|_| u64::from(m.step().unwrap())).sum();
    assert_eq!(on, off * 113 / 100);
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
