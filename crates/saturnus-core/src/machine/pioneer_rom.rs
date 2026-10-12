//! The two forms a Pioneer ROM image (42S, 27S, 17B, 17BII) comes in
//! (wiki: hardware/saturn-models "ROM image forms").
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
//! its first unit in dump order.

use std::borrow::Cow;

/// The text every Pioneer ROM carries in its last 32 bytes.
const MARK: &[u8] = b"COPYRIGHT HP 198";

/// Whether `image` (a 64 or 96 KB Pioneer image) is in dump order.
pub fn is_dump_order(image: &[u8]) -> bool {
    if !image.len().is_multiple_of(256) || image.len() < 256 * MARK.len() {
        return false;
    }
    let unit = image.len() / 256;
    let first = &image[..unit];
    first.windows(MARK.len()).any(|w| w == MARK)
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
}
