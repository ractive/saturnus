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

/// One HP byte as text.
pub fn char_of(b: u8) -> String {
    decode(&[b])
}

/// Text as HP bytes: ASCII and Latin-1 as themselves, the math and Greek
/// characters of 128-159 from their Unicode forms (`x̄` is `x` with a
/// combining macron). `Err` names the first character the set lacks
/// (C1 controls and anything beyond U+00FF that is not one of 128-159).
pub fn encode(text: &str) -> Result<Vec<u8>, char> {
    let mut out = Vec::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'x' && chars.peek() == Some(&'\u{0304}') {
            chars.next();
            out.push(0x81);
            continue;
        }
        let mut buf = [0u8; 4];
        let s: &str = c.encode_utf8(&mut buf);
        if let Some(i) = HIGH.iter().position(|&h| h == s) {
            out.push(0x80 + i as u8);
            continue;
        }
        match u32::from(c) {
            n @ (0..=0x7F | 0xA0..=0xFF) => out.push(n as u8),
            _ => return Err(c),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_is_decode_inverted() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(encode(&decode(&all)).unwrap(), all);
        assert_eq!(
            encode("« 1 2 + »").unwrap(),
            [0xAB, 32, 49, 32, 50, 32, 43, 32, 0xBB]
        );
        assert_eq!(encode("\u{85}"), Err('\u{85}'));
        assert_eq!(encode("€"), Err('€'));
    }

    #[test]
    fn high_characters() {
        assert_eq!(decode(b"AB"), "AB");
        assert_eq!(decode(&[0x8D, 0x85, 0xAB, 0xBB]), "→Σ«»");
        assert_eq!(decode(&[0x81]), "x\u{0304}");
    }
}
