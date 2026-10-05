use super::*;
use crate::cpu::{Bus, INTERRUPT_VECTOR};
use crate::io::Key;
use crate::io::timers::{CTRL_INT, CTRL_XTRA_OR_RUN};

/// A 48SX-sized ROM: #00000 jumps to #00020, which loops; the interrupt
/// vector is a bare RTI. `salt` changes one unused nibble.
fn rom(salt: u8) -> Vec<u8> {
    rom_for(Model::Hp48sx, salt)
}

/// As [`rom`], sized for `model`.
fn rom_for(model: Model, salt: u8) -> Vec<u8> {
    let mut nibbles = vec![0u8; model.rom_bytes() * 2];
    for (i, c) in "6F10".chars().enumerate() {
        nibbles[i] = c.to_digit(16).unwrap() as u8;
    }
    nibbles[INTERRUPT_VECTOR as usize] = 0;
    nibbles[INTERRUPT_VECTOR as usize + 1] = 0xF;
    for (i, c) in "6FFF".chars().enumerate() {
        nibbles[0x20 + i] = c.to_digit(16).unwrap() as u8;
    }
    nibbles[0x1000] = salt;
    nibbles.chunks(2).map(|p| p[0] | (p[1] << 4)).collect()
}

/// A running machine with most state away from power-on: HDW, RAM and
/// port 1 configured, display on, TIMER2 interrupting, a key held, a card
/// inserted, and the UART on at 9600 baud with bytes on the wire in both
/// directions.
fn busy_machine() -> Machine {
    let mut m = Machine::new(Model::Hp48sx, &rom(0)).unwrap();
    for v in [0x100, 0xF0000, 0x70000, 0xC0000, 0x80000] {
        m.hw.config(v);
    }
    m.hw.write_nibble(0x100, 0x8);
    m.hw.write_nibble(0x101, 0xB);
    m.hw.write_nibble(0x10E, 0xC);
    m.hw.io.timers.t2 = 5000;
    m.hw.write_nibble(0x12F, CTRL_XTRA_OR_RUN | CTRL_INT);
    m.hw.write_out(0x1FF);
    m.key_down(Key::Seven);
    m.insert_card(Port::One, &[0x5A; 1024]).unwrap();
    m.hw.write_nibble(0x10D, 6);
    m.hw.write_nibble(0x110, 0xB);
    m.serial_push(b"saturnus serial line");
    m.hw.write_nibble(0x116, 0x1);
    m.hw.write_nibble(0x117, 0x4);
    for a in 0..40u32 {
        m.hw.write_nibble(0x70000 + a * 7, (a & 0xF) as u8);
    }
    m.run_cycles(10_000).unwrap();
    m
}

#[test]
fn round_trip_resumes_identically() {
    let mut m = busy_machine();
    let saved = m.save_state();
    assert_eq!(&saved[..8], b"SATURNUS");
    m.run_cycles(50_000).unwrap();
    let after = m.save_state();
    assert_ne!(after, saved);
    m.load_state(&saved).unwrap();
    assert_eq!(m.save_state(), saved);
    m.run_cycles(50_000).unwrap();
    assert_eq!(m.save_state(), after);
    assert_eq!(m.card_image(Port::One).unwrap(), vec![0x5A; 1024]);
}

#[test]
fn loads_into_a_fresh_machine() {
    let m = busy_machine();
    let saved = m.save_state();
    let mut fresh = Machine::new(Model::Hp48sx, &rom(0)).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.save_state(), saved);
    assert_eq!(fresh.cpu.regs, m.cpu.regs);
    assert_eq!(fresh.hw, m.hw);
    assert_eq!(fresh.cycles(), m.cycles());
}

#[test]
fn truncated_input_is_an_error() {
    let mut m = busy_machine();
    let saved = m.save_state();
    let lens = (0..256).chain((256..saved.len()).step_by(61));
    for len in lens {
        let err = m.load_state(&saved[..len]).unwrap_err();
        assert!(
            matches!(err, Error::InvalidState { .. }),
            "prefix {len}: {err:?}"
        );
    }
    assert_eq!(
        m.save_state(),
        saved,
        "failed loads leave the machine alone"
    );
}

