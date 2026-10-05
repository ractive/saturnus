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
const CATEGORIES: &[u8] = packed!("categories.json");

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
}

/// `categories.json`: per command and model, where a manual places it.
#[derive(Debug, Deserialize)]
struct CategoriesFile {
    commands: BTreeMap<String, BTreeMap<String, Placed>>,
}

#[derive(Debug, Deserialize)]
struct Placed {
    category: String,
    manual: String,
    page: u32,
    /// From another model's manual (the 48G AUR for the 48SX).
    #[serde(default)]
    other_model: bool,
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
    models: Vec<String>,
}

/// Everything parsed once.
#[derive(Debug)]
struct Data {
    catalogs: Vec<CatalogFile>,
    examples: Vec<ExamplesFile>,
    reference: ReferenceFile,
    manuals: ManualsFile,
    categories: CategoriesFile,
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
        categories: unpack(CATEGORIES).context("categories")?,
    })
}

/// The models that have `name`, with its category on each: where that
/// model's manual (or, failing it, another model's manual) places it,
/// else our own category (`source: "ours"`), else none.
fn availability(d: &Data, name: &str) -> Vec<(&'static str, Option<Value>)> {
    let ours = d
        .reference
        .commands
        .get(name)
        .and_then(|e| e.category.clone());
    let placed = d.categories.commands.get(name);
    MODELS
        .iter()
        .zip(&d.catalogs)
        .filter(|(_, c)| c.commands.iter().any(|x| x.name == name))
        .map(|(m, _)| {
            let from_manual = placed.and_then(|p| p.get(*m)).map(|p| {
                let manual = d.manuals.manuals.iter().find(|x| x.id == p.manual);
                let mut v = json!({
                    "category": p.category,
                    "source": "manual",
                    "manual": manual.map_or(p.manual.as_str(), |x| x.title.as_str()),
                    "page": p.page,
                    "url": manual.map(|x| format!("{}#page={}", x.url, p.page)),
                });
                if p.other_model
                    && let Some(o) = v.as_object_mut()
                {
                    // Another model's manual: its keys may differ.
                    o.insert("other_model".into(), Value::Bool(true));
                }
                v
            });
            let cat = from_manual.or_else(|| {
                ours.as_ref()
                    .map(|c| json!({"category": c, "source": "ours"}))
            });
            (*m, cat)
        })
        .collect()
}

