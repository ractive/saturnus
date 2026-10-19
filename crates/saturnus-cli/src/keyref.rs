//! `saturnus ref` for the key-driven models (the 42S, 27S, 17BII, 17B,
//! 19BII, the 38G, 39G and 40G, and any model with a
//! `data/commands/keys-<model>.json`): no command line to type into, so a
//! function is looked up with the keys that run it (on the 38G, 39G and
//! 40G: that put it on HOME's edit line), what it asks for, its menu, the
//! examples run on the emulator (the screen before and after) and its
//! page in the manual. The format is `saturnus-refgen`'s
//! (`crates/saturnus-refgen/src/keyed.rs`); embedded at build time.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::reference::{Found, friendly, from_codes, to_codes};

include!(concat!(env!("OUT_DIR"), "/keyed.rs"));

/// One key press: the key's script name and what it shows.
#[derive(Clone, Debug, Deserialize)]
pub struct Step {
    pub key: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Alpha {
    start: Vec<Step>,
    chars: BTreeMap<String, Vec<Step>>,
}

#[derive(Debug, Deserialize)]
struct Xeq {
    start: Vec<Step>,
    end: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Command {
    name: String,
    #[serde(default)]
    long: Option<String>,
    description: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    stack: Option<String>,
    #[serde(default)]
    param: Option<String>,
    #[serde(default)]
    keys: Option<Vec<Step>>,
    #[serde(default)]
    xeq: Option<String>,
    #[serde(default)]
    menu: Option<String>,
    #[serde(default)]
    inserts: Option<String>,
    page: u32,
    /// The example specs, for their comments.
    #[serde(default)]
    examples: Vec<Spec>,
}

/// An example spec: the keys it pressed, and its comment.
#[derive(Debug, Deserialize)]
struct Spec {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    then: Vec<String>,
    #[serde(default)]
    comment: Option<String>,
}

/// The generated examples of a function, each with the comment of its
/// spec: the one at the same index when it pressed the same keys, else the
/// first unused one that did (as `scripts/commands-json.py` joins them).
fn with_comments(specs: &[Spec], list: &[Value]) -> Vec<Value> {
    let keys = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|k| k.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut free: Vec<usize> = (0..specs.len()).collect();
    list.iter()
        .enumerate()
        .map(|(j, x)| {
            let (input, then) = (keys(&x["input"]), keys(&x["then"]));
            let same = |i: usize| specs[i].input == input && specs[i].then == then;
            let hit = if free.contains(&j) && same(j) {
                Some(j)
            } else {
                free.iter().copied().find(|&i| same(i))
            };
            let mut out = x.clone();
            if let Some(i) = hit {
                free.retain(|&k| k != i);
                if let (Some(c), Some(o)) = (&specs[i].comment, out.as_object_mut()) {
                    o.insert("comment".into(), Value::String(c.clone()));
                }
            }
            out
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct KeyFile {
    model: String,
    manual: String,
    #[serde(default)]
    alpha: Option<Alpha>,
    #[serde(default)]
    xeq: Option<Xeq>,
    commands: Vec<Command>,
}

#[derive(Debug, Default, Deserialize)]
struct ExamplesFile {
    #[serde(default)]
    examples: BTreeMap<String, Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct ManualsFile {
    manuals: Vec<Manual>,
}

#[derive(Debug, Deserialize)]
struct Manual {
    id: String,
    title: String,
    url: String,
}

/// One model's reference, parsed.
#[derive(Debug)]
struct Model {
    file: KeyFile,
    examples: ExamplesFile,
}

fn inflate<T: serde::de::DeserializeOwned>(packed: &[u8]) -> Result<T> {
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed)
        .map_err(|e| anyhow::anyhow!("cannot inflate: {e:?}"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn parsed() -> Result<&'static Vec<Model>> {
    static DATA: OnceLock<std::result::Result<Vec<Model>, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        KEYED
            .iter()
            .map(|(m, keys, ex)| {
                let file: KeyFile = inflate(keys).map_err(|e| format!("keys-{m}.json: {e:#}"))?;
                let examples = if ex.is_empty() {
                    ExamplesFile::default()
                } else {
                    inflate(ex).map_err(|e| format!("examples-{m}.json: {e:#}"))?
                };
                Ok(Model { file, examples })
            })
            .collect()
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!("the embedded key-driven reference is broken: {e}"))
}

/// The key-driven models with a reference, in order.
pub fn models() -> Vec<&'static str> {
    KEYED.iter().map(|(m, _, _)| *m).collect()
}

/// Whether a shift key.
fn is_shift(key: &str) -> bool {
    matches!(key, "shift" | "leftshift" | "rightshift")
}

/// The steps as they read: labels in order, `⇧` for a shift.
pub fn keys_text(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| match &s.label {
            Some(l) => l.clone(),
            None if is_shift(&s.key) => "⇧".to_string(),
            None => s.key.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The keys that run `c` from the normal state: its keys, or XEQ, the
/// letter entry, its name's characters and the end of XEQ.
fn steps(f: &KeyFile, c: &Command) -> Option<Vec<Step>> {
    if let Some(k) = &c.keys {
        return Some(k.clone());
    }
    let (name, xeq, alpha) = (c.xeq.as_ref()?, f.xeq.as_ref()?, f.alpha.as_ref()?);
    let mut out = xeq.start.clone();
    out.extend(alpha.start.iter().cloned());
    for ch in name.chars() {
        out.extend(alpha.chars.get(&ch.to_string())?.iter().cloned());
    }
    out.extend(xeq.end.iter().cloned());
    Some(out)
}

/// The function of `model` that `query` names: the exact name, then the
/// same letters in another case, the calculator's ASCII codes or a
/// friendly spelling (`->POL`, `SIGMA+`); several are listed.
pub fn lookup(model: &str, query: &str) -> Result<Found> {
    let all = parsed()?;
    let Some(m) = all.iter().find(|m| m.file.model == model) else {
        bail!("no key-driven reference for {model:?}");
    };
    let q = query.trim();
    let names: Vec<&str> = m.file.commands.iter().map(|c| c.name.as_str()).collect();
    let name = if let Some(n) = names.iter().find(|n| **n == q) {
        *n
    } else {
        let upper = q.to_uppercase();
        let decoded = from_codes(q);
        let hits: Vec<&str> = names
            .iter()
            .copied()
            .filter(|n| {
                n.to_uppercase() == upper
                    || decoded.as_deref() == Some(*n)
                    || to_codes(n).to_uppercase() == upper
                    || (!n.is_ascii() && friendly(n) == upper)
            })
            .collect();
        match hits.as_slice() {
            [one] => *one,
            [] => {
                let close: Vec<&str> = names
                    .iter()
                    .copied()
                    .filter(|n| n.to_uppercase().contains(&upper) || friendly(n).contains(&upper))
                    .take(20)
                    .collect();
                if close.is_empty() {
                    bail!("no function {q:?} on the {model}");
                }
                bail!(
                    "no function {q:?} on the {model}; did you mean one of: {}",
                    close.join(" ")
                );
            }
            _ => return Ok(Found::Many(hits.iter().map(|s| (*s).to_string()).collect())),
        }
    };
    let Some(c) = m.file.commands.iter().find(|c| c.name == name) else {
        bail!("no function {name:?} on the {model}");
    };
    let manuals: ManualsFile = crate::reference::manuals_file()?;
    let manual = manuals.manuals.iter().find(|x| x.id == m.file.manual);
    let steps = steps(&m.file, c);
    let keys = match (&c.keys, &c.xeq) {
        (Some(k), _) => Some(keys_text(k)),
        (None, Some(x)) => Some(format!("XEQ \"{x}\"")),
        _ => None,
    };
    Ok(Found::One(json!({
        "name": c.name,
        "models": [model],
        "long_name": c.long,
        "description": c.description,
        "note": c.note,
        "stack": c.stack,
        "param": c.param,
        "keys": keys,
        "key_names": steps.as_ref().map(|s| s.iter().map(|x| x.key.clone()).collect::<Vec<_>>()),
        "xeq": c.xeq,
        "menu": c.menu,
        "inserts": c.inserts,
        "manuals": manual.map(|x| vec![json!({
            "manual": x.title,
            "page": c.page,
            "url": format!("{}#page={}", x.url, c.page),
        })]).unwrap_or_default(),
        "examples": m.examples.examples.get(&c.name).map(|l| with_comments(&c.examples, l)).unwrap_or_default(),
        "keyed": true,
    })))
}

/// The models that have a function `query` names exactly (or by another
/// case or spelling), for a lookup with no model.
pub fn models_with(query: &str) -> Vec<&'static str> {
    models()
        .into_iter()
        .filter(|m| matches!(lookup(m, query), Ok(Found::One(_)) | Ok(Found::Many(_))))
        .collect()
}

/// Key names as the keys read on `model`'s skin: a shifted key by its
/// shifted legend after `⇧`, a key after ALPHA by its letter; a name the
/// skin lacks in capitals.
pub fn legend_text(model: &str, names: &[String]) -> String {
    let skin = model
        .parse::<saturnus_core::Model>()
        .ok()
        .and_then(saturnus_host::skins::skin);
    let mut out = Vec::new();
    let mut shifted = false;
    let mut alpha = false;
    for n in names {
        if is_shift(n) {
            out.push("⇧".to_string());
            shifted = !shifted;
            continue;
        }
        let k = skin.and_then(|sk| sk.keys.iter().find(|k| k.name == n));
        let legend = k
            .map(|k| match (alpha && !k.alpha.is_empty(), shifted) {
                (true, _) => k.alpha,
                (false, true) => k.left,
                (false, false) => k.label,
            })
            .unwrap_or("");
        alpha = n == "alpha";
        out.push(if legend.is_empty() {
            n.to_uppercase()
        } else {
            legend.to_string()
        });
        shifted = false;
    }
    out.join(" ")
}

/// A key-driven model's entry as text for the terminal.
pub fn text(entry: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let model = entry["models"][0].as_str().unwrap_or("");
    let mut out = s(&entry["name"]);
    if let Some(m) = entry["menu"].as_str() {
        out.push_str(&format!("  ({m})"));
    }
    if let Some(long) = entry["long_name"].as_str() {
        out.push_str(&format!("\nname:   {long}"));
    }
    out.push_str(&format!("\nmodels: {model}\n"));
    if let Some(k) = entry["keys"].as_str() {
        out.push_str(&format!("keys:   {k}\n"));
    }
    if let Some(i) = entry["inserts"].as_str() {
        out.push_str(&format!("puts:   {} (on the edit line)\n", i.trim_end()));
    }
    if let Some(p) = entry["param"].as_str() {
        out.push_str(&format!("asks:   {p}\n"));
    }
    if let Some(st) = entry["stack"].as_str() {
        out.push_str(&format!("stack:  {st}\n"));
    }
    out.push_str(&format!("\n{}\n", s(&entry["description"])));
    if let Some(note) = entry["note"].as_str() {
        out.push_str(&format!("note:   {note}\n"));
    }
    if let Some(list) = entry["examples"].as_array().filter(|a| !a.is_empty()) {
        out.push_str(&format!(
            "\nexamples ({model}, the display before and after):\n"
        ));
        for x in list {
            let keys = |v: &Value| -> Vec<String> {
                v.as_array()
                    .map(|a| a.iter().map(s).collect())
                    .unwrap_or_default()
            };
            let names = keys(&x["input"]);
            let then = keys(&x["then"]);
            // The lines with something on them, as they read (HOME's
            // results stand at the right; their spaces are dropped here).
            let lines = |v: &Value| -> String {
                let l: Vec<String> = v
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| s(x).trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                if l.is_empty() {
                    "blank".to_string()
                } else {
                    l.join(" / ")
                }
            };
            let after = if then.is_empty() {
                String::new()
            } else {
                format!(" {}", legend_text(model, &then))
            };
            let labels = keys(&x["labels"]);
            out.push_str(&format!(
                "  {}  [{}]  {}{after}  =>  [{}]\n",
                if labels.is_empty() {
                    legend_text(model, &names)
                } else {
                    labels.join(" ")
                },
                lines(&x["before"]),
                s(&entry["keys"]),
                lines(&x["after"])
            ));
            if let Some(c) = x["comment"].as_str() {
                out.push_str(&format!("  # {c}\n"));
            }
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

    fn one(model: &str, q: &str) -> Value {
        match lookup(model, q).unwrap() {
            Found::One(v) => v,
            Found::Many(v) => panic!("{q}: several: {v:?}"),
        }
    }

    #[test]
    fn every_function_has_keys_to_run_it_or_a_menu_to_type_it_in() {
        for m in parsed().unwrap() {
            assert!(!m.file.commands.is_empty(), "{}", m.file.model);
            for c in &m.file.commands {
                // A function only typed into an equation (the Solver's)
                // has no keys but says where it is typed.
                let typed = c.keys.is_none() && c.xeq.is_none() && c.menu.is_some();
                let s = steps(&m.file, c);
                assert!(
                    typed || s.is_some_and(|s| !s.is_empty()),
                    "{} {}",
                    m.file.model,
                    c.name
                );
            }
        }
        for m in ["42s", "27s", "17bii", "17b", "19bii"] {
            assert!(models().contains(&m), "{m}");
        }
    }

    #[test]
    fn the_17bii_by_menu_keys_and_its_solver_functions() {
        let n = one("17bii", "n");
        assert_eq!(n["keys"], "FIN TVM N");
        assert_eq!(n["key_names"], json!(["a", "a", "a"]));
        assert_eq!(n["menu"], "FIN TVM");
        let t = text(&n);
        assert!(
            t.starts_with(
                "N  (FIN TVM)\nname:   Number of periods\nmodels: 17bii\nkeys:   FIN TVM N\n"
            ),
            "{t}"
        );
        // A Solver function: no keys, typed into an equation.
        let uspv = one("17bii", "USPV(i%:n)");
        assert_eq!(uspv["keys"], Value::Null);
        assert_eq!(uspv["menu"], "SOLVE");
        assert!(!text(&uspv).contains("keys:"));
        // The same name on several key-driven models.
        assert_eq!(models_with("I%YR"), vec!["17b", "17bii", "19bii", "27s"]);
    }

    #[test]
    fn an_example_shows_its_comment_under_it() {
        let ch = one("17bii", "%CH");
        assert_eq!(
            ch["examples"][0]["comment"],
            "From 100 to 125 is a rise of 25 percent"
        );
        let t = text(&ch);
        assert!(
            t.contains("=>  [%CHANGE=25.00 / [OLD][NEW][%CH][][][]]\n  # From 100 to 125 is a rise of 25 percent\n"),
            "{t}"
        );
        assert!(one("17bii", "%T")["examples"][0].get("comment").is_none());
    }

    #[test]
    fn the_39g_puts_a_function_on_the_edit_line() {
        assert!(models().contains(&"39g"));
        let comb = one("39g", "comb");
        assert_eq!(comb["keys"], "MATH P P ▶ ENTER");
        assert_eq!(comb["inserts"], "COMB(");
        assert_eq!(comb["menu"], "MATH Prob.");
        let t = text(&comb);
        assert!(t.contains("puts:   COMB( (on the edit line)\n"), "{t}");
        // The example types the arguments and ENTER after the keys.
        assert!(t.contains("MATH P P ▶ ENTER 5 , 2 ) ENTER  =>"), "{t}");
        assert!(t.contains("COMB(5,2)"), "{t}");
        assert_eq!(one("40g", "->HMS")["name"], "\u{2192}HMS");
        assert_eq!(one("38g", "SIN")["keys"], "SIN");
        let names = ["alpha", "vars", "shift", "alpha", "math", "1"].map(String::from);
        assert_eq!(legend_text("39g", &names), "ALPHA A ⇧ alpha B 1");
    }

    #[test]
    fn the_42s_by_name_spelling_and_keys() {
        let pol = one("42s", "->pol");
        assert_eq!(pol["name"], "\u{2192}POL");
        assert_eq!(pol["keys"], "⇧ CONVERT →POL");
        assert_eq!(pol["key_names"], json!(["shift", "5", "xeq"]));
        assert_eq!(pol["menu"], "CONVERT");
        assert!(
            pol["manuals"][0]["url"]
                .as_str()
                .unwrap()
                .ends_with("#page=93")
        );
        let t = text(&pol);
        assert!(
            t.starts_with(
                "→POL  (CONVERT)\nname:   To polar\nmodels: 42s\nkeys:   ⇧ CONVERT →POL\n"
            ),
            "{t}"
        );
        assert!(
            t.contains("4 ENTER 3  [y:4.0000 / x:3_]  ⇧ CONVERT →POL  =>  [y:53.1301 / x:5.0000]"),
            "{t}"
        );
        // Run by name: XEQ and the name, its keys spelt through the ALPHA menu.
        let sinh = one("42s", "sinh");
        assert_eq!(sinh["keys"], "XEQ \"SINH\"");
        let k = sinh["key_names"].as_array().unwrap();
        assert_eq!(k.first().unwrap(), "xeq");
        assert_eq!(k.last().unwrap(), "enter");
        assert_eq!(one("42s", "SIGMA+")["name"], "\u{3a3}+");
        assert_eq!(one("42s", "\\GS+")["name"], "\u{3a3}+");
        assert!(
            lookup("42s", "NOSUCH")
                .unwrap_err()
                .to_string()
                .contains("no function")
        );
        assert!(lookup("27x", "SIN").is_err());
        assert_eq!(
            models_with("SINH"),
            vec!["19bii", "27s", "38g", "39g", "40g", "42s"]
        );
        assert_eq!(models_with("MAKELIST"), vec!["38g", "39g", "40g"]);
        let names = ["2", "enter", "1", "sigmaplus", "shift", "sto"].map(String::from);
        assert_eq!(legend_text("42s", &names), "2 ENTER 1 Σ+ ⇧ COMPLEX");
    }
}
