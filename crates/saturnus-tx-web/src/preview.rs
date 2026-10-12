//! The preview's decoding, without wasm: a binary transfer file as the
//! typed object `web/shared/objects.js` shows, and a text file's body.
//! Without a ROM, data objects and GROBs decode in full but a program's or
//! an algebraic's text needs the ROM's command names; with the ROM of the
//! file's model ([`RomNames`], the 48SX and 48GX) they decode in full too
//! (`kb/docs/saturnus-tx.md`, "Preview").

use anyhow::{Context, bail};
use saturnus_core::Model;
use saturnus_objects::charset;
use saturnus_objects::decompile::{Settings, described};
use saturnus_objects::names::NameTable;
use saturnus_objects::object::{Memory, NoMemory, Reader};
use saturnus_objects::transfer::{HEADER_LEN, ascii_body, decode_file, is_binary_file, unpack};
use serde_json::Value;

/// The model a binary transfer file comes from, by its header: a
/// `HPHP49-` file is the 49G's; in `HPHP48-x`, x is the ROM revision
/// (`#30794h SYSEVAL` returns it), A to J on the 48S and SX, K to R on the
/// 48G and GX (wiki: protocols/hp-object-format, from the HP 48 FAQ 3.4,
/// 3.5, 3.7 and 3.10). `None` for another letter or no binary file.
pub fn file_model(data: &[u8]) -> Option<Model> {
    if !is_binary_file(data) {
        return None;
    }
    let letter = *data.get(HEADER_LEN - 1)?;
    if data.starts_with(b"HPHP49-") {
        return Some(Model::Hp49g);
    }
    match letter {
        b'A'..=b'J' => Some(Model::Hp48sx),
        b'K'..=b'R' => Some(Model::Hp48gx),
        _ => None,
    }
}

/// The display settings of `data`'s model (a 48 file of an unknown
/// revision in the 48GX's).
fn settings_of(data: &[u8]) -> Settings {
    Settings::standard(file_model(data).unwrap_or(Model::Hp48gx))
}

/// The typed object of binary transfer file `data`, described (each
/// object's `text` where it has one), in the display settings of the
/// file's model. Without command names: a program or an algebraic that
/// holds a command has no text ([`RomNames::describe`] has).
pub fn describe_bytes(data: &[u8]) -> anyhow::Result<Value> {
    let obj = decode_file(data, &NoMemory)?;
    described(&obj, &settings_of(data))
}

/// A mask ROM image (48SX, 48GX: the image index is the address).
struct Image<'a>(&'a [u8]);

impl Memory for Image<'_> {
    fn nibble(&self, addr: u32) -> Option<u8> {
        self.0.get(addr as usize).copied()
    }
}

/// The command names of one ROM, for decoding transfer files from a
/// calculator of the same model.
pub struct RomNames {
    model: Model,
    nibbles: Vec<u8>,
    table: NameTable,
}

impl std::fmt::Debug for RomNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RomNames")
            .field("model", &self.model)
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

impl RomNames {
    /// The names of `model`'s ROM image `rom` (the packed file the
    /// emulator keeps). Only the 48SX and 48GX: their ROM is mask ROM, so
    /// a ROM pointer in a file is an index into the image; the 49G's
    /// pointers depend on which flash banks the calculator shows, which
    /// a file does not say.
    pub fn new(model: Model, rom: &[u8]) -> anyhow::Result<RomNames> {
        if !matches!(model, Model::Hp48sx | Model::Hp48gx) {
            bail!("command names from a ROM image only for the 48SX and 48GX");
        }
        if rom.len() != model.rom_bytes() {
            bail!(
                "a {} ROM image has {} bytes, not {}",
                model.name(),
                model.rom_bytes(),
                rom.len()
            );
        }
        let nibbles = unpack(rom);
        let table = NameTable::build(model, &nibbles);
        Ok(RomNames {
            model,
            nibbles,
            table,
        })
    }

    /// The model whose ROM this is.
    pub fn model(&self) -> Model {
        self.model
    }

