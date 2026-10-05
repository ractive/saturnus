//! The HP 48/49 character set as Unicode text.
//!
//! Bytes 0-127 are ASCII and 160-255 ISO 8859-1; 128-159 are HP's math and
//! Greek characters (wiki: protocols/hp-object-format, the same table as
//! hptx-core's charset, checked there against a 48SX string holding every
//! character from 128 to 255).

/// Characters 128-159 in byte order.
const HIGH: [&str; 32] = [
    "∡",
    "x\u{0304}",
    "∇",
    "√",
    "∫",
    "Σ",
    "▶",
    "π",
    "∂",
    "≤",
    "≥",
    "≠",
    "α",
    "→",
    "←",
    "↓",
    "↑",
    "γ",
    "δ",
    "ε",
    "η",
    "θ",
    "λ",
    "ρ",
    "σ",
    "τ",
    "ω",
    "Δ",
    "Π",
    "Ω",
    "■",
    "∞",
];

/// HP bytes as text; every byte has a meaning, so this never fails.
pub fn decode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x80..=0x9F => s.push_str(HIGH[usize::from(b - 0x80)]),
            _ => s.push(char::from(b)),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_characters() {
        assert_eq!(decode(b"AB"), "AB");
        assert_eq!(decode(&[0x8D, 0x85, 0xAB, 0xBB]), "→Σ«»");
        assert_eq!(decode(&[0x81]), "x\u{0304}");
    }
}
