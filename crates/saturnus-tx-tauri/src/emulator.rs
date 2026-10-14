//! The emulator desktop app's ROMs (kb/iterations/iteration-49-tx-desktop-roots.md):
//! read-only from its `settings.json` (crates/saturnus-tauri, identifier
//! `ch.ractive.saturnus`; `{"version": 1, "roms": {"48gx": {"path",
//! "sha256"}}, ...}`), so desktop tx offers the ROMs the user gave the
//! emulator app. The page gets each ROM's model, file name and SHA-256,
//! and the bytes by model; never a path (web/protocol.md, "ROM slots").
//! The page checks the bytes against the SHA-256 (a changed file is left
//! out). Not checked against the roots: the paths are the emulator
//! app's, not the page's.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// The emulator app's settings file in its config folder.
pub const SETTINGS_FILE: &str = "settings.json";
/// The emulator app's identifier: its config folder's name.
pub const EMULATOR_ID: &str = "ch.ractive.saturnus";
/// The most of the settings read (the emulator's own cap).
const MAX_SETTINGS: u64 = 64 * 1024;
/// The most of a ROM read (the emulator's own cap).
pub const MAX_ROM: u64 = 4 << 20;

/// One of the emulator app's ROMs, as the page sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rom {
    /// The model (`48gx`).
    pub model: String,
    /// The file's name, without its folder.
    pub file_name: String,
    /// The SHA-256 the emulator app recorded, lower-case hex.
    pub sha256: String,
}

/// `path`'s bytes, at most `cap` (more is an error).
fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let f = fs::File::open(path).map_err(|e| format!("the file cannot be read: {e}"))?;
    let mut out = Vec::new();
    f.take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("the file cannot be read: {e}"))?;
    if out.len() as u64 > cap {
        return Err(format!("the file is more than {} KiB", cap >> 10));
    }
    Ok(out)
}

/// A model's name as the settings hold it: a few lower-case letters and
/// digits.
fn is_model(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 8
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The ROMs of settings file `settings` with their paths: those whose
/// file is there, in the models' order. None when the file is missing,
/// over the cap or malformed.
fn entries(settings: &Path) -> Vec<(Rom, PathBuf)> {
    let Ok(bytes) = read_capped(settings, MAX_SETTINGS) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    if v.get("version").and_then(Value::as_u64) != Some(1) {
        return Vec::new();
    }
    let Some(roms) = v.get("roms").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out: Vec<(Rom, PathBuf)> = roms
        .iter()
        .filter_map(|(model, r)| {
            let path = PathBuf::from(r.get("path")?.as_str()?);
            let sha256 = r.get("sha256")?.as_str()?;
            let file_name = path.file_name()?.to_str()?.to_string();
            let hex = sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit());
            let file = fs::metadata(&path).is_ok_and(|m| m.is_file());
            (is_model(model) && hex && path.is_absolute() && file).then(|| {
                (
                    Rom {
                        model: model.clone(),
                        file_name,
                        sha256: sha256.to_ascii_lowercase(),
                    },
                    path,
                )
            })
        })
        .collect();
    out.sort_by(|a, b| a.0.model.cmp(&b.0.model));
    out
}

/// The emulator app's ROMs whose files are there.
pub fn roms(settings: &Path) -> Vec<Rom> {
    entries(settings).into_iter().map(|(r, _)| r).collect()
}

/// The bytes of the emulator app's ROM of `model` (at most [`MAX_ROM`]).
pub fn rom(settings: &Path, model: &str) -> Result<Vec<u8>, String> {
    let Some((_, path)) = entries(settings)
        .into_iter()
        .find(|(r, _)| r.model == model)
    else {
        return Err(format!("the emulator app has no ROM for {model}"));
    };
    read_capped(&path, MAX_ROM)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("saturnus-tx-emu-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SHA: &str = "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12";

    #[test]
    fn the_emulators_roms_are_listed_and_read() {
        let dir = temp_dir("list");
        let gx = dir.join("gx.rom");
        fs::write(&gx, b"ROM bytes").unwrap();
        let big = dir.join("big.rom");
        fs::write(&big, vec![0u8; MAX_ROM as usize + 1]).unwrap();
        let settings = dir.join(SETTINGS_FILE);
        fs::write(
            &settings,
            serde_json::to_vec(&json!({"version": 1, "bootLast": true, "roms": {
                "48gx": {"path": gx, "sha256": SHA.to_uppercase()},
                "49g": {"path": big, "sha256": SHA},
                "48sx": {"path": dir.join("gone.rom"), "sha256": SHA},
                "38g": {"path": "relative.rom", "sha256": SHA},
                "39g": {"path": gx, "sha256": "short"},
                "../x": {"path": gx, "sha256": SHA},
            }}))
            .unwrap(),
        )
        .unwrap();
        let list = roms(&settings);
        assert_eq!(
            list,
            [
                Rom {
                    model: "48gx".into(),
                    file_name: "gx.rom".into(),
                    sha256: SHA.into()
                },
                Rom {
                    model: "49g".into(),
                    file_name: "big.rom".into(),
                    sha256: SHA.into()
                },
            ],
            "a missing file is left out, as are bad entries"
        );
        assert_eq!(rom(&settings, "48gx").unwrap(), b"ROM bytes");
        assert!(rom(&settings, "49g").is_err(), "over the cap");
        assert!(rom(&settings, "48sx").is_err(), "missing");
        assert!(rom(&settings, "42s").is_err(), "none");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_settings_read_as_none() {
        let dir = temp_dir("bad");
        let settings = dir.join(SETTINGS_FILE);
        assert!(roms(&settings).is_empty(), "no file");
        for bad in [
            b"{".to_vec(),
            br#"{"version": 2, "roms": {}}"#.to_vec(),
            br#"{"version": 1, "roms": []}"#.to_vec(),
            vec![b' '; MAX_SETTINGS as usize + 1],
        ] {
            fs::write(&settings, bad).unwrap();
            assert!(roms(&settings).is_empty());
            assert!(rom(&settings, "48gx").is_err());
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
