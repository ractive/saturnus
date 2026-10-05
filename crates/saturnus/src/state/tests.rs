use super::*;
use crate::cpu::{Bus, INTERRUPT_VECTOR};
use crate::io::Key;
use crate::io::timers::{CTRL_INT, CTRL_XTRA_OR_RUN};

/// A 48SX-sized ROM: #00000 jumps to #00020, which loops; the interrupt
/// vector is a bare RTI. `salt` changes one unused nibble.
fn rom(salt: u8) -> Vec<u8> {
    let mut nibbles = vec![0u8; Model::Hp48sx.rom_bytes() * 2];
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
/// port 1 configured, display on, TIMER2 interrupting, a key held and a
/// card inserted.
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
    bad[8] = 2;
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

    // The cycle counter sits 33 bytes before the end (three u64, one u32,
    // five u8 follow it, the counter itself included). A value near
    // u64::MAX would overflow `advance`; it is rejected.
    let cycles_at = saved.len() - 33;
    assert_eq!(&saved[cycles_at..cycles_at + 8], &m.cycles().to_le_bytes());
    let mut bad = saved.clone();
    bad[cycles_at..cycles_at + 8].copy_from_slice(&(u64::MAX - 3).to_le_bytes());
    check(&mut m, &bad, "cycle counter");

    let mut other = Machine::new(Model::Hp48sx, &rom(1)).unwrap();
    assert_eq!(other.load_state(&saved), Err(Error::StateRomMismatch));
    assert_eq!(m.save_state(), saved);
}
