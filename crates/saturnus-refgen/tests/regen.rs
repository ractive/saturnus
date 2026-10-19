//! The data in `crates/saturnus-cli/data/commands/` regenerated from the ROMs and compared,
//! gated by `SATURNUS_ROM_DIR` (see `kb/docs/test-policy.md`).
//!
//! - `names_match_the_catalog`: the name tables of the libraries in the
//!   48SX catalog equal the catalog (the other models and the scan for
//!   other libraries are in the ignored test).
//! - `sample_examples_match`: a sample of the 48SX examples runs again to
//!   the same JSON (seconds).
//! - `menus_match_the_catalogs`: every model's menu placement read again
//!   from the ROM's definitions with the root menus the catalog records
//!   equals the catalog's (no keys pressed; the keyboard is observed in
//!   the ignored test).
//! - `the_28s_catalog_and_sample_examples_match`: the 28S's CATALOG read
//!   off the display gives the catalog's names in its order, and a sample
//!   of its examples, typed by keys, gives the same JSON (seconds).
//! - `catalogs_and_examples_regenerate_byte_for_byte` (ignored, minutes in
//!   a release build): every catalog (names, then menus with the keyboard
//!   observed; the 28S's names, its menus kept from the file) and examples
//!   file regenerated in full equals the file. Run with
//!   `cargo test --release -p saturnus-refgen -- --ignored`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use saturnus_core::Model;
use saturnus_refgen::catalog::{self, Catalog, model_name};
use saturnus_refgen::examples::{self, Examples};
use saturnus_refgen::keyed::{self, KeyFile};
use saturnus_refgen::menus;
use saturnus_refgen::names;
use saturnus_refgen::reference::Reference;

const MODELS: [(Model, &str); 3] = [
    (Model::Hp48sx, "sxrom-j"),
    (Model::Hp48gx, "gxrom-r"),
    (Model::Hp49g, "rom.49g"),
];

fn rom(name: &str) -> Option<PathBuf> {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping refgen ROM test ({name})");
        return None;
    };
    let path = Path::new(&dir).join(name);
    assert!(path.exists(), "{} missing", path.display());
    Some(path)
}

fn data(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saturnus-cli/data/commands")
        .join(file)
}

fn read<T: serde::de::DeserializeOwned>(file: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(data(file)).unwrap()).unwrap()
}

/// `reference.json` with a comment on every example: comments are never
/// run and never generated, so the examples must come out the same.
fn reference_with_comments() -> Reference {
    let mut r: Reference = read("reference.json");
    for e in r.commands.values_mut() {
        for x in &mut e.examples {
            x.comment = Some("a comment".into());
        }
    }
    r
}

#[test]
fn menus_match_the_catalogs() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else { return };
        let m = model_name(model);
        let mut cat: Catalog = read(&format!("{m}.json"));
        let before = cat.clone();
        let p = menus::from_catalog(model, &rom, &cat).unwrap();
        menus::apply(&mut cat, &p);
        assert_eq!(cat, before, "{m}");
        let placed = cat.commands.iter().filter(|c| !c.menus.is_empty()).count();
        eprintln!("{m}: {placed} of {} commands in a menu", cat.commands.len());
    }
}

#[test]
fn names_match_the_catalog() {
    let (model, file) = MODELS[0];
    let Some(rom) = rom(file) else { return };
    let mut emu = catalog::boot(model, &rom).unwrap();
    emu.start_server().unwrap();
    let cat: Catalog = read("48sx.json");
    let mut libs: Vec<u32> = cat
        .commands
        .iter()
        .flat_map(|c| c.xlib.iter().map(|x| x[0]))
        .collect();
    libs.sort_unstable();
    libs.dedup();
    let entries = names::read_libraries(&mut emu, &libs).unwrap();
    let from_rom: Vec<[u32; 2]> = entries.iter().map(|e| [e.lib, e.number]).collect();
    let mut from_file: Vec<[u32; 2]> = cat
        .commands
        .iter()
        .flat_map(|c| c.xlib.iter().copied())
        .collect();
    from_file.sort_unstable();
    assert_eq!(from_rom, from_file);
    for e in &entries {
        assert!(
            cat.commands
                .iter()
                .any(|c| c.name == e.name && c.xlib.contains(&[e.lib, e.number])),
            "{e:?}"
        );
    }
}

