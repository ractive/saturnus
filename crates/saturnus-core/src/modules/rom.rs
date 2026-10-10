//! Read-only memory module (the NCE1 system ROM or a ROM card).

/// A read-only nibble array. Reads are mirrored modulo its length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rom {
    nibbles: Vec<u8>,
}

impl Rom {
    /// Builds a ROM from a packed image: each byte holds two nibbles, the
    /// nibble at the even address in the low half (wiki: emulators/emu48,
    /// "packed" images). The result has `2 * bytes.len()` nibbles.
    pub fn from_packed(bytes: &[u8]) -> Self {
        let nibbles = bytes.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect();
        Self { nibbles }
    }

    /// Builds a ROM from one nibble per element; each value is masked to
    /// 4 bits.
    pub fn from_nibbles(mut nibbles: Vec<u8>) -> Self {
        for n in &mut nibbles {
            *n &= 0xF;
        }
        Self { nibbles }
    }

    /// Reads the nibble at `addr`, mirrored modulo the ROM length. An empty
    /// ROM reads 0.
    pub fn read(&self, addr: u32) -> u8 {
        if self.nibbles.is_empty() {
            return 0;
        }
        self.nibbles[addr as usize % self.nibbles.len()]
    }

    /// The ROM contents, one nibble per element.
    pub fn as_slice(&self) -> &[u8] {
        &self.nibbles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_low_nibble_first() {
        let rom = Rom::from_packed(&[0x32, 0x96]);
        assert_eq!(rom.as_slice(), &[2, 3, 6, 9]);
        assert_eq!(rom.as_slice().len(), 4);
    }

    #[test]
    fn reads_mirror() {
        let rom = Rom::from_packed(&[0x32, 0x96]);
        assert_eq!(rom.read(0), 2);
        assert_eq!(rom.read(3), 9);
        assert_eq!(rom.read(4), 2);
        assert_eq!(rom.read(7), 9);
        assert_eq!(rom.read(0xF_FFFF), 9);
    }

    #[test]
    fn from_nibbles_masks() {
        let rom = Rom::from_nibbles(vec![0x1F, 0xA0]);
        assert_eq!(rom.as_slice(), &[0xF, 0x0]);
    }

    #[test]
    fn empty_reads_zero() {
        let rom = Rom::from_nibbles(Vec::new());
        assert!(rom.as_slice().is_empty());
        assert_eq!(rom.read(123), 0);
    }
}
