//! `saturnus-refgen`: writes the command reference data. See the README,
//! "Command reference".

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use saturnus_refgen::catalog::{self, Catalog};
use saturnus_refgen::examples;
use saturnus_refgen::menus;
use saturnus_refgen::reference::Reference;

const USAGE: &str = "usage:
  saturnus-refgen catalog --model M --rom PATH --out FILE
  saturnus-refgen examples --model M --rom PATH --catalog FILE --reference FILE --out FILE
                           [--only NAME,NAME...]
  saturnus-refgen menus --model M --rom PATH --catalog FILE --categories FILE --out FILE

catalog   the ROM's command names (crates/saturnus-cli/data/commands/<model>.json)
examples  runs the reference's inputs (crates/saturnus-cli/data/commands/examples-<model>.json)
menus     each command's menus from the ROM's menu definitions, into the catalog
          (written to --out) and into categories.json (updated in place)
M is 48sx, 48gx or 49g.";

#[derive(Debug, Default)]
struct Args {
    model: Option<String>,
    rom: Option<PathBuf>,
    out: Option<PathBuf>,
    catalog: Option<PathBuf>,
    reference: Option<PathBuf>,
    categories: Option<PathBuf>,
    only: Option<Vec<String>>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = it
            .next()
            .with_context(|| format!("{a} needs a value\n{USAGE}"))?;
        match a.as_str() {
            "--model" => out.model = Some(v.clone()),
            "--rom" => out.rom = Some(v.into()),
            "--out" => out.out = Some(v.into()),
            "--catalog" => out.catalog = Some(v.into()),
            "--reference" => out.reference = Some(v.into()),
            "--categories" => out.categories = Some(v.into()),
            "--only" => out.only = Some(v.split(',').map(str::to_string).collect()),
            other => bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    Ok(out)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

/// Write each text to its file through a temporary file next to it,
/// renaming only once every temporary file is written.
fn write_all(files: &[(PathBuf, String)]) -> Result<()> {
    let tmp = |p: &Path| {
        let mut t = p.as_os_str().to_owned();
        t.push(".tmp");
        PathBuf::from(t)
    };
    for (path, text) in files {
        std::fs::write(tmp(path), text)
            .with_context(|| format!("cannot write {}", tmp(path).display()))?;
    }
    for (path, _) in files {
        std::fs::rename(tmp(path), path)
            .with_context(|| format!("cannot replace {}", path.display()))?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    if matches!(cmd.as_str(), "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let a = parse(&args[1..])?;
    let model = saturnus_host::model_from_name(a.model.as_deref().context("--model is required")?)
        .map_err(anyhow::Error::msg)?;
    let rom = a.rom.context("--rom is required")?;
    let out = a.out.context("--out is required")?;
    let text = match cmd.as_str() {
        "catalog" => {
            let cat = catalog::generate(model, &rom)?;
            eprintln!("{} commands", cat.commands.len());
            catalog::to_file_text(&cat)?
        }
        "examples" => {
            let cat: Catalog = read_json(&a.catalog.context("--catalog is required")?)?;
            let reference: Reference = read_json(&a.reference.context("--reference is required")?)?;
            let ex = examples::generate(model, &rom, &cat, &reference, a.only.as_deref())?;
            let n: usize = ex.examples.values().map(Vec::len).sum();
            let errors = ex
                .examples
                .values()
                .flatten()
                .filter(|e| e.error.is_some())
                .count();
            eprintln!(
                "{n} examples of {} commands ({errors} end in an error), {} skipped",
                ex.examples.len(),
                ex.skipped.len()
            );
            examples::to_file_text(&ex)?
        }
        "menus" => {
            let mut cat: Catalog = read_json(&a.catalog.context("--catalog is required")?)?;
            let categories_path = a.categories.context("--categories is required")?;
            let mut categories: serde_json::Value = read_json(&categories_path)?;
            let placement = menus::update(model, &rom, &mut cat, &mut categories)?;
            let placed = cat.commands.iter().filter(|c| !c.menus.is_empty()).count();
            eprintln!(
                "{placed} of {} commands in {} named menus; roots: {}",
                cat.commands.len(),
                placement.paths.len(),
                placement
                    .roots
                    .iter()
                    .map(|(n, l)| format!("{l}={n}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            // Both files or neither: write them aside, then rename.
            return write_all(&[
                (categories_path, menus::categories_text(&categories)?),
                (out, catalog::to_file_text(&cat)?),
            ]);
        }
        other => bail!("unknown command {other:?}\n{USAGE}"),
    };
    std::fs::write(&out, text).with_context(|| format!("cannot write {}", out.display()))?;
    Ok(())
}