#[test]
fn sample_examples_match() {
    let Some(rom) = rom("sxrom-j") else { return };
    let cat: Catalog = read("48sx.json");
    let reference = reference_with_comments();
    let file: Examples = read("examples-48sx.json");
    let only: Vec<String> = ["+", "ABS", "STO", "→LIST", "SIN", "SWAP", "IF", "DOERR"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ex = examples::generate(Model::Hp48sx, &rom, &cat, &reference, Some(&only)).unwrap();
    assert_eq!(ex.examples.len() + ex.skipped.len(), only.len());
    for (name, list) in &ex.examples {
        assert_eq!(Some(list), file.examples.get(name), "{name}");
    }
}

#[test]
fn the_28s_catalog_and_sample_examples_match() {
    let Some(rom) = rom("hp28s.rom") else { return };
    let file: Catalog = read("28s.json");
    let cat = catalog::generate_28s(&rom, Some(&file)).unwrap();
    assert_eq!(cat, file);
    let reference = reference_with_comments();
    let examples: Examples = read("examples-28s.json");
    let only: Vec<String> = ["+", "SIN", "→LIST", "STO+", "RND", "DUP", "IF", "CHR"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ex = examples::generate(Model::Hp28s, &rom, &file, &reference, Some(&only)).unwrap();
    assert_eq!(ex.examples.len() + ex.skipped.len(), only.len());
    for (name, list) in &ex.examples {
        assert_eq!(Some(list), examples.examples.get(name), "{name}");
    }
}

#[test]
#[ignore = "minutes in a release build; run with --release --ignored"]
fn catalogs_and_examples_regenerate_byte_for_byte() {
    let reference: Reference = read("reference.json");
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else { return };
        let m = model_name(model);
        let mut cat = catalog::generate(model, &rom).unwrap();
        menus::apply(&mut cat, &menus::generate(model, &rom).unwrap());
        let text = catalog::to_file_text(&cat).unwrap();
        assert_eq!(
            text,
            std::fs::read_to_string(data(&format!("{m}.json"))).unwrap(),
            "{m}.json"
        );
        let ex = examples::generate(model, &rom, &cat, &reference, None).unwrap();
        assert_eq!(
            examples::to_file_text(&ex).unwrap(),
            std::fs::read_to_string(data(&format!("examples-{m}.json"))).unwrap(),
            "examples-{m}.json"
        );
    }
    let Some(rom) = rom("hp28s.rom") else { return };
    let file: Catalog = read("28s.json");
    let cat = catalog::generate_28s(&rom, Some(&file)).unwrap();
    assert_eq!(
        catalog::to_file_text(&cat).unwrap(),
        std::fs::read_to_string(data("28s.json")).unwrap(),
        "28s.json"
    );
    let ex = examples::generate(Model::Hp28s, &rom, &cat, &reference, None).unwrap();
    assert_eq!(
        examples::to_file_text(&ex).unwrap(),
        std::fs::read_to_string(data("examples-28s.json")).unwrap(),
        "examples-28s.json"
    );
}

/// Every key-driven model's reference (`keys-<model>.json`) checked on the
/// emulator and its examples run again to the committed file, byte for
/// byte (a second or two per Pioneer; the 38G, 39G and 40G take about
/// 40 s each in a debug build). A model without its ROM in
/// `SATURNUS_ROM_DIR` is skipped.
#[test]
fn keyed_references_regenerate_byte_for_byte() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping the key-driven references");
        return;
    };
    for entry in std::fs::read_dir(data("")).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(m) = file
            .strip_prefix("keys-")
            .and_then(|f| f.strip_suffix(".json"))
        else {
            continue;
        };
        let model: Model = m.parse().unwrap();
        // The file saturnus downloads (rom.39g for the 39G and 40G), else
        // the owner's dump, hp<model>.rom.
        let name = saturnus_host::romid::download(model)
            .map_or_else(|| format!("hp{m}.rom"), |k| k.file.to_string());
        let rom = Path::new(&dir).join(name);
        if !rom.exists() {
            eprintln!("{} missing: skipping {file}", rom.display());
            continue;
        }
        let mut keys: KeyFile = read(&file);
        // Comments are never run nor generated.
        for c in &mut keys.commands {
            for x in &mut c.examples {
                x.comment = Some("a comment".into());
            }
        }
        let ex = keyed::generate(model, &rom, &keys, None).unwrap();
        assert_eq!(
            keyed::to_file_text(&ex).unwrap(),
            std::fs::read_to_string(data(&format!("examples-{m}.json"))).unwrap(),
            "examples-{m}.json"
        );
    }
}

/// The 40G runs the 39G's ROM with the same keyboard: its reference is the
/// 39G's, function for function.
#[test]
fn the_40g_reference_is_the_39gs() {
    let a: KeyFile = read("keys-39g.json");
    let b: KeyFile = read("keys-40g.json");
    assert_eq!((a.model.as_str(), b.model.as_str()), ("39g", "40g"));
    assert_eq!(a.commands, b.commands);
    assert_eq!((a.learn, a.home, a.ready), (b.learn, b.home, b.ready));
}