    /// [`describe_bytes`] with this ROM's command names: programs and
    /// algebraics get their text. For a file of this ROM's model.
    pub fn describe(&self, data: &[u8]) -> anyhow::Result<Value> {
        if !is_binary_file(data) {
            bail!("not an HP binary object (no HPHP48-x / HPHP49-x header)");
        }
        let nibbles = unpack(
            data.get(HEADER_LEN..)
                .context("no object after the header")?,
        );
        let settings = settings_of(data);
        let mem = Image(&self.nibbles);
        let obj = Reader::with_names(&mem, &self.table, settings).decode(&nibbles)?;
        described(&obj, &settings)
    }
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
    fn the_model_comes_from_the_rom_letter() {
        let file = |h: &[u8]| [h, &[0u8; 4][..]].concat();
        assert_eq!(file_model(&file(b"HPHP48-A")), Some(Model::Hp48sx));
        assert_eq!(file_model(&file(b"HPHP48-J")), Some(Model::Hp48sx));
        assert_eq!(file_model(&file(b"HPHP48-K")), Some(Model::Hp48gx));
        assert_eq!(file_model(&file(b"HPHP48-R")), Some(Model::Hp48gx));
        assert_eq!(file_model(&file(b"HPHP49-C")), Some(Model::Hp49g));
        assert_eq!(file_model(&file(b"HPHP48-X")), None);
        assert_eq!(file_model(b"plain text"), None);
        assert!(RomNames::new(Model::Hp49g, &[]).is_err());
        assert!(RomNames::new(Model::Hp48sx, &[0; 16]).is_err());
    }

    /// Nibbles of the 5-nibble field `v`, low first.
    fn f5(v: u32) -> Vec<u8> {
        (0..5).map(|i| ((v >> (4 * i)) & 0xF) as u8).collect()
    }

    /// `'SIN(X)'` as the 48SX keeps it (a 48SX's EQ, 18.5 bytes with its
    /// name): the name X, a pointer to SIN in `names`' ROM, SEMI.
    fn sin_x(names: &RomNames, header: &[u8]) -> Vec<u8> {
        use saturnus_objects::ObjectType;
        use saturnus_objects::prolog::SEMI;
        let (lib, cmd) = names.table.find("SIN").unwrap();
        let sin = names.table.object_index(lib, cmd).unwrap() as u32;
        let mut n = f5(ObjectType::Algebraic.prolog());
        n.extend(f5(ObjectType::GlobalName.prolog()));
        n.extend([1, 0, 8, 5]); // one character, "X" (#58)
        n.extend(f5(sin));
        n.extend(f5(SEMI));
        [header, &saturnus_objects::transfer::pack(&n)].concat()
    }

    /// ROM-gated (`SATURNUS_ROM_DIR` with `sxrom-j`): a 48SX file's
    /// algebraic has its text with the 48SX ROM's names, none without.
    #[test]
    fn a_48sx_algebraic_has_its_text_with_the_48sx_rom() {
        let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
            eprintln!("SATURNUS_ROM_DIR not set: skipped");
            return;
        };
        let rom = std::fs::read(std::path::Path::new(&dir).join("sxrom-j")).unwrap();
        let names = RomNames::new(Model::Hp48sx, &rom).unwrap();
        let file = sin_x(&names, b"HPHP48-J");
        assert_eq!(file_model(&file), Some(Model::Hp48sx));
        let v = names.describe(&file).unwrap();
        assert_eq!(v["type"], "algebraic");
        assert_eq!(v["text"], "'SIN(X)'");
        // Without the ROM: no text, as before.
        assert!(describe_bytes(&file).unwrap().get("text").is_none());
    }

    #[test]
    fn a_text_file_reads_with_its_trigraphs_as_characters() {
        let file = b"%%HP: T(3)A(R)F(.);\r\n\\<< \\-> A 'A^2' \\>>";
        assert_eq!(text_body(file), "« → A 'A^2' »");
        assert_eq!(text_body(b"plain"), "plain");
    }
}
