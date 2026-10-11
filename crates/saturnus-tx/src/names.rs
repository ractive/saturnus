//! Variable names to file names and back, and what a file holds
//! (`kb/docs/saturnus-tx.md`, "File names").
//!
//! A variable becomes `NAME.hp` (a binary transfer file) or `NAME.txt`
//! (the calculator's text form). Names arrive from `satx-core` in Unicode
//! (`→`, `α`, `Σ`: its `charset`), which every file system takes; the few
//! characters a file system refuses (`/ \ : * ? " < > |`), control
//! characters, `%` itself and a trailing dot or space (Windows drops
//! those) are written `%XX`, one per UTF-8 byte, and read back the same
//! way. So is the first letter of a name Windows reserves for a device
//! (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`, in any case,
//! before the first dot): `AUX` becomes `%41UX.hp`. A file name goes back
//! to a variable name without its extension.
//!
//! Names are kept as they are, case included. The calculator tells `A`
//! from `a`; a case-insensitive file system (Windows, macOS by default)
//! does not, so `A.hp` and `a.hp` collide in one folder: whoever writes
//! files into a folder has to check for that.

use satx_core::object::{AsciiHeader, BinaryHeader};

use crate::device::{Format, TxError};

/// What a file holds, as a put sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A binary transfer file (`HPHP48-x` or `HPHP49-x` header): sent as
    /// is, in binary mode.
    Binary,
    /// The calculator's text form (`%%HP:` header): sent in text mode, the
    /// calculator compiles it.
    Text,
    /// Anything else: the calculator stores it as a string.
    Other,
}

impl FileKind {
    /// What `data` holds, by its header.
    pub fn of(data: &[u8]) -> FileKind {
        if BinaryHeader::parse(data).is_some() {
            FileKind::Binary
        } else if AsciiHeader::parse(data).is_some() {
            FileKind::Text
        } else {
            FileKind::Other
        }
    }
}

/// The file name of variable `name` got in `format`: `NAME.hp` or
/// `NAME.txt`.
pub fn file_name(name: &str, format: Format) -> String {
    let extension = match format {
        Format::Binary => "hp",
        Format::Text => "txt",
    };
    format!("{}.{extension}", escape(name))
}

/// The variable name a put takes from `file_name`: without its extension
/// (the last `.` and what follows, unless the name starts there), `%XX`
/// read back; [`TxError::BadName`] when that is not UTF-8.
pub fn var_name(file_name: &str) -> Result<String, TxError> {
    let stem = match file_name.rfind('.') {
        Some(dot) if dot > 0 => &file_name[..dot],
        _ => file_name,
    };
    unescape(stem).ok_or_else(|| TxError::BadName(file_name.to_string()))
}

/// Whether Windows reserves `name` (its part before the first dot) for a
/// device.
fn reserved_on_windows(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    match base.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            let digit = |prefix: &str| {
                base.strip_prefix(prefix)
                    .is_some_and(|d| d.len() == 1 && d.bytes().all(|b| (b'1'..=b'9').contains(&b)))
            };
            digit("COM") || digit("LPT")
        }
    }
}

/// `name` with what a file system refuses written `%XX`.
pub fn escape(name: &str) -> String {
    let refused = |c: char| {
        c.is_control()
            || matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '%'
            )
    };
    let last = name.chars().count().saturating_sub(1);
    let reserved = reserved_on_windows(name);
    let mut out = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        if refused(c) || (i == last && matches!(c, '.' | ' ')) || (i == 0 && reserved) {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `%XX` (two hex digits) read back as the byte; any other `%` stays.
/// `None` when the bytes are not UTF-8.
pub fn unescape(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .filter(|h| h.bytes().all(|d| d.is_ascii_hexdigit()))
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b, hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn unicode_names_stay_as_they_are() {
        assert_eq!(file_name("A→B", Format::Binary), "A→B.hp");
        assert_eq!(file_name("Σx²", Format::Text), "Σx².txt");
        assert_eq!(var_name("A→B.hp").unwrap(), "A→B");
        assert_eq!(var_name("αβ.txt").unwrap(), "αβ");
    }

    #[test]
    fn refused_characters_round_trip_as_percent_codes() {
        for name in ["A?B", "X|Y", "50%", "DOT.", "P*Q", "a\"b", "c:d", "x\u{1}"] {
            let file = file_name(name, Format::Binary);
            assert!(
                !file[..file.len() - 3].contains(['?', '|', '*', '"', ':', '\u{1}']),
                "{file}"
            );
            assert_eq!(var_name(&file).unwrap(), name, "{file}");
        }
        assert_eq!(escape("50%"), "50%25");
        assert_eq!(escape("DOT."), "DOT%2E");
        assert_eq!(escape("A.B"), "A.B");
    }

    #[test]
    fn a_put_takes_the_name_without_the_extension() {
        assert_eq!(var_name("NOTES.md").unwrap(), "NOTES");
        assert_eq!(var_name("A.B.hp").unwrap(), "A.B");
        assert_eq!(var_name("PLAIN").unwrap(), "PLAIN");
        assert_eq!(var_name(".hidden").unwrap(), ".hidden");
        // A stray `%` stays; a lowercase hex code reads back too.
        assert_eq!(var_name("5%X.txt").unwrap(), "5%X");
        assert_eq!(var_name("a%3fb.hp").unwrap(), "a?b");
    }

    #[test]
    fn windows_device_names_are_escaped() {
        for (name, file) in [
            ("AUX", "%41UX.hp"),
            ("con", "%63on.hp"),
            ("Nul", "%4Eul.hp"),
            ("COM1", "%43OM1.hp"),
            ("lpt9", "%6Cpt9.hp"),
            ("CON.X", "%43ON.X.hp"),
        ] {
            assert_eq!(file_name(name, Format::Binary), file);
            assert_eq!(var_name(file).unwrap(), name);
        }
        for name in ["COM", "COM0", "COM10", "LPT", "CONS", "AUXB", "XCON"] {
            assert_eq!(escape(name), name);
        }
    }

    #[test]
    fn percent_codes_that_are_not_utf8_are_refused() {
        assert!(matches!(var_name("%FF.hp"), Err(TxError::BadName(n)) if n == "%FF.hp"));
        assert!(matches!(var_name("A%E2%86.hp"), Err(TxError::BadName(_))));
        assert_eq!(unescape("%E2%86%92"), Some("→".to_string()));
    }

    #[test]
    fn file_kinds_by_header() {
        assert_eq!(FileKind::of(b"HPHP48-R\x2c\x2a"), FileKind::Binary);
        assert_eq!(FileKind::of(b"HPHP49-C\x2c\x2a"), FileKind::Binary);
        assert_eq!(
            FileKind::of(b"%%HP: T(3)A(R)F(.);\r\n\"hi\""),
            FileKind::Text
        );
        assert_eq!(FileKind::of(b"hello"), FileKind::Other);
        assert_eq!(FileKind::of(b""), FileKind::Other);
    }
}
