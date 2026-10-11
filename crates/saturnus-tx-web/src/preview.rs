//! The preview's decoding, without wasm: a binary transfer file as the
//! typed object `web/shared/objects.js` shows, and a text file's body.
//! No ROM: data objects and GROBs decode in full; a program's or an
//! algebraic's text needs the ROM's command names, so the page gets those
//! as text from the calculator instead (`kb/docs/saturnus-tx.md`,
//! "Preview").

use saturnus_core::Model;
use saturnus_objects::charset;
use saturnus_objects::decompile::{Settings, described};
use saturnus_objects::object::NoMemory;
use saturnus_objects::transfer::{ascii_body, decode_file};
use serde_json::Value;

/// The typed object of binary transfer file `data`, described (each
/// object's `text` where it has one), in the display settings of the
/// file's family (a `HPHP49-` file in the 49G's).
pub fn describe_bytes(data: &[u8]) -> anyhow::Result<Value> {
    let obj = decode_file(data, &NoMemory)?;
    let model = if data.starts_with(b"HPHP49-") {
        Model::Hp49g
    } else {
        Model::Hp48gx
    };
    described(&obj, &Settings::standard(model))
}

/// The text of file `data` after its `%%HP:` header line, read as the
/// calculator's characters, with the ASCII trigraphs (`\<<`, `\->`,
/// `\GS`) as the characters they stand for.
pub fn text_body(data: &[u8]) -> String {
    let text = charset::decode(data);
    let body = ascii_body(&text).trim_start_matches(['\r', '\n']);
    // Trigraphs to their bytes, then the bytes as characters; a character
    // outside the calculator's set stays as it was.
    match charset::encode_command(body) {
        Ok(bytes) => charset::decode(&bytes),
        Err(_) => body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saturnus_objects::transfer::{Family, encode_file};

    #[test]
    fn a_real_is_described_with_its_text() {
        let obj: saturnus_objects::Object =
            serde_json::from_value(serde_json::json!({"type": "real", "value": 1.5})).unwrap();
        let file = encode_file(&obj, Family::Hp48).unwrap().unwrap();
        let v = describe_bytes(&file).unwrap();
        assert_eq!(v["type"], "real");
        assert_eq!(v["text"], "1.5");
        assert!(describe_bytes(b"not a file").is_err());
    }

    #[test]
    fn a_text_file_reads_with_its_trigraphs_as_characters() {
        let file = b"%%HP: T(3)A(R)F(.);\r\n\\<< \\-> A 'A^2' \\>>";
        assert_eq!(text_body(file), "« → A 'A^2' »");
        assert_eq!(text_body(b"plain"), "plain");
    }
}
