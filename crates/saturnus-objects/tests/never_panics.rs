//! `decode` and `object_size` never panic on damaged objects: real
//! objects (reals, strings, lists, a directory, nested composites)
//! truncated, bit-flipped, with a length field inflated or nibbles
//! appended, from a fixed-seed xorshift (the same cases every run, no
//! dependency, no ROM). Decoded both without memory and over memory of
//! random nibbles, so ROM pointers lead somewhere.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use saturnus_objects::object::{Memory, NoMemory, decode};
use saturnus_objects::prolog::object_size;

/// xorshift64: deterministic, good enough to pick offsets and nibbles.
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

    fn nibble(&mut self) -> u8 {
        (self.next() & 0xF) as u8
    }
}

/// 1 MB of address space with nibbles from `seed`.
struct Noise(Vec<u8>);

impl Memory for Noise {
    fn nibble(&self, addr: u32) -> Option<u8> {
        self.0.get(addr as usize % self.0.len()).copied()
    }
}

/// Nibbles from hex in memory order.
fn nib(s: &str) -> Vec<u8> {
    s.chars().map(|c| c.to_digit(16).unwrap() as u8).collect()
}

/// Objects as the calculators store them (the crate's own fixtures).
const SEEDS: [&str; 10] = [
    "339207997389453562780",
    "339200070000000000519",
    "C2A2011000849602BA87BB",
    "47A20CFA201045D13A2B2130",
    "47A209C2A2D13A200001B2130",
    "E4A2051000FF00000000000000",
    "47A209C2A2C2A207000014B2130",
    "416209100098765432109876543219",
    "7792000000000000000100000000000000029",
    "69A20FF7A00000000010A510339200000000000000050",
];

/// The seeds and lists of them, nested two deep.
fn corpus() -> Vec<Vec<u8>> {
    let mut all: Vec<Vec<u8>> = SEEDS.iter().map(|s| nib(s)).collect();
    let list = |items: &[Vec<u8>]| {
        let mut l = nib("47A20");
        for i in items {
            l.extend_from_slice(i);
        }
        l.extend(nib("B2130"));
        l
    };
    let flat = list(&all);
    let nested = list(&[flat.clone(), all[2].clone(), flat.clone()]);
    all.push(flat);
    all.push(nested);
    all
}

/// One damaged copy of `obj`.
fn mutate(rng: &mut Rng, obj: &[u8]) -> Vec<u8> {
    let mut d = obj.to_vec();
    match rng.below(5) {
        0 => d.truncate(rng.below(d.len())),
        1 => {
            for _ in 0..=rng.below(6) {
                let i = rng.below(d.len());
                d[i] ^= 1 << rng.below(4);
            }
        }
        2 => {
            // A five-nibble length or count made huge.
            let at = rng.below(d.len());
            let big = if rng.below(2) == 0 {
                0xFFFFF
            } else {
                rng.next() & 0xFFFFF
            };
            for k in 0..5 {
                if let Some(slot) = d.get_mut(at + k) {
                    *slot = ((big >> (4 * k)) & 0xF) as u8;
                }
            }
        }
        3 => {
            for _ in 0..=rng.below(32) {
                d.push(rng.nibble());
            }
        }
        _ => {
            // A prolog swapped in anywhere.
            let at = rng.below(d.len());
            let pick = nib(SEEDS[rng.below(SEEDS.len())]);
            for (k, p) in pick.iter().take(5).enumerate() {
                if let Some(slot) = d.get_mut(at + k) {
                    *slot = *p;
                }
            }
        }
    }
    d
}

#[test]
fn damaged_objects_never_panic() {
    let mut rng = Rng(0x000B_1EC7);
    let noise = Noise((0..1 << 20).map(|_| rng.nibble()).collect());
    let corpus = corpus();
    for (k, obj) in corpus.iter().enumerate() {
        decode(obj, &NoMemory).unwrap_or_else(|e| panic!("seed {k}: {e}"));
        object_size(obj, 0).unwrap_or_else(|e| panic!("seed {k}: {e}"));
    }
    for _ in 0..40_000 {
        let obj = &corpus[rng.below(corpus.len())];
        let bad = mutate(&mut rng, obj);
        let _ = object_size(&bad, 0);
        let _ = object_size(&bad, rng.below(bad.len() + 1));
        let _ = decode(&bad, &NoMemory);
        let _ = decode(&bad, &noise);
    }
}