/// Deep links into the manuals for `name`, from the manuals of `models`.
fn links(d: &Data, name: &str, models: &[&str]) -> Vec<Value> {
    let Some(pages) = d.manuals.pages.get(name) else {
        return Vec::new();
    };
    d.manuals
        .manuals
        .iter()
        .filter(|m| m.models.iter().any(|x| models.contains(&x.as_str())))
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

/// Whether an example ran `name` itself without an error on one of
/// `models`: only then has the emulator confirmed the stack effect;
/// otherwise it is taken from the manuals and unverified.
fn ran(d: &Data, name: &str, models: &[&str]) -> bool {
    MODELS.iter().zip(&d.examples).any(|(m, f)| {
        models.contains(m)
            && f.examples.get(name).is_some_and(|list| {
                list.iter().any(|x| {
                    x.get("error").is_none()
                        && x.get("run")
                            .and_then(Value::as_str)
                            .is_some_and(|r| r.split_whitespace().any(|w| w == name))
                })
            })
    })
}

/// What a lookup found.
#[derive(Debug)]
pub enum Found {
    /// One command's entry.
    One(Value),
    /// Several commands the query could mean, sorted.
    Many(Vec<String>),
}

/// Look `query` up: the entry of the one command it names (see
/// [`resolve`]), restricted to `model` when given, or the candidates when
/// it names several.
pub fn lookup(query: &str, model: Option<&str>) -> Result<Found> {
    let d = data()?;
    if let Some(m) = model
        && !MODELS.contains(&m)
    {
        bail!("no reference for model {m:?}: expected 48sx, 48gx or 49g");
    }
    let name = match resolve(d, query)? {
        Resolved::One(n) => n,
        Resolved::Many(v) => return Ok(Found::Many(v.into_iter().map(String::from).collect())),
    };
    let mut avail = availability(d, name);
    if let Some(m) = model {
        if !avail.iter().any(|(a, _)| *a == m) {
            let on: Vec<&str> = avail.iter().map(|(a, _)| *a).collect();
            bail!("{name} is not a {m} command (it is on: {})", on.join(" "));
        }
        avail.retain(|(a, _)| *a == m);
    }
    let models: Vec<&str> = avail.iter().map(|(m, _)| *m).collect();
    let entry = d.reference.commands.get(name);
    let mut examples = serde_json::Map::new();
    let mut skipped = serde_json::Map::new();
    for (m, file) in MODELS.iter().zip(&d.examples) {
        if !models.contains(m) {
            continue;
        }
        if let Some(list) = file.examples.get(name) {
            examples.insert((*m).to_string(), Value::Array(list.clone()));
        } else if let Some(why) = file.skipped.get(name) {
            skipped.insert((*m).to_string(), Value::String(why.clone()));
        }
    }
    let categories: serde_json::Map<String, Value> = avail
        .iter()
        .map(|(m, c)| ((*m).to_string(), c.clone().unwrap_or(Value::Null)))
        .collect();
    let first = avail.iter().find_map(|(_, c)| c.clone());
    let mut out = json!({
        "name": name,
        "models": models,
        "category": first.as_ref().map(|c| c["category"].clone()),
        "category_source": first,
        "categories": categories,
        "description": entry.map(|e| e.description.as_str()),
        "stack": entry.map(|e| e.stack.as_str()),
        "stack_verified": ran(d, name, &models),
        "manuals": links(d, name, &models),
        "examples": examples,
    });
    if !skipped.is_empty()
        && let Some(o) = out.as_object_mut()
    {
        o.insert("no_examples".into(), Value::Object(skipped));
    }
    Ok(Found::One(out))
}

/// What a query names.
#[derive(Debug, PartialEq, Eq)]
enum Resolved<'a> {
    One(&'a str),
    Many(Vec<&'a str>),
}

/// The command(s) `query` names. An exact name wins. Otherwise every name
/// that the query spells counts: the same letters in another case, the
/// calculator's ASCII translation codes (`\->LIST`, `\GS+`, `\.S`), or a
/// friendly spelling (`->LIST`, `SIGMA+`) when it is no other command's
/// name and no other command's friendly spelling. One such name is the
/// answer; several are listed for the caller to choose.
fn resolve<'a>(d: &'a Data, query: &str) -> Result<Resolved<'a>> {
    let all = names(d);
    let q = query.trim();
    if let Some(n) = all.iter().find(|n| **n == q) {
        return Ok(Resolved::One(n));
    }
    let upper = q.to_uppercase();
    let decoded = from_codes(q);
    let mut hits: Vec<&str> = all
        .iter()
        .copied()
        .filter(|n| {
            n.to_uppercase() == upper
                || decoded.as_deref() == Some(*n)
                || friendly_unique(&all, n).is_some_and(|f| f == upper)
        })
        .collect();
    hits.dedup();
    match hits.as_slice() {
        [one] => return Ok(Resolved::One(one)),
        [] => {}
        _ => return Ok(Resolved::Many(hits)),
    }
    let close: Vec<&str> = all
        .iter()
        .copied()
        .filter(|n| {
            n.to_uppercase().contains(&upper)
                || friendly(n).contains(&upper)
                || to_codes(n).to_uppercase().contains(&upper)
        })
        .take(20)
        .collect();
    if close.is_empty() {
        bail!("no command {q:?} on the 48SX, 48GX or 49G");
    }
    bail!("no command {q:?}; did you mean one of: {}", close.join(" "))
}

/// The calculator's translation codes for the HP characters that occur in
/// command names: what an ASCII transfer (translation mode 3) writes for
/// them (wiki: protocols/hp-object-format, "trigraphs"). Unambiguous: no
/// command name contains a backslash.
const CODES: [(char, &str); 18] = [
    ('\u{221a}', "\\v/"),
    ('\u{222b}', "\\.S"),
    ('\u{3a3}', "\\GS"),
    ('\u{25b6}', "\\|>"),
    ('\u{3c0}', "\\pi"),
    ('\u{2202}', "\\.d"),
    ('\u{2264}', "\\<="),
    ('\u{2265}', "\\>="),
    ('\u{2260}', "\\=/"),
    ('\u{2192}', "\\->"),
    ('\u{2193}', "\\|v"),
    ('\u{2191}', "\\|^"),
    ('\u{3bb}', "\\Gl"),
    ('\u{394}', "\\GD"),
    ('\u{3a0}', "\\PI"),
    ('\u{221e}', "\\oo"),
    ('\u{ab}', "\\<<"),
    ('\u{bb}', "\\>>"),
];

/// `name` with its HP characters as translation codes.
fn to_codes(name: &str) -> String {
    name.chars()
        .map(|c| {
            CODES
                .iter()
                .find(|(h, _)| *h == c)
                .map_or_else(|| c.to_string(), |(_, code)| (*code).to_string())
        })
        .collect()
}

