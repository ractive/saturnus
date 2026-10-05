//! Compresses the command reference (`data/commands/`) for `saturnus ref`:
//! the JSON is about 2.5 MB, deflated about a tenth of that.

use std::path::Path;

const FILES: [&str; 9] = [
    "48sx.json",
    "48gx.json",
    "49g.json",
    "examples-48sx.json",
    "examples-48gx.json",
    "examples-49g.json",
    "reference.json",
    "manuals.json",
    "categories.json",
];

fn main() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/commands");
    let out = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from);
    let Some(out) = out else {
        panic!("OUT_DIR is not set");
    };
    for file in FILES {
        let src = data.join(file);
        println!("cargo::rerun-if-changed={}", src.display());
        let bytes =
            std::fs::read(&src).unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));
        let packed = miniz_oxide::deflate::compress_to_vec(&bytes, 10);
        std::fs::write(out.join(format!("{file}.deflate")), packed)
            .unwrap_or_else(|e| panic!("cannot write {file}.deflate: {e}"));
    }
}
