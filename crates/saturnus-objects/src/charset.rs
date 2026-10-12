//! The HP 48/49 character set as Unicode text, and the ASCII trigraphs of
//! the transfer translation modes. The one table of the workspace:
//! satx-core's `charset` wraps it.
//!
//! Bytes 0-127 are ASCII and 160-255 ISO 8859-1; 128-159 are HP's math and
//! Greek characters (wiki: protocols/hp-object-format).
//!
//! In ASCII transfers with translation mode 2 or 3 (`TRANSIO`) the
//! calculator writes characters 128-255 as backslash trigraphs: `\->` for
//! `→`, `\<<` for `«`, `\160` for a no-break space. The tables were
//! checked against a 48SX GET with `T(3)` of a string holding every
//! character from 128 to 255 (2026-10-05). The calculator does not read
//! trigraphs in a host command (`C` packet) itself: `\->LIST` there is a
//! syntax error, so [`encode_command`] translates them before sending.

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

/// The ASCII trigraphs of characters 128-159, in byte order: the
/// calculator's translation modes 2 and 3 (`TRANSIO`) write them in ASCII
/// transfers (wiki: protocols/hp-object-format).
const HIGH_TRIGRAPHS: [&str; 32] = [
    "\\<)", "\\x-", "\\.V", "\\v/", "\\.S", "\\GS", "\\|>", "\\pi", "\\.d", "\\<=", "\\>=", "\\=/",
    "\\Ga", "\\->", "\\<-", "\\|v", "\\|^", "\\Gg", "\\Gd", "\\Ge", "\\Gn", "\\Gh", "\\Gl", "\\Gr",
    "\\Gs", "\\Gt", "\\Gw", "\\GD", "\\PI", "\\GW", "\\[]", "\\oo",
];

/// Characters 160-255 with a mnemonic trigraph; the others are `\nnn`.
const LATIN_TRIGRAPHS: [(u8, &str); 8] = [
    (171, "\\<<"),
    (176, "\\^o"),
    (181, "\\Gm"),
    (187, "\\>>"),
    (215, "\\.x"),
    (216, "\\O/"),
    (223, "\\Gb"),
    (247, "\\:-"),
];

/// [`encode`] for text typed in ASCII, such as a Kermit host command:
/// the trigraphs (`\->`, `\<<`, `\GS`, `\160`) become their bytes, since
/// the calculator does not read them in a host command itself. A
/// backslash that starts no trigraph stays a backslash.
pub fn encode_command(text: &str) -> Result<Vec<u8>, char> {
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('\\') {
        out.extend(encode(&rest[..pos])?);
        let tail = &rest[pos..];
        match trigraph_at(tail) {
            Some((byte, len)) => {
                out.push(byte);
                rest = &tail[len..];
            }
            None => {
                out.push(b'\\');
                rest = &tail[1..];
            }
        }
    }
    out.extend(encode(rest)?);
    Ok(out)
}

/// The byte and length of the trigraph that starts `s` (at a backslash),
/// if any: a mnemonic (`\->`) or a code of 128-255 (`\160`).
pub fn trigraph_at(s: &str) -> Option<(u8, usize)> {
    if let Some(head) = s.get(..3) {
        if let Some(i) = HIGH_TRIGRAPHS.iter().position(|t| *t == head) {
            return Some((0x80 + i as u8, 3));
        }
        if let Some((b, _)) = LATIN_TRIGRAPHS.iter().find(|(_, t)| *t == head) {
            return Some((*b, 3));
        }
    }
    let digits = s.get(1..4)?;
    if !digits.bytes().all(|d| d.is_ascii_digit()) {
        return None;
    }
    let n: u16 = digits.parse().ok()?;
    u8::try_from(n).ok().filter(|b| *b >= 128).map(|b| (b, 4))
}

/// A character with a mnemonic trigraph: its code, its Unicode text and the
/// ASCII trigraph that types it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedChar {
    /// The byte in the HP character set (128-255).
    pub code: u8,
    /// The character as Unicode text (`x̄` is two code points).
    pub text: String,
    /// The trigraph, e.g. `\->`.
    pub trigraph: &'static str,
}

/// Every character with a mnemonic trigraph (all of 128-159 and some of
/// 160-255), by code. The other codes 160-255 are typed as `\nnn`, three
/// decimal digits.
pub fn named_chars() -> Vec<NamedChar> {
    let high = (0x80u8..)
        .zip(HIGH.iter().zip(HIGH_TRIGRAPHS))
        .map(|(code, (text, trigraph))| NamedChar {
            code,
            text: (*text).to_string(),
            trigraph,
        });
    let latin = LATIN_TRIGRAPHS.iter().map(|&(code, trigraph)| NamedChar {
        code,
        text: char::from(code).to_string(),
        trigraph,
    });
    high.chain(latin).collect()
}

