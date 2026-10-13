//! The two forms a Lewis ROM image (the Pioneers 42S, 27S, 17B, 17BII and
//! the 28S) comes in (wiki: hardware/saturn-models "ROM image forms").
//!
//! An image is either in **address order** (byte `i` holds the nibbles at
//! #2i and #2i+1) or in **dump order**: cut into 256 equal units (256
//! bytes of a 64 KB image, 384 of a 96 KB one, 512 of the 28S's 128 KB)
//! stored last unit first. One 42S ROM in both forms differs exactly by
//! this reordering, and the owner's 27S, 17B, 17BII and 28S images are in
//! dump order (observed on the images; every one of them then boots).
//!
//! The Lewis CRC tells the forms apart: every Lewis ROM's first 64 KB
//! (#00000-#1FFFF) read in address order gives #FFFF, the value its
//! self-test wants (wiki: hardware/lewis "CRC"). An image for which
//! neither order gives #FFFF (a damaged dump, a test image) falls back to
//! the Pioneers' text: each of their ROMs ends with "COPYRIGHT HP 198x"
//! in its last 32 bytes, so the image's first unit holds it in dump
//! order. The 28S's ROM has no such text, only the CRC.

use std::borrow::Cow;

/// The text every Pioneer ROM carries in its last 32 bytes.
const MARK: &[u8] = b"COPYRIGHT HP 198";
/// Bytes the CRC test reads: #00000-#1FFFF.
const CRC_BYTES: usize = 64 * 1024;
/// What a good Lewis ROM's CRC over [`CRC_BYTES`] gives.
const CRC_GOOD: u16 = 0xFFFF;

/// The Lewis CRC of the first 64 KB of `image` read in address order,
/// low nibble of each byte first.
fn rom_crc(image: impl Iterator<Item = u8>) -> u16 {
    image.take(CRC_BYTES).fold(0, |crc, b| {
        let crc = crate::io::registers::lewis_crc_step(crc, b & 0xF);
        crate::io::registers::lewis_crc_step(crc, b >> 4)
    })
}

/// `image`'s bytes in the other order (dump order read as address order
/// and back: the reordering is its own inverse).
fn reordered(image: &[u8]) -> impl Iterator<Item = u8> + '_ {
    image
        .chunks_exact(image.len() / 256)
        .rev()
        .flatten()
        .copied()
}

/// Whether `image` (a 64 or 96 KB Pioneer image, or a 128 KB 28S image)
/// is in dump order.
pub fn is_dump_order(image: &[u8]) -> bool {
    if !image.len().is_multiple_of(256) || image.len() < CRC_BYTES {
        return false;
    }
    if rom_crc(image.iter().copied()) == CRC_GOOD {
        return false;
    }
    if rom_crc(reordered(image)) == CRC_GOOD {
        return true;
    }
    let unit = image.len() / 256;
    image[..unit].windows(MARK.len()).any(|w| w == MARK)
}

/// `image` in address order: reordered when it is in dump order, as it
/// is otherwise.
pub fn address_order(image: &[u8]) -> Cow<'_, [u8]> {
    if !is_dump_order(image) {
        return Cow::Borrowed(image);
    }
    Cow::Owned(reordered(image).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic image in address order whose first 64 KB pass the
    /// Lewis CRC: varied bytes, the last four nibbles of #00000-#1FFFF
    /// chosen to make the CRC #FFFF (after four zero nibbles the register
    /// holds the prefix times x^16; adding the wanted value's nibbles
    /// instead gives that XOR the value).
    fn crc_good(len: usize) -> Vec<u8> {
        let mut v: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
        v[CRC_BYTES - 2] = 0;
        v[CRC_BYTES - 1] = 0;
        let x = rom_crc(v.iter().copied()) ^ CRC_GOOD;
        // Nibbles at #1FFFC-#1FFFF, fed in that order, x's top nibble first.
        v[CRC_BYTES - 2] = ((x >> 12) as u8) | (((x >> 8) as u8 & 0xF) << 4);
        v[CRC_BYTES - 1] = ((x >> 4) as u8 & 0xF) | ((x as u8 & 0xF) << 4);
        assert_eq!(rom_crc(v.iter().copied()), CRC_GOOD);
        v
    }

    #[test]
    fn the_crc_tells_the_orders_apart() {
        for len in [64 * 1024, 96 * 1024, 128 * 1024] {
            let addr = crc_good(len);
            assert!(!is_dump_order(&addr), "{len}: address order kept");
            assert!(matches!(address_order(&addr), Cow::Borrowed(_)));
            let dump: Vec<u8> = reordered(&addr).collect();
            assert!(is_dump_order(&dump), "{len}");
            assert_eq!(address_order(&dump).as_ref(), addr.as_slice());
        }
    }

    #[test]
    fn without_a_good_crc_the_text_decides() {
        let len = 64 * 1024;
        let unit = len / 256;
        let mut addr: Vec<u8> = (0..len).map(|i| (i / unit) as u8).collect();
        addr[len - 0x1F..len - 0x1F + MARK.len()].copy_from_slice(MARK);
        assert!(!is_dump_order(&addr));
        let dump: Vec<u8> = reordered(&addr).collect();
        assert!(is_dump_order(&dump));
        assert!(!is_dump_order(&[0; 64 * 1024]), "neither: as it is");
        assert!(!is_dump_order(b"COPYRIGHT HP 198"));
    }
}