#[test]
fn malformed_input_is_rejected() {
    let mut m = busy_machine();
    let saved = m.save_state();
    let check = |m: &mut Machine, bytes: &[u8], want: &str| {
        let err = m.load_state(bytes).unwrap_err();
        assert!(err.to_string().contains(want), "{err} lacks {want:?}");
    };

    let mut bad = saved.clone();
    bad[0] = b'X';
    check(&mut m, &bad, "bad magic");

    let mut bad = saved.clone();
    bad[8] = 9;
    check(&mut m, &bad, "version");

    let mut bad = saved.clone();
    bad[10] = 7;
    assert_eq!(m.load_state(&bad), Err(Error::StateModelMismatch));

    let mut bad = saved.clone();
    bad.push(0);
    check(&mut m, &bad, "trailing");

    // P is the first byte after A-D, R0-R4 and D0, D1, PC.
    let p_at = 19 + 9 * 8 + 3 * 4;
    assert_eq!(saved[p_at], m.cpu.regs.p);
    let mut bad = saved.clone();
    bad[p_at] = 16;
    check(&mut m, &bad, "P above 15");

    // Carry follows P, ST and HST.
    let mut bad = saved.clone();
    bad[p_at + 4] = 2;
    check(&mut m, &bad, "boolean");

    // A RAM block of the wrong length (first byte after the 5 x 14
    // controller bytes and the CPU block).
    let ram_len_at = p_at + 1 + 2 + 1 + 1 + 1 + 2 + 2 + 8 * 4 + 3 + 5 * 14;
    assert_eq!(
        &saved[ram_len_at..ram_len_at + 4],
        &0x10000u32.to_le_bytes()
    );
    let mut bad = saved.clone();
    bad[ram_len_at..ram_len_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    check(&mut m, &bad, "nibble block length");

    // The cycle counter sits 34 bytes before the end (three u64, one u32,
    // six u8 follow it, the counter itself included). A value near
    // u64::MAX would overflow `advance`; it is rejected.
    let cycles_at = saved.len() - 34;
    assert_eq!(&saved[cycles_at..cycles_at + 8], &m.cycles().to_le_bytes());
    let mut bad = saved.clone();
    bad[cycles_at..cycles_at + 8].copy_from_slice(&(u64::MAX - 3).to_le_bytes());
    check(&mut m, &bad, "cycle counter");

    let mut other = Machine::new(Model::Hp48sx, &rom(1)).unwrap();
    assert_eq!(other.load_state(&saved), Err(Error::StateRomMismatch));
    assert_eq!(m.save_state(), saved);
}

#[test]
fn gx_state_round_trip_keeps_latch_and_banked_card() {
    let mut m = Machine::new(Model::Hp48gx, &rom_for(Model::Hp48gx, 0)).unwrap();
    for v in [
        0x100, 0xC0000, 0x80000, 0xFF000, 0x7F000, 0xFF000, 0x7E000, 0xC0000, 0xC0000,
    ] {
        m.hw.config(v);
    }
    let mut card = vec![0u8; 256 * 1024];
    card[128 * 1024] = 0x65;
    m.insert_card(Port::Two, &card).unwrap();
    m.hw.read_data(0x7F042);
    assert_eq!(m.peek(0xC0000), 5);
    m.run_cycles(10_000).unwrap();
    let saved = m.save_state();
    assert_eq!(saved[10], 1, "model code");

    let mut fresh = Machine::new(Model::Hp48gx, &rom_for(Model::Hp48gx, 0)).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.hw, m.hw);
    assert_eq!(fresh.hw.bank_latch(), 0x21);
    assert_eq!(fresh.peek(0xC0000), 5);
    assert_eq!(fresh.save_state(), saved);

    let mut sx = Machine::new(Model::Hp48sx, &rom(0)).unwrap();
    assert_eq!(sx.load_state(&saved), Err(Error::StateModelMismatch));
}

/// 49G: programmed flash, lock-bits, the flash's read mode, the bank latch
/// and an 8-bit keyboard row (APPS is IN #80) survive a round trip, and
/// the flash write gate follows the restored #11C.
#[test]
fn hp49g_round_trip_keeps_flash() {
    let rom = rom_for(Model::Hp49g, 3);
    let mut m = Machine::new(Model::Hp49g, &rom).unwrap();
    // HDW, NCE2 256 KB at #80000, CE1 2 KB at #3F000, CE2 out of the
    // way, NCE3 128 KB at #40000 (the flash write path).
    for v in [
        0x100, 0x80000, 0x80000, 0xFF000, 0x3F000, 0xFF000, 0x7E000, 0xC0000, 0x40000,
    ] {
        m.hw.config(v);
    }
    m.hw.read_nibble(0x3F002); // high bank 1
    m.hw.write_nibble(0x11C, 0x8);
    // Program #00 at offset #10 of bank 1, leave the chip in status mode.
    for (a, v) in [
        (0x40000, 0x0),
        (0x40001, 0x4),
        (0x40010, 0x0),
        (0x40011, 0x0),
    ] {
        m.hw.write_nibble(a, v);
    }
    if let crate::modules::Nce1::Flash(f) = &mut m.hw.nce1 {
        f.set_lock_bits(0b100);
    }
    m.key_down(Key::Apps);
    let saved = m.save_state();

    let mut fresh = Machine::new(Model::Hp49g, &rom).unwrap();
    fresh.load_state(&saved).unwrap();
    assert_eq!(fresh.save_state(), saved);
    let f = fresh.hw.nce1.flash().unwrap();
    assert_eq!(f.bank(1)[0x10], 0);
    assert_eq!(f.lock_bits(), 0b100);
    assert_eq!(f.read_mode(), crate::modules::flash::ReadMode::Status);
    assert!(f.write_enabled(), "gate follows #11C bit 3");
    assert_eq!(fresh.hw.bank_latch(), 0x01);
    assert!(fresh.hw.keyboard.is_pressed(Key::Apps));

    // A 48 state does not load into the 49G.
    let sx = Machine::new(Model::Hp48sx, &rom_for(Model::Hp48sx, 3)).unwrap();
    assert_eq!(
        fresh.load_state(&sx.save_state()),
        Err(Error::StateModelMismatch)
    );
}