/// HP bytes as 7-bit ASCII the way the calculator writes them with
/// translation mode 3: bytes 128-255 become trigraphs, the rest is
/// unchanged.
pub fn to_trigraphs(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x80..=0x9F => s.push_str(HIGH_TRIGRAPHS[usize::from(b - 0x80)]),
            0xA0..=0xFF => match LATIN_TRIGRAPHS.iter().find(|(l, _)| *l == b) {
                Some((_, t)) => s.push_str(t),
                None => s.push_str(&format!("\\{b}")),
            },
            _ => s.push(char::from(b)),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a 48SX sends for `"" 128 255 FOR I I CHR + NEXT` with
    /// `IOPAR` translation 3 (recorded 2026-10-05).
    const SX_T3: &str = concat!(
        "\\<)\\x-\\.V\\v/\\.S\\GS\\|>\\pi\\.d\\<=\\>=\\=/\\Ga\\->\\<-\\|v\\|^\\Gg\\Gd",
        "\\Ge\\Gn\\Gh\\Gl\\Gr\\Gs\\Gt\\Gw\\GD\\PI\\GW\\[]\\oo\\160\\161\\162\\163",
        "\\164\\165\\166\\167\\168\\169\\170\\<<\\172\\173\\174\\175\\^o\\177\\178",
        "\\179\\180\\Gm\\182\\183\\184\\185\\186\\>>\\188\\189\\190\\191\\192\\193",
        "\\194\\195\\196\\197\\198\\199\\200\\201\\202\\203\\204\\205\\206\\207",
        "\\208\\209\\210\\211\\212\\213\\214\\.x\\O/\\217\\218\\219\\220\\221\\222",
        "\\Gb\\224\\225\\226\\227\\228\\229\\230\\231\\232\\233\\234\\235\\236",
        "\\237\\238\\239\\240\\241\\242\\243\\244\\245\\246\\:-\\248\\249\\250",
        "\\251\\252\\253\\254\\255",
    );

    #[test]
    fn trigraphs_match_the_calculator() {
        let high: Vec<u8> = (128..=255).collect();
        assert_eq!(to_trigraphs(&high), SX_T3);
        assert_eq!(encode_command(SX_T3).unwrap(), high);
    }

    #[test]
    fn named_chars_cover_both_tables_and_round_trip() {
        let named = named_chars();
        assert_eq!(named.len(), HIGH.len() + LATIN_TRIGRAPHS.len());
        assert!(named.windows(2).all(|w| w[0].code < w[1].code));
        for c in &named {
            assert_eq!(
                encode_command(c.trigraph).unwrap(),
                [c.code],
                "{}",
                c.trigraph
            );
            assert_eq!(encode(&c.text).unwrap(), [c.code], "{}", c.text);
            assert_eq!(decode(&[c.code]), c.text);
            assert_eq!(to_trigraphs(&[c.code]), c.trigraph);
        }
        assert_eq!(named[3].trigraph, "\\v/");
        assert_eq!(named[3].text, "√");
    }

    #[test]
    fn command_trigraphs() {
        assert_eq!(
            encode_command("{ 1 2 } \\->STR").unwrap(),
            b"{ 1 2 } \x8dSTR"
        );
        assert_eq!(encode_command("→STR").unwrap(), b"\x8dSTR");
        // Not a trigraph: kept.
        assert_eq!(encode_command("\"a\\b\" \\1").unwrap(), b"\"a\\b\" \\1");
        assert_eq!(encode_command("\\127 \\256").unwrap(), b"\\127 \\256");
        assert_eq!(encode_command("\\").unwrap(), b"\\");
    }

    #[test]
    fn trigraphs_become_their_bytes() {
        assert_eq!(
            encode_command("\\<< 1 \\->LIST \\>> \\GS \\160 \\q").unwrap(),
            [
                0xAB, b' ', b'1', b' ', 0x8D, b'L', b'I', b'S', b'T', b' ', 0xBB, b' ', 0x85, b' ',
                160, b' ', b'\\', b'q'
            ]
        );
        assert_eq!(encode_command("\\127").unwrap(), b"\\127");
        assert_eq!(encode_command("€"), Err('€'));
        for (i, t) in HIGH_TRIGRAPHS.iter().enumerate() {
            assert_eq!(encode_command(t).unwrap(), [0x80 + i as u8]);
        }
    }

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
