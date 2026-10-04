//! Read/write memory module (built-in RAM or a RAM card).

/// A read/write nibble array. Accesses are mirrored modulo its length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ram {
    nibbles: Vec<u8>,
}

impl Ram {
    /// Creates a zeroed RAM of `len_nibbles` nibbles.
    pub fn new(len_nibbles: usize) -> Self {
        Self {
            nibbles: vec![0; len_nibbles],
        }
    }

    /// Reads the nibble at `offset`, mirrored modulo the RAM length. A
    /// zero-length RAM reads 0.
    pub fn read(&self, offset: u32) -> u8 {
        if self.nibbles.is_empty() {
            return 0;
        }
        self.nibbles[offset as usize % self.nibbles.len()]
    }

    /// Writes `nibble` (masked to 4 bits) at `offset`, mirrored modulo the
    /// RAM length. Writes to a zero-length RAM are ignored.
    pub fn write(&mut self, offset: u32, nibble: u8) {
        let len = self.nibbles.len();
        if len == 0 {
            return;
        }
        self.nibbles[offset as usize % len] = nibble & 0xF;
    }

    /// Length in nibbles.
    pub fn len(&self) -> usize {
        self.nibbles.len()
    }

    /// True when the RAM holds no nibbles.
    pub fn is_empty(&self) -> bool {
        self.nibbles.is_empty()
    }

    /// The RAM contents, one nibble per element.
    pub fn as_slice(&self) -> &[u8] {
        &self.nibbles
    }

    /// Mutable access to the RAM contents, one nibble per element. Callers
    /// must keep every element within 0..=15.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.nibbles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_zeroed() {
        let ram = Ram::new(8);
        assert_eq!(ram.len(), 8);
        assert!(ram.as_slice().iter().all(|&n| n == 0));
    }

    #[test]
    fn write_read_mirrors_and_masks() {
        let mut ram = Ram::new(4);
        ram.write(5, 0x1A);
        assert_eq!(ram.read(1), 0xA);
        assert_eq!(ram.read(9), 0xA);
        assert_eq!(ram.as_slice(), &[0, 0xA, 0, 0]);
    }

    #[test]
    fn empty_ram() {
        let mut ram = Ram::new(0);
        assert!(ram.is_empty());
        ram.write(3, 7);
        assert_eq!(ram.read(3), 0);
    }
}
