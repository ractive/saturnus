//! The HP 48/49 character set and the ASCII trigraphs of the transfer
//! translation modes: `saturnus-objects`' table, with this crate's
//! [`Error`] on the encoding side.
//!
//! Bytes 0-127 are ASCII, 160-255 are ISO 8859-1, and 128-159 are HP's own
//! math and Greek characters. Names, file data and server text cross the
//! `kermit-proto` boundary as raw bytes in this character set; this module
//! translates them to and from UTF-8. In ASCII transfers with translation
//! mode 2 or 3 (`TRANSIO`) the calculator writes characters 128-255 as
//! backslash trigraphs (`\->` for `→`, `\160` for a no-break space); it
//! does not read them in a host command (`C` packet) itself, so
//! [`encode_command`] translates them before sending.

pub use saturnus_objects::charset::{NamedChar, decode, named_chars, to_trigraphs, trigraph_at};

use crate::{Error, Result};

/// Translate a string to HP bytes. `x̄` is `x` followed by U+0304
/// (combining macron); everything else is one character per byte.
pub fn encode(text: &str) -> Result<Vec<u8>> {
    saturnus_objects::charset::encode(text).map_err(Error::Charset)
}

/// Like [`encode`], but also translates the ASCII trigraphs (`\->`, `\<<`,
/// `\GS`, `\160`, ...) to their bytes. A backslash that starts no known
/// trigraph is kept as is. Use it for host command text typed in ASCII.
pub fn encode_command(text: &str) -> Result<Vec<u8>> {
    saturnus_objects::charset::encode_command(text).map_err(Error::Charset)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_character() {
        assert!(matches!(encode("€"), Err(Error::Charset('€'))));
        assert!(matches!(encode_command("\\-> €"), Err(Error::Charset('€'))));
        assert_eq!(encode("x\u{0304}y").unwrap(), b"\x81y");
        assert_eq!(encode_command("\\->STR").unwrap(), b"\x8dSTR");
    }
}
