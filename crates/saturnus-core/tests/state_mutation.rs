//! `Machine::load_state` never panics on a damaged state, and one it
//! refuses changes nothing: saved states of several models, truncated,
//! bit-flipped, with a length field inflated or bytes appended, from a
//! fixed-seed xorshift (the same cases every run, no dependency, no ROM).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use saturnus_core::{Machine, Model, Port};

/// xorshift64: deterministic, good enough to pick offsets and bits.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// One damaged copy of `state`.
fn mutate(rng: &mut Rng, state: &[u8]) -> Vec<u8> {
    let mut d = state.to_vec();
    match rng.below(5) {
        0 => d.truncate(rng.below(d.len())),
        1 => {
            for _ in 0..=rng.below(8) {
                let i = rng.below(d.len());
                d[i] ^= 1 << rng.below(8);
            }
        }
        2 => {
            // A length or count field made huge: four or eight bytes of
            // ones, or a large random value, little-endian.
            let at = rng.below(d.len());
            let big = match rng.below(3) {
                0 => u64::MAX,
                1 => u64::from(u32::MAX),
                _ => rng.next() | (1 << 31),
            };
            let width = if rng.below(2) == 0 { 4 } else { 8 };
            for (k, b) in big.to_le_bytes().iter().take(width).enumerate() {
                if let Some(slot) = d.get_mut(at + k) {
                    *slot = *b;
                }
            }
        }
        3 => {
            for _ in 0..=rng.below(64) {
                d.push(rng.next() as u8);
            }
        }
        _ => {
            // A random byte anywhere (a small count made off by some).
            let i = rng.below(d.len());
            d[i] = rng.next() as u8;
        }
    }
    d
}

fn rom(model: Model) -> Vec<u8> {
    vec![0u8; model.rom_bytes()]
}

fn check(model: Model, cases: usize, seed: u64) {
    let mut m = Machine::new(model, &rom(model)).unwrap();
    if model.card_max_bytes(Port::One) > 0 {
        m.insert_card(Port::One, &[0x5A; 1024]).unwrap();
    }
    let saved = m.save_state();
    let mut rng = Rng(seed);
    for case in 0..cases {
        let bad = mutate(&mut rng, &saved);
        match m.load_state(&bad) {
            Err(_) => assert!(
                m.save_state() == saved,
                "{model:?} case {case}: a refused state changed the machine"
            ),
            Ok(()) => m.load_state(&saved).unwrap(),
        }
    }
}

#[test]
fn damaged_48sx_states_never_panic() {
    check(Model::Hp48sx, 3000, 0x5A7E_0001);
}

#[test]
fn damaged_48gx_states_never_panic() {
    check(Model::Hp48gx, 2000, 0x5A7E_0002);
}

#[test]
fn damaged_49g_states_never_panic() {
    check(Model::Hp49g, 300, 0x5A7E_0003);
}

#[test]
fn damaged_lewis_states_never_panic() {
    check(Model::Hp42s, 2000, 0x5A7E_0004);
    check(Model::Hp28s, 2000, 0x5A7E_0005);
}
