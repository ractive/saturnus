//! `saturnus ref`: the command reference (`data/commands/`, made by
//! `saturnus-refgen`): the ROM's command list per model with its menu
//! categories, our descriptions and stack effects, the examples generated
//! on the emulator, and deep links into HP's manuals. Embedded at build
//! time; read only.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

/// The models with a reference, in order.
pub const MODELS: [&str; 3] = ["48sx", "48gx", "49g"];

/// The files, deflated by `build.rs`.
macro_rules! packed {
    ($file:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $file, ".deflate"))
    };
}
const CATALOGS: [&[u8]; 3] = [
    packed!("48sx.json"),
    packed!("48gx.json"),
    packed!("49g.json"),
];
const EXAMPLES: [&[u8]; 3] = [
    packed!("examples-48sx.json"),
    packed!("examples-48gx.json"),
    packed!("examples-49g.json"),
];
const REFERENCE: &[u8] = packed!("reference.json");
const MANUALS: &[u8] = packed!("manuals.json");

/// An embedded file parsed as `T`.
fn unpack<T: serde::de::DeserializeOwned>(packed: &[u8]) -> Result<T> {
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed)
        .map_err(|e| anyhow::anyhow!("cannot inflate: {e:?}"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    commands: Vec<CatalogCommand>,
}

#[derive(Debug, Deserialize)]
struct CatalogCommand {
    name: String,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExamplesFile {
    examples: BTreeMap<String, Vec<Value>>,
    skipped: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ReferenceFile {
    commands: BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    description: String,
    stack: String,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManualsFile {
    manuals: Vec<Manual>,
    pages: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Debug, Deserialize)]
struct Manual {
    id: String,
    title: String,
    url: String,
}

/// Everything parsed once.
#[derive(Debug)]
struct Data {
    catalogs: Vec<CatalogFile>,
    examples: Vec<ExamplesFile>,
    reference: ReferenceFile,
    manuals: ManualsFile,
}

fn data() -> Result<&'static Data> {
    static DATA: OnceLock<std::result::Result<Data, String>> = OnceLock::new();
    DATA.get_or_init(|| parse().map_err(|e| format!("{e:#}")))
        .as_ref()
        .map_err(|e| anyhow::anyhow!("the embedded command reference is broken: {e}"))
}

fn parse() -> Result<Data> {
    Ok(Data {
        catalogs: CATALOGS
            .iter()
            .map(|p| unpack(p))
            .collect::<Result<_>>()
            .context("catalog")?,
        examples: EXAMPLES
            .iter()
            .map(|p| unpack(p))
            .collect::<Result<_>>()
            .context("examples")?,
        reference: unpack(REFERENCE).context("reference")?,
        manuals: unpack(MANUALS).context("manuals")?,
    })
}

/// The models that have `name`, with its category on each.
fn availability(d: &Data, name: &str) -> Vec<(&'static str, Option<String>)> {
    MODELS
        .iter()
        .zip(&d.catalogs)
        .filter_map(|(m, c)| {
            c.commands
                .iter()
                .find(|x| x.name == name)
                .map(|x| (*m, x.category.clone()))
        })
        .collect()
}

/// `name`'s category: the ROM's menu on the first model that has one,
/// else ours.
fn category(d: &Data, name: &str) -> Option<String> {
    availability(d, name)
        .into_iter()
        .find_map(|(_, c)| c)
        .or_else(|| d.reference.commands.get(name)?.category.clone())
}

/// Deep links into the manuals for `name`.
fn links(d: &Data, name: &str) -> Vec<Value> {
    let Some(pages) = d.manuals.pages.get(name) else {
        return Vec::new();
    };
    d.manuals
        .manuals
        .iter()
        .filter_map(|m| {
            pages.get(&m.id).map(
                |p| json!({"manual": m.title, "page": p, "url": format!("{}#page={p}", m.url)}),
            )
        })
        .collect()
}

/// All command names of every model, in sorted order.
fn names(d: &Data) -> Vec<&str> {
    let mut v: Vec<&str> = d
        .catalogs
        .iter()
        .flat_map(|c| c.commands.iter().map(|x| x.name.as_str()))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Whether an example ran `name` itself without an error on some model:
/// only then has the emulator confirmed the stack effect; otherwise it is
/// taken from the manuals and unverified.
fn ran(d: &Data, name: &str) -> bool {
    d.examples.iter().any(|f| {
        f.examples.get(name).is_some_and(|list| {
            list.iter().any(|x| {
                x.get("error").is_none()
                    && x.get("run")
                        .and_then(Value::as_str)
                        .is_some_and(|r| r.split_whitespace().any(|w| w == name))
            })
        })
    })
}

/// The full entry of `name`: description, stack effect, category, the
/// models that have it, manual links and the generated examples (on
/// `model` only, when given).
pub fn help(name: &str, model: Option<&str>) -> Result<Value> {
    let d = data()?;
    let name = resolve(d, name)?;
    if let Some(m) = model
        && !MODELS.contains(&m)
    {
        bail!("no reference for model {m:?}: expected 48sx, 48gx or 49g");
    }
    let avail = availability(d, name);
    let entry = d.reference.commands.get(name);
    let mut examples = serde_json::Map::new();
    let mut skipped = serde_json::Map::new();
    for (m, file) in MODELS.iter().zip(&d.examples) {
        if model.is_some_and(|x| x != *m) || !avail.iter().any(|(a, _)| a == m) {
            continue;
        }
        if let Some(list) = file.examples.get(name) {
            examples.insert((*m).to_string(), Value::Array(list.clone()));
        } else if let Some(why) = file.skipped.get(name) {
            skipped.insert((*m).to_string(), Value::String(why.clone()));
        }
    }
    let mut out = json!({
        "name": name,
        "models": avail.iter().map(|(m, _)| *m).collect::<Vec<_>>(),
        "category": category(d, name),
        "description": entry.map(|e| e.description.as_str()),
        "stack": entry.map(|e| e.stack.as_str()),
        "stack_verified": ran(d, name),
        "manuals": links(d, name),
        "examples": examples,
    });
    if !skipped.is_empty()
        && let Some(o) = out.as_object_mut()
    {
        o.insert("no_examples".into(), Value::Object(skipped));
    }
    Ok(out)
}

/// The command `query` names: exact, else case-insensitive, else ASCII
/// spellings of HP characters (`->STR`, `SIGMA+`); else an error listing
/// close names.
fn resolve<'a>(d: &'a Data, query: &str) -> Result<&'a str> {
    let all = names(d);
    let q = query.trim();
    if let Some(n) = all.iter().find(|n| **n == q) {
        return Ok(n);
    }
    let folded = ascii_spelling(q).to_uppercase();
    if let Some(n) = all
        .iter()
        .find(|n| ascii_spelling(n).to_uppercase() == folded)
    {
        return Ok(n);
    }
    let close: Vec<&str> = all
        .iter()
        .copied()
        .filter(|n| ascii_spelling(n).to_uppercase().contains(&folded))
        .take(20)
        .collect();
    if close.is_empty() {
        bail!(
            "no command {q:?} on the 48SX, 48GX or 49G; see data/commands/reference.json for the list"
        );
    }
    bail!("no command {q:?}; did you mean one of: {}", close.join(" "))
}

/// HP characters spelled the way the calculator's ASCII transfers do.
fn ascii_spelling(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2192}' => "->".to_string(),
            '\u{3a3}' => "SIGMA".to_string(),
            '\u{3c0}' => "PI".to_string(),
            '\u{2202}' => "DER".to_string(),
            '\u{222b}' => "INT".to_string(),
            '\u{221a}' => "SQRT".to_string(),
            '\u{2264}' => "<=".to_string(),
            '\u{2265}' => ">=".to_string(),
            '\u{2260}' => "<>".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// `help`'s entry as text for the terminal.
pub fn text(entry: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut out = s(&entry["name"]);
    if let Some(c) = entry["category"].as_str() {
        out.push_str(&format!("  ({c})"));
    }
    let models: Vec<String> = entry["models"]
        .as_array()
        .map(|a| a.iter().map(s).collect())
        .unwrap_or_default();
    out.push_str(&format!("\nmodels: {}\n", models.join(" ")));
    let unverified = if entry["stack_verified"] == true {
        ""
    } else {
        "  (from the manuals, not run here)"
    };
    out.push_str(&format!("stack:  {}{unverified}\n", s(&entry["stack"])));
    out.push_str(&format!("\n{}\n", s(&entry["description"])));
    if let Some(examples) = entry["examples"].as_object() {
        for (model, list) in examples {
            out.push_str(&format!("\nexamples ({model}):\n"));
            for x in list.as_array().into_iter().flatten() {
                let source = [s(&x["setup"]), s(&x["input"]), s(&x["run"])]
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let result = match x["error"].as_str() {
                    Some(e) => format!("Error: {e}"),
                    None => {
                        let shown: Vec<String> = x["display"]
                            .as_array()
                            .map(|a| a.iter().rev().map(s).collect())
                            .unwrap_or_default();
                        shown.join("  ")
                    }
                };
                out.push_str(&format!("  {source}  =>  {result}\n"));
            }
        }
    }
    if let Some(why) = entry["no_examples"].as_object() {
        for (model, reason) in why {
            out.push_str(&format!("\nno example ({model}): {}\n", s(reason)));
        }
    }
    if let Some(links) = entry["manuals"].as_array().filter(|a| !a.is_empty()) {
        out.push_str("\nmanuals:\n");
        for l in links {
            out.push_str(&format!(
                "  {}, p. {}: {}\n",
                s(&l["manual"]),
                l["page"],
                s(&l["url"])
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_reference_parses_and_covers_the_48sx() {
        let d = data().unwrap();
        for c in &d.catalogs[0].commands {
            let e = d.reference.commands.get(&c.name);
            assert!(e.is_some(), "{} has no reference entry", c.name);
            assert!(category(d, &c.name).is_some(), "{} has no category", c.name);
        }
    }

    #[test]
    fn help_finds_commands_by_ascii_spelling() {
        let h = help("sto", None).unwrap();
        assert_eq!(h["name"], "STO");
        assert_eq!(h["stack_verified"], true);
        assert_eq!(help("OFF", None).unwrap()["stack_verified"], false);
        assert!(h["models"].as_array().unwrap().len() >= 2);
        assert!(!h["examples"]["48sx"].as_array().unwrap().is_empty());
        assert_eq!(help("->STR", Some("48sx")).unwrap()["name"], "\u{2192}STR");
        let e = help("NOSUCHCMD", None).unwrap_err().to_string();
        assert!(e.contains("no command"), "{e}");
        assert!(help("SIN", Some("50g")).is_err());
        let t = text(&help("STO", Some("48sx")).unwrap());
        assert!(t.starts_with("STO"), "{t}");
        assert!(
            t.contains("examples (48sx):") && t.contains("#page="),
            "{t}"
        );
    }
}