/// `query` with translation codes turned into HP characters, `None` when
/// it holds none.
fn from_codes(query: &str) -> Option<String> {
    if !query.contains('\\') {
        return None;
    }
    let mut out = query.to_string();
    for (h, code) in CODES {
        out = out.replace(code, &h.to_string());
    }
    Some(out)
}

/// A friendly ASCII spelling of `name`, uppercase: `->LIST`, `SIGMA+`.
fn friendly(name: &str) -> String {
    name.chars()
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
            '\u{2191}' => "UP".to_string(),
            '\u{2193}' => "DOWN".to_string(),
            '\u{394}' => "DELTA".to_string(),
            '\u{3a0}' => "PROD".to_string(),
            '\u{3bb}' => "LAMBDA".to_string(),
            '\u{25b6}' => ">".to_string(),
            '\u{221e}' => "INF".to_string(),
            '\u{ab}' => "<<".to_string(),
            '\u{bb}' => ">>".to_string(),
            c => c.to_uppercase().to_string(),
        })
        .collect()
}

/// [`friendly`] when `name` has HP characters and that spelling is no
/// other command's name or friendly spelling (`INT` is the 49G's INT, so
/// `∫` has no friendly spelling, only `\.S`).
fn friendly_unique(all: &[&str], name: &str) -> Option<String> {
    if name.is_ascii() {
        return None;
    }
    let f = friendly(name);
    let clash = all
        .iter()
        .any(|o| *o != name && (o.to_uppercase() == f || (!o.is_ascii() && friendly(o) == f)));
    (!clash).then_some(f)
}

/// Our own category as text: a grouping, not where the key is.
fn ours_label(c: &Value) -> String {
    format!(
        "{} (ours; not a menu location)",
        c["category"].as_str().unwrap_or("")
    )
}

