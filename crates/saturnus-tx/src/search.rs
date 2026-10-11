//! What the speed and parity search sends and listens for.
//!
//! The search opens the port at one speed with 8 data bits and no parity,
//! sends a Kermit I packet ([`probe`]) framed for no, odd and even parity
//! in turn (a calculator with parity checks it on receive and ignores the
//! other framings), which the server answers with an ACK and nothing else (wiki: protocols/server-commands, "an I packet is
//! answered"), and asks [`hear`] whether a Kermit packet came in: that
//! ACK, or one of the NAKs an idle server sends now and then (wiki:
//! protocols/server-commands). At the wrong speed the bytes are garbage or
//! nothing at all.
//!
//! Why probe instead of only listening: an idle NAK is not something to
//! count on. Kermit does not oblige a server to NAK while it waits, and
//! saturnus's emulated 48SX, 48GX and 49G send one NAK per server timeout
//! (after `SERVER`, after an I exchange, after half a packet) and then
//! none for at least 120 s, in the native runner (`saturnus run --serial`)
//! as in satx-core's in-process transport (2026-10-11), while the wiki has
//! saturnng's idle server NAKing on. A probe gets an answer from either.
//!
//! With parity on (IOPAR's second field, wiki: protocols/iopar) the HP's
//! frame still has 8 data bits (wiki: hardware/uart), so the parity bit
//! arrives as bit 7 of each byte: the packet reads right once bit 7 is
//! masked, and bit 7 tells odd from even. Mark parity (bit 7 always set)
//! is not reported: neither Web Serial nor the `serialport` crate can set
//! it. Space parity (bit 7 always clear) reads as no parity, which is what
//! a link without parity receives anyway.

use kermit_proto::codec::{BlockCheck, CR, Framing, Packet, parse_frame};
use kermit_proto::params::InitParams;

use crate::device::Parity;

const SOH: u8 = 0x01;

/// The I packet the search sends, framed for a calculator set to
/// `parity`: sequence 0, block check 1 (as every command packet, wiki:
/// protocols/server-commands), our Send-Init parameters without 8th-bit
/// quoting or repeats. With parity the host's port still sends 8 data bits
/// with no parity, so bit 7 of each byte is the parity bit the calculator
/// checks (wiki: protocols/iopar, hardware/uart).
pub fn probe(parity: Parity) -> Vec<u8> {
    let ours = InitParams {
        maxl: 94,
        time: 10,
        npad: 0,
        padc: 0,
        eol: CR,
        qctl: b'#',
        qbin: b'Y',
        chkt: b'1',
        rept: b' ',
    };
    Packet::new(0, b'I', ours.encode())
        .encode(BlockCheck::Type1, &Framing::default())
        .unwrap_or_default()
        .into_iter()
        .map(|b| b | parity_bit(b, parity))
        .collect()
}

/// The parity of the first Kermit packet in `bytes` heard on a link with 8
/// data bits and no parity: [`Parity::None`] when it reads as is, odd or
/// even when it reads with bit 7 masked and bit 7 is that parity bit of
/// every byte; `None` when no packet is in there.
pub fn hear(bytes: &[u8]) -> Option<Parity> {
    if packets(bytes).next().is_some() {
        return Some(Parity::None);
    }
    let low: Vec<u8> = bytes.iter().map(|b| b & 0x7F).collect();
    packets(&low).find_map(|range| {
        let frame = bytes.get(range)?;
        [Parity::Odd, Parity::Even].into_iter().find(|&parity| {
            frame
                .iter()
                .all(|&b| b & 0x80 == parity_bit(b & 0x7F, parity))
        })
    })
}

/// Bit 7 of `low` (7 data bits) sent with `parity`.
fn parity_bit(low: u8, parity: Parity) -> u8 {
    let odd_ones = low.count_ones() % 2 == 1;
    let set = match parity {
        Parity::Odd => !odd_ones,
        Parity::Even => odd_ones,
        Parity::None => false,
    };
    if set { 0x80 } else { 0 }
}

/// The byte ranges of the valid Kermit packets in `bytes` (any block
/// check), from the SOH through the check.
fn packets(bytes: &[u8]) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
    bytes
        .iter()
        .enumerate()
        .filter(|&(_, &b)| b == SOH)
        .filter_map(move |(at, _)| {
            let len = usize::from(bytes.get(at + 1)?.wrapping_sub(32));
            let range = at..at + 2 + len;
            let frame = bytes.get(range.clone())?;
            [BlockCheck::Type1, BlockCheck::Type2, BlockCheck::Type3]
                .into_iter()
                .any(|check| parse_frame(frame, check).is_ok())
                .then_some(range)
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use kermit_proto::codec::{Framing, Packet};

    fn nak() -> Vec<u8> {
        Packet::new(0, b'N', Vec::new())
            .encode(BlockCheck::Type1, &Framing::default())
            .unwrap()
    }

    fn with_parity(bytes: &[u8], parity: Parity) -> Vec<u8> {
        bytes.iter().map(|&b| b | parity_bit(b, parity)).collect()
    }

    #[test]
    fn the_probe_is_an_i_packet() {
        let plain = probe(Parity::None);
        let p = parse_frame(&plain[..plain.len() - 1], BlockCheck::Type1).unwrap();
        assert_eq!((p.seq, p.kind), (0, b'I'));
        assert_eq!(hear(&plain), Some(Parity::None));
        for parity in [Parity::Odd, Parity::Even] {
            assert_eq!(probe(parity), with_parity(&plain, parity));
            assert_eq!(hear(&probe(parity)), Some(parity));
        }
    }

    #[test]
    fn a_nak_as_is_is_no_parity() {
        let mut heard = b"\x00\xff".to_vec();
        heard.extend(nak());
        assert_eq!(hear(&heard), Some(Parity::None));
    }

    #[test]
    fn odd_and_even_parity_are_told_apart() {
        for parity in [Parity::Odd, Parity::Even] {
            let heard = with_parity(&nak(), parity);
            assert_ne!(heard, nak(), "{parity:?} changes some byte");
            assert_eq!(hear(&heard), Some(parity));
        }
    }

    #[test]
    fn garbage_and_silence_are_nothing() {
        assert_eq!(hear(b""), None);
        assert_eq!(hear(&[0x80, 0x00, 0xf8, 0x78, 0x80, 0x01, 0x23]), None);
        // Mark parity: bit 7 on every byte.
        let mark: Vec<u8> = nak().iter().map(|b| b | 0x80).collect();
        assert_eq!(hear(&mark), None);
        // Half a packet.
        let nak = nak();
        assert_eq!(hear(&nak[..nak.len() - 2]), None);
    }
}
