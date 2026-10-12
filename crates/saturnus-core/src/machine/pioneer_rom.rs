//! The two forms a Lewis ROM image (the Pioneers 42S, 27S, 17B, 17BII and
//! the 28S) comes in (wiki: hardware/saturn-models "ROM image forms").
//!
//! An image is either in **address order** (byte `i` holds the nibbles at
//! #2i and #2i+1) or in **dump order**: cut into 256 equal units (256
//! bytes of a 64 KB image, 384 of a 96 KB one) stored last unit first.
//! One 42S ROM in both forms differs exactly by this reordering, and the
//! owner's 27S, 17B and 17BII images are in dump order (observed on the
//! images; every one of them then boots). Each ROM, the 64 KB main ROM and
//! the 32 KB language ROM of a 96 KB image alike, ends with "COPYRIGHT HP
//! 198x" and its self-test pass text in its last 32 bytes, so the text
//! tells the forms apart: the image's last unit holds it in address order,
//! its first unit in dump order. The 28S's 128 KB image comes in the same
//! two forms (units of 512 bytes; the owner's image is in dump order and
//! boots once reordered); its ROM ends with "Copyright HP 1986, 1987" in
//! mixed case, starting at an odd nibble (observed on the image).

use std::borrow::Cow;

/// The text every Pioneer ROM carries in its last 32 bytes.
const MARK: &[u8] = b"COPYRIGHT HP 198";
/// The text the 28S's ROM carries near its end, from an odd nibble.
const MARK_28S: &[u8] = b"Copyright HP 198";

/// Whether `bytes` hold `mark` from an even nibble (byte-aligned) or from
/// an odd one (each character's low nibble in a byte's high nibble).
fn holds(bytes: &[u8], mark: &[u8]) -> bool {
    let aligned = bytes.windows(mark.len()).any(|w| w == mark);
    let odd = bytes.windows(mark.len() + 1).any(|w| {
        w.windows(2)
            .zip(mark)
            .all(|(p, &c)| (p[0] >> 4) | (p[1] << 4) == c)
    });
    aligned || odd
}

/// Whether `image` (a 64 or 96 KB Pioneer image, or a 128 KB 28S image)
/// is in dump order.
pub fn is_dump_order(image: &[u8]) -> bool {
    if !image.len().is_multiple_of(256) || image.len() < 256 * MARK.len() {
        return false;
    }
    let unit = image.len() / 256;
    let first = &image[..unit];
    holds(first, MARK) || holds(first, MARK_28S)
}

/// `image` in address order: reordered when it is in dump order
/// (its first unit holds the mark), as it is otherwise.
pub fn address_order(image: &[u8]) -> Cow<'_, [u8]> {
    if !is_dump_order(image) {
        return Cow::Borrowed(image);
    }
    let unit = image.len() / 256;
    Cow::Owned(image.chunks_exact(unit).rev().flatten().copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic 64 or 96 KB image in address order: unit `u` filled
    /// with `u`, the mark 31 bytes before the end of each ROM.
    fn synthetic(len: usize) -> Vec<u8> {
        let unit = len / 256;
        let mut v: Vec<u8> = (0..len).map(|i| (i / unit) as u8).collect();
        for end in [0x1_0000, len] {
            v[end - 0x1F..end - 0x1F + MARK.len()].copy_from_slice(MARK);
        }
        v
    }

    #[test]
    fn dump_order_round_trips() {
        for len in [64 * 1024, 96 * 1024] {
            let addr = synthetic(len);
            assert!(!is_dump_order(&addr), "{len}: address order kept");
            assert!(matches!(address_order(&addr), Cow::Borrowed(_)));
            let unit = len / 256;
            let dump: Vec<u8> = addr.chunks_exact(unit).rev().flatten().copied().collect();
            assert!(is_dump_order(&dump), "{len}");
            assert_eq!(address_order(&dump).as_ref(), addr.as_slice());
        }
        assert!(!is_dump_order(&[0; 64 * 1024]), "no mark: as it is");
        assert!(!is_dump_order(b"COPYRIGHT HP 198"));
    }

    #[test]
    fn the_28s_mark_is_found_from_an_odd_nibble() {
        let len = 128 * 1024;
        let unit = len / 256;
        let mut addr: Vec<u8> = (0..len).map(|i| (i / unit) as u8).collect();
        // The mark's nibbles from nibble 2 * (len - 0x40) + 1.
        let start = len - 0x40;
        for (i, &c) in MARK_28S.iter().enumerate() {
            let b = start + i;
            addr[b] = (addr[b] & 0x0F) | (c << 4);
            addr[b + 1] = (addr[b + 1] & 0xF0) | (c >> 4);
        }
        assert!(holds(&addr[len - unit..], MARK_28S));
        assert!(!is_dump_order(&addr));
        let dump: Vec<u8> = addr.chunks_exact(unit).rev().flatten().copied().collect();
        assert!(is_dump_order(&dump));
        assert_eq!(address_order(&dump).as_ref(), addr.as_slice());
    }
}
