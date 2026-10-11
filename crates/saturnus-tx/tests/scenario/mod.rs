//! The session `just tx-traces` records from saturnus and the trace tests
//! replay: one script for both, so a replay asks for exactly what was
//! recorded and checks the same results.

use saturnus_tx::names::file_name;
use saturnus_tx::{Format, Model, Outcome, Request, TxError};

/// The models recorded, with saturnus's model name and the ROM file in
/// `SATURNUS_ROM_DIR`.
pub const MODELS: [(&str, &str); 3] =
    [("48sx", "sxrom-j"), ("48gx", "gxrom-r"), ("49g", "rom.49g")];

/// The seed of every recorded device (the sync markers).
pub const SEED: u64 = 0x5a7_0037;

/// A program in the calculator's text form (translation mode 3: `\<<` is
/// `«`).
pub const PROGRAM: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n";
/// A file that is no calculator object.
pub const NOTE: &[u8] = b"hello, calculator";
/// A name outside ASCII.
pub const ARROW: &str = "A\u{2192}B";

fn model(name: &str) -> Model {
    match name {
        "48sx" => Model::Hp48Sx,
        "48gx" => Model::Hp48Gx,
        _ => Model::Hp49G,
    }
}

/// The driver runs one request to its end; the label names it in the
/// trace (`# request LABEL`).
pub type Driver<'a> = dyn FnMut(&str, Request) -> Result<Outcome, TxError> + 'a;

fn put(name: &str, data: &[u8], replace: bool, as_string: bool) -> Request {
    Request::Put {
        dir: vec!["HOME".into()],
        name: name.into(),
        data: data.to_vec(),
        replace,
        as_string,
    }
}

fn get(name: &str, format: Format) -> Request {
    Request::Get {
        dir: vec!["HOME".into()],
        name: name.into(),
        format,
    }
}

fn stored(outcome: Result<Outcome, TxError>, name: &str) -> Result<(), String> {
    match outcome {
        Ok(Outcome::Stored(n)) if n == name => Ok(()),
        other => Err(format!("put {name}: {other:?}")),
    }
}

fn file(outcome: Result<Outcome, TxError>, name: &str, format: Format) -> Result<Vec<u8>, String> {
    match outcome {
        Ok(Outcome::File(f)) if f.name == name && f.format == format => {
            if f.file_name != file_name(name, format) {
                return Err(format!("get {name}: file name {}", f.file_name));
            }
            Ok(f.data)
        }
        other => Err(format!("get {name}: {other:?}")),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// The recorded session on the calculator `model_name` (fresh memory).
pub fn run(model_name: &str, driver: &mut Driver<'_>) -> Result<(), String> {
    let header: &[u8] = if model_name == "49g" {
        b"HPHP49-"
    } else {
        b"HPHP48-"
    };
    match driver("connect", Request::Connect(Default::default())) {
        Ok(Outcome::Connected(c)) => {
            if c.model != model(model_name) {
                return Err(format!("connect: model {:?}", c.model));
            }
            if c.settings != saturnus_tx::LinkSettings::DEFAULT {
                return Err(format!("connect: settings {}", c.settings));
            }
        }
        other => return Err(format!("connect: {other:?}")),
    }
    match driver("list HOME", Request::List { dir: vec![] }) {
        Ok(Outcome::Listing(l)) if l.path.as_deref() == Some(&["HOME".to_string()][..]) => {}
        other => return Err(format!("list: {other:?}")),
    }
    stored(
        driver("put PRG text", put("PRG", PROGRAM, false, false)),
        "PRG",
    )?;
    stored(
        driver("put NOTE string", put("NOTE", NOTE, false, true)),
        "NOTE",
    )?;
    stored(
        driver("put A→B string", put(ARROW, NOTE, false, true)),
        ARROW,
    )?;

    let prg = file(
        driver("get PRG binary", get("PRG", Format::Binary)),
        "PRG",
        Format::Binary,
    )?;
    if !prg.starts_with(header) {
        return Err(format!("get PRG: header {:?}", &prg[..prg.len().min(8)]));
    }
    stored(
        driver("put PRG2 binary", put("PRG2", &prg, false, false)),
        "PRG2",
    )?;
    let text = file(
        driver("get PRG2 text", get("PRG2", Format::Text)),
        "PRG2",
        Format::Text,
    )?;
    if !text.starts_with(b"%%HP:") || !contains(&text, b"1 2 +") {
        return Err(format!("get PRG2 text: {}", String::from_utf8_lossy(&text)));
    }
    let note = file(
        driver("get NOTE binary", get("NOTE", Format::Binary)),
        "NOTE",
        Format::Binary,
    )?;
    if !note.starts_with(header) || !contains(&note, NOTE) {
        return Err(format!("get NOTE: {note:?}"));
    }
    file(
        driver("get A→B binary", get(ARROW, Format::Binary)),
        ARROW,
        Format::Binary,
    )?;

    match driver("put PRG clash", put("PRG", PROGRAM, false, false)) {
        Err(TxError::NameClash(n)) if n == "PRG" => {}
        other => return Err(format!("put PRG again: {other:?}")),
    }
    stored(
        driver("put PRG replace", put("PRG", &prg, true, false)),
        "PRG",
    )?;
    match driver("list HOME again", Request::List { dir: vec![] }) {
        Ok(Outcome::Listing(l)) => {
            for name in ["PRG", "PRG2", "NOTE", ARROW] {
                if !l.entries.iter().any(|e| e.name == name) {
                    return Err(format!("list again: no {name} in {:?}", l.entries));
                }
            }
            // BYTES counts the name too; the checksum is the object's.
            let checksum = |n: &str| l.entries.iter().find(|e| e.name == n).map(|e| e.checksum);
            if checksum("PRG") != checksum("PRG2") || checksum("NOTE") != checksum(ARROW) {
                return Err(format!("list again: PRG and PRG2 differ: {:?}", l.entries));
            }
        }
        other => return Err(format!("list again: {other:?}")),
    }
    match driver("disconnect", Request::Disconnect) {
        Ok(Outcome::Disconnected) => Ok(()),
        other => Err(format!("disconnect: {other:?}")),
    }
}
