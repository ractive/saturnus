//! The command reference (`data/commands/`, made by `saturnus-refgen`):
//! the ROM's command list per model with its menu categories, our
//! descriptions and stack effects, the examples generated on the
//! emulator, and deep links into HP's manuals. Embedded at build time;
//! served by the `help` tool and as MCP resources.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

/// The models with a reference, in order.
pub const MODELS: [&str; 3] = ["48sx", "48gx", "49g"];

const CATALOGS: [&str; 3] = [
    include_str!("../../../data/commands/48sx.json"),
    include_str!("../../../data/commands/48gx.json"),
    include_str!("../../../data/commands/49g.json"),
];
const EXAMPLES: [&str; 3] = [
    include_str!("../../../data/commands/examples-48sx.json"),
    include_str!("../../../data/commands/examples-48gx.json"),
    include_str!("../../../data/commands/examples-49g.json"),
];
const REFERENCE: &str = include_str!("../../../data/commands/reference.json");
const MANUALS: &str = include_str!("../../../data/commands/manuals.json");

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
            .map(|t| serde_json::from_str(t))
            .collect::<std::result::Result<_, _>>()
            .context("catalog")?,
        examples: EXAMPLES
            .iter()
            .map(|t| serde_json::from_str(t))
            .collect::<std::result::Result<_, _>>()
            .context("examples")?,
        reference: serde_json::from_str(REFERENCE).context("reference")?,
        manuals: serde_json::from_str(MANUALS).context("manuals")?,
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
            "no command {q:?} on the 48SX, 48GX or 49G; read the saturnus://reference/index resource for the list"
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

/// The index of every command: name, models, category, description and
/// stack effect, without examples.
pub fn index() -> Result<Value> {
    let d = data()?;
    let commands: Vec<Value> = names(d)
        .into_iter()
        .map(|n| {
            let e = d.reference.commands.get(n);
            json!({
                "name": n,
                "models": availability(d, n).iter().map(|(m, _)| *m).collect::<Vec<_>>(),
                "category": category(d, n),
                "description": e.map(|e| e.description.as_str()),
                "stack": e.map(|e| e.stack.as_str()),
                "stack_verified": ran(d, n),
            })
        })
        .collect();
    let manuals: Vec<Value> = d
        .manuals
        .manuals
        .iter()
        .map(|m| json!({"id": m.id, "title": m.title, "url": m.url}))
        .collect();
    Ok(json!({"commands": commands, "manuals": manuals}))
}

/// URI of the index resource.
pub const INDEX_URI: &str = "saturnus://reference/index";
/// URI template of one command's entry.
pub const COMMAND_URI: &str = "saturnus://reference/command/{name}";

/// The text of the resource at `uri`, or `None` for an unknown URI.
pub fn read(uri: &str) -> Result<Option<String>> {
    if uri == INDEX_URI {
        return Ok(Some(serde_json::to_string(&index()?)?));
    }
    let Some(name) = uri.strip_prefix("saturnus://reference/command/") else {
        return Ok(None);
    };
    let name = percent_decode(name)?;
    Ok(Some(serde_json::to_string(&help(&name, None)?)?))
}

/// `%XX` escapes of a URI path segment, as UTF-8.
fn percent_decode(s: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut it = s.bytes();
    while let Some(b) = it.next() {
        if b == b'%' {
            let hex = [
                it.next().context("cut % escape")?,
                it.next().context("cut % escape")?,
            ];
            let hex = std::str::from_utf8(&hex).context("bad % escape")?;
            bytes.push(u8::from_str_radix(hex, 16).context("bad % escape")?);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).context("the command name is not UTF-8")
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
    }

    #[test]
    fn resources_by_uri() {
        let idx = read(INDEX_URI).unwrap().unwrap();
        assert!(idx.contains("\"STO\""));
        let one = read("saturnus://reference/command/%E2%86%92LIST")
            .unwrap()
            .unwrap();
        assert!(one.contains("\u{2192}LIST"));
        assert!(read("saturnus://other").unwrap().is_none());
    }
}
