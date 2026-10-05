//! The ROM's command names, read through the calculator's own decompiler.
//!
//! A built-in command is reachable as an XLIB name (library number and
//! command number, three nibbles each); decompiling an XLIB name of an
//! attached library prints the command's name from the library's name
//! table, and `XLIB lib n` when the entry has none (RPL manual in the
//! 1993 SDK, "XLIB names"; wiki: protocols/hp-object-format). The
//! extraction sends lists of XLIB names as binary objects, fetches them
//! back as ASCII (the decompiled text) and keeps the named entries.

use anyhow::{Context, Result, bail};
use hptx_core::TransferMode;
use hptx_core::object::{BinaryHeader, Family, ObjectType, pack};
use saturnus::Model;
use saturnus_mcp::emulator::Emulator;

/// Highest library number (three nibbles).
const MAX_LIBRARY: u32 = 0x7FF;
/// Commands probed per library when looking for named libraries.
const PROBE_COMMANDS: u32 = 16;
/// Libraries probed per transfer.
const PROBE_LIBRARIES: u32 = 64;
/// Commands decompiled per transfer when reading a library (small enough
/// for the 48SX's 32 KB of RAM).
const CHUNK: u32 = 512;
/// Highest command number (three nibbles).
const MAX_COMMAND: u32 = 0xFFF;
/// Temporary variable for the probes.
const TEMP: &str = "SATRNREF";

/// A named entry of a built-in library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Library number.
    pub lib: u32,
    /// Command number within the library.
    pub number: u32,
    /// The name as the decompiler prints it (HP characters as Unicode).
    pub name: String,
}

/// The binary transfer family of `model`.
pub fn family(model: Model) -> Family {
    if model == Model::Hp49g {
        Family::Hp49
    } else {
        Family::Hp48
    }
}

fn put_field(out: &mut Vec<u8>, value: u32, width: usize) {
    out.extend((0..width).map(|i| ((value >> (4 * i)) & 0xF) as u8));
}

/// A binary transfer file holding a list of XLIB names.
fn xlib_list(family: Family, names: &[(u32, u32)]) -> Vec<u8> {
    let mut n = Vec::new();
    put_field(&mut n, ObjectType::List.prolog(), 5);
    for &(lib, cmd) in names {
        put_field(&mut n, ObjectType::XlibName.prolog(), 5);
        put_field(&mut n, lib, 3);
        put_field(&mut n, cmd, 3);
    }
    // SEMI, the end of a composite.
    put_field(&mut n, 0x0312B, 5);
    let mut data = BinaryHeader { family, rom: b'X' }.to_bytes().to_vec();
    data.extend(pack(&n));
    data
}

/// Decompile `names` on the calculator (its Kermit server running) and
/// return the name of each, `None` where the decompiler printed `XLIB`.
fn decompile(emu: &mut Emulator, names: &[(u32, u32)]) -> Result<Vec<Option<String>>> {
    let data = xlib_list(family(emu.model()), names);
    let stored = emu
        .send_object(TEMP, &data, TransferMode::Binary)
        .context("cannot send the XLIB list")?;
    if stored != TEMP {
        bail!("the calculator stored the XLIB list as {stored}");
    }
    let text = emu
        .receive_object(TEMP, TransferMode::Ascii)
        .context("cannot fetch the decompiled XLIB list")?;
    let reply = emu.run_command(&format!("'{TEMP}' PURGE"))?;
    if let Some(e) = reply.error {
        bail!("cannot purge {TEMP}: {e}");
    }
    let text = hptx_core::charset::decode(&text);
    let words = list_words(&text)?;
    let mut out = Vec::with_capacity(names.len());
    let mut i = 0;
    while i < words.len() {
        if words[i] == "XLIB" {
            out.push(None);
            i += 3;
        } else {
            out.push(Some(words[i].to_string()));
            i += 1;
        }
    }
    if out.len() != names.len() {
        bail!(
            "the decompiled list has {} entries, {} were sent",
            out.len(),
            names.len()
        );
    }
    Ok(out)
}

/// The words between the outer `{` and `}` of an ASCII transfer (after
/// its `%%HP` header line).
fn list_words(text: &str) -> Result<Vec<&str>> {
    let body = text
        .find('{')
        .and_then(|a| text.rfind('}').map(|b| &text[a + 1..b]))
        .context("the decompiled text holds no list")?;
    Ok(body.split_whitespace().collect())
}

/// The libraries that name any of their first commands.
fn named_libraries(emu: &mut Emulator) -> Result<Vec<u32>> {
    let mut libs = Vec::new();
    let all: Vec<u32> = (0..=MAX_LIBRARY).collect();
    for chunk in all.chunks(PROBE_LIBRARIES as usize) {
        let names: Vec<(u32, u32)> = chunk
            .iter()
            .flat_map(|&l| (0..PROBE_COMMANDS).map(move |c| (l, c)))
            .collect();
        let decoded = decompile(emu, &names)?;
        for (&(lib, _), name) in names.iter().zip(&decoded) {
            if name.is_some() && libs.last() != Some(&lib) {
                libs.push(lib);
            }
        }
    }
    Ok(libs)
}

/// Every named entry of the ROM's libraries, by library and number. The
/// Kermit server must be running.
pub fn extract(emu: &mut Emulator) -> Result<Vec<Entry>> {
    let libs = named_libraries(emu)?;
    read_libraries(emu, &libs)
}

/// Every named entry of the libraries `libs`, by library and number. The
/// Kermit server must be running.
pub fn read_libraries(emu: &mut Emulator, libs: &[u32]) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for &lib in libs {
        let mut start = 0;
        while start <= MAX_COMMAND {
            let end = (start + CHUNK).min(MAX_COMMAND + 1);
            let names: Vec<(u32, u32)> = (start..end).map(|c| (lib, c)).collect();
            let decoded = decompile(emu, &names)?;
            let before = out.len();
            for (&(lib, number), name) in names.iter().zip(decoded) {
                if let Some(name) = name {
                    out.push(Entry { lib, number, name });
                }
            }
            if out.len() == before {
                // A whole chunk without names: the table has ended.
                break;
            }
            start = end;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlib_lists_are_packed_nibbles() {
        let data = xlib_list(Family::Hp48, &[(2, 0x123)]);
        assert_eq!(&data[..6], b"HPHP48");
        // List prolog 02A74, XLIB prolog 02E92, 002, 123, SEMI 0312B,
        // nibbles low first, padded to whole bytes.
        let nibbles = hptx_core::object::unpack(&data[8..]);
        assert_eq!(
            nibbles,
            [
                4, 7, 0xA, 2, 0, 2, 9, 0xE, 2, 0, 2, 0, 0, 3, 2, 1, 0xB, 2, 1, 3, 0, 0
            ]
        );
    }

    #[test]
    fn list_words_skip_the_header() {
        let text = "%%HP: T(1)A(D)F(.);\n{ ASR XLIB 2 1\nRL }\n";
        assert_eq!(list_words(text).unwrap(), ["ASR", "XLIB", "2", "1", "RL"]);
        assert!(list_words("no list").is_err());
    }
}
