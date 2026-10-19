//! Compresses the command reference (`data/commands/` in this crate, so it
//! is in the published package) for `saturnus ref`: the JSON is about
//! 2.5 MB, deflated about a tenth of that. Every `keys-<model>.json` (a
//! key-driven model's reference) is packed too and listed in `keyed.rs`.

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

fn pack(data: &Path, out: &Path, file: &str) {
    let src = data.join(file);
    println!("cargo::rerun-if-changed={}", src.display());
    let bytes =
        std::fs::read(&src).unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));
    let packed = miniz_oxide::deflate::compress_to_vec(&bytes, 10);
    std::fs::write(out.join(format!("{file}.deflate")), packed)
        .unwrap_or_else(|e| panic!("cannot write {file}.deflate: {e}"));
}

fn main() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/commands");
    let out = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from);
    let Some(out) = out else {
        panic!("OUT_DIR is not set");
    };
    for file in FILES {
        pack(&data, &out, file);
    }
    // The key-driven models: every keys-<model>.json, with its examples
    // when generated, listed in keyed.rs for `include!`.
    println!("cargo::rerun-if-changed={}", data.display());
    let mut models: Vec<String> = std::fs::read_dir(&data)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", data.display()))
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            Some(
                name.strip_prefix("keys-")?
                    .strip_suffix(".json")?
                    .to_string(),
            )
        })
        .collect();
    models.sort();
    let mut list = String::from(
        "/// The key-driven models' references: model, `keys-<model>.json` and\n\
         /// `examples-<model>.json` (empty when none was generated), deflated.\n\
         const KEYED: &[(&str, &[u8], &[u8])] = &[\n",
    );
    for m in &models {
        pack(&data, &out, &format!("keys-{m}.json"));
        let ex = format!("examples-{m}.json");
        let examples = if data.join(&ex).exists() {
            pack(&data, &out, &ex);
            format!("include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{ex}.deflate\"))")
        } else {
            "&[]".to_string()
        };
        list.push_str(&format!(
            "    (\"{m}\", include_bytes!(concat!(env!(\"OUT_DIR\"), \"/keys-{m}.json.deflate\")), {examples}),\n"
        ));
    }
    list.push_str("];\n");
    std::fs::write(out.join("keyed.rs"), list)
        .unwrap_or_else(|e| panic!("cannot write keyed.rs: {e}"));
}