/// `help`'s entry as text for the terminal.
pub fn text(entry: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut out = s(&entry["name"]);
    if let Some(c) = entry["category_source"].as_object() {
        let c = Value::Object(c.clone());
        if c["source"] == "ours" {
            out.push_str(&format!("  (group: {})", ours_label(&c)));
        } else {
            out.push_str(&format!("  ({})", s(&c["category"])));
        }
    }
    let models: Vec<String> = entry["models"]
        .as_array()
        .map(|a| a.iter().map(s).collect())
        .unwrap_or_default();
    out.push_str(&format!("\nmodels: {}\n", models.join(" ")));
    if let Some(cats) = entry["categories"].as_object() {
        // A manual's statement is where the command is; our own
        // category is only a grouping for browsing.
        let label = |c: &Value| match c["source"].as_str() {
            Some("ours") => format!("group {}", ours_label(c)),
            Some(_) => format!(
                "menu {} ({}, p. {})",
                s(&c["category"]),
                s(&c["manual"]),
                c["page"]
            ),
            None => "-".to_string(),
        };
        let distinct: std::collections::BTreeSet<String> = cats.values().map(&label).collect();
        if distinct.len() > 1 {
            let per: Vec<String> = MODELS
                .iter()
                .filter_map(|m| cats.get(*m).map(|c| format!("{m} {}", label(c))))
                .collect();
            out.push_str(&format!("where:  {}\n", per.join("; ")));
        } else if let Some(c) = entry["category_source"].as_object() {
            let c = Value::Object(c.clone());
            if c["source"] == "ours" {
                out.push_str(&format!("group:  {}\n", ours_label(&c)));
            } else {
                out.push_str(&format!(
                    "menu:   {} ({}, p. {})\n",
                    s(&c["category"]),
                    s(&c["manual"]),
                    c["page"]
                ));
            }
        }
    }
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

/// The candidates of an ambiguous query as text.
pub fn candidates_text(query: &str, names: &[String]) -> String {
    let lines: Vec<String> = names
        .iter()
        .map(|n| format!("  {n}  (also {})", to_codes(n)))
        .collect();
    format!(
        "{query:?} names several commands; ask for one:\n{}\n",
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(q: &str, model: Option<&str>) -> Value {
        match lookup(q, model).unwrap() {
            Found::One(v) => v,
            Found::Many(v) => panic!("{q}: several: {v:?}"),
        }
    }

    #[test]
    fn the_embedded_reference_covers_every_command() {
        let d = data().unwrap();
        for c in d.catalogs.iter().flat_map(|f| &f.commands) {
            assert!(
                d.reference.commands.contains_key(&c.name),
                "{} has no entry",
                c.name
            );
            let cat = availability(d, &c.name);
            assert!(!cat.is_empty(), "{} is on no model", c.name);
        }
    }

    #[test]
    fn every_hp_character_in_a_name_has_a_code_and_round_trips() {
        let d = data().unwrap();
        let all = names(d);
        for n in &all {
            for c in n.chars().filter(|c| !c.is_ascii()) {
                assert!(
                    CODES.iter().any(|(h, _)| *h == c),
                    "{c:?} (in {n}) has no translation code"
                );
            }
            assert!(!n.contains('\\'), "{n} holds a backslash");
            let coded = to_codes(n);
            assert_eq!(resolve(d, &coded).unwrap(), Resolved::One(n), "{coded}");
        }
    }

    #[test]
    fn exact_names_win_and_ascii_spellings_do_not_collide() {
        let d = data().unwrap();
        assert_eq!(resolve(d, "INT").unwrap(), Resolved::One("INT"));
        assert_eq!(resolve(d, "int").unwrap(), Resolved::One("INT"));
        assert_eq!(resolve(d, "\\.S").unwrap(), Resolved::One("\u{222b}"));
        assert_eq!(resolve(d, "SIGMA").unwrap(), Resolved::One("SIGMA"));
        assert_eq!(resolve(d, "\\GS").unwrap(), Resolved::One("\u{3a3}"));
        assert_eq!(resolve(d, "->list").unwrap(), Resolved::One("\u{2192}LIST"));
        assert_eq!(resolve(d, "sigma+").unwrap(), Resolved::One("\u{3a3}+"));
        assert_eq!(
            resolve(d, "UPMATCH").unwrap(),
            Resolved::One("\u{2191}MATCH")
        );
        assert_eq!(resolve(d, "qr").unwrap(), Resolved::One("qr"));
        assert_eq!(resolve(d, "Qr").unwrap(), Resolved::Many(vec!["QR", "qr"]));
        assert!(
            resolve(d, "NOSUCHCMD")
                .unwrap_err()
                .to_string()
                .contains("no command")
        );
        assert!(matches!(lookup("Qr", None).unwrap(), Found::Many(_)));
    }

    #[test]
    fn the_model_selects_category_examples_and_manuals() {
        let all = one("AMORT", None);
        assert_eq!(all["category"], "SOLVE");
        assert_eq!(all["category_source"]["source"], "manual");
        assert_eq!(all["categories"]["48gx"]["page"], 176);
        assert!(all["categories"]["48gx"].get("other_model").is_none());
        assert_eq!(all["categories"]["49g"]["source"], "ours");
        let drop = one("DROP", None);
        assert_eq!(drop["categories"]["48sx"]["other_model"], true);
        let h = one("ASR", Some("49g"));
        assert_eq!(h["models"], json!(["49g"]));
        assert!(h["examples"].get("48sx").is_none() && h["examples"].get("49g").is_some());
        assert_eq!(h["stack_verified"], true);
        let manuals: Vec<&str> = h["manuals"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["manual"].as_str())
            .collect();
        assert!(manuals.iter().all(|m| m.contains("49G")), "{manuals:?}");
        let gcd = one("ABCUV", Some("49g"));
        assert_eq!(gcd["category"], "Arithmetic");
        assert_eq!(gcd["category_source"]["source"], "manual");
        let ours = one("NIP", None);
        assert_eq!(ours["category_source"]["source"], "ours");
        let t = text(&ours);
        assert!(
            t.contains("group:  ") && t.contains("(ours; not a menu location)"),
            "{t}"
        );
        assert!(!t.contains("menu:"), "{t}");
        let asr = text(&one("ASR", Some("49g")));
        assert!(
            asr.starts_with("ASR  (group: ") && !asr.contains("menu:"),
            "{asr}"
        );
        let both = text(&one("AMORT", None));
        assert!(both.contains("where:  48gx menu SOLVE (HP 48G"), "{both}");
        assert!(
            both.contains("49g group SOLVE TVM (ours; not a menu location)"),
            "{both}"
        );
        let abs = one("ABS", Some("48sx"));
        assert!(
            text(&abs).contains("menu:   MTH (HP 48SX Owner's Manual, p. "),
            "{}",
            text(&abs)
        );
        assert!(lookup("ASR", Some("50g")).is_err());
        assert!(lookup("GCD", Some("48sx")).is_err());
        assert_eq!(one("OFF", None)["stack_verified"], false);
        let t = text(&one("STO", Some("48sx")));
        assert!(t.starts_with("STO"), "{t}");
        assert!(
            t.contains("examples (48sx):") && t.contains("#page="),
            "{t}"
        );
    }
}
