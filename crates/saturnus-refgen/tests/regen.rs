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
//! - `catalogs_and_examples_regenerate_byte_for_byte` (ignored, minutes in
//!   a release build): every catalog (names, then menus with the keyboard
//!   observed) and examples file regenerated in full equals the file. Run with
//!   `cargo test --release -p saturnus-refgen -- --ignored`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use saturnus::Model;
use saturnus_refgen::catalog::{self, Catalog, model_name};
use saturnus_refgen::examples::{self, Examples};
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
    let reference: Reference = read("reference.json");
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
}
