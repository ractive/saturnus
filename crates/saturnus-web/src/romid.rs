//! Which calculator a ROM image belongs to, by its content, and which
//! remembered slots a batch of images fills: one set of rules for every
//! host (the Worker through [`identify_rom`] and [`plan_roms`], the
//! desktop app natively). No I/O, so it builds for `wasm32`.
//!
//! An image is **exact** when its SHA-256 is one of the images saturnus
//! knows ([`KNOWN`]: those `saturnus rom fetch` downloads and the 49G
//! fallback), which names its models and revision. Otherwise it **fits**
//! the models whose loader takes its size and form (packed, or unpacked
//! with one nibble per byte), or it is **unknown**. The 39G and 40G share
//! one image. Nothing ambiguous is assigned without the user's say
//! ([`plan`]).

use saturnus::Model;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::sha256;

/// The ROM images saturnus knows: SHA-256 (lowercase hex), the models
/// that run it, and its revision.
pub const KNOWN: [(&str, &[Model], &str); 6] = [
    (
        "e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74",
        &[Model::Hp48sx],
        "48SX J",
    ),
    (
        "de3a5a07b0f00640f4ba3599ea4092e9473113aad75c04bd03d3e37c059b5b33",
        &[Model::Hp48gx],
        "48GX R",
    ),
    (
        "3c9f747f637757d3adc414ed14d7f3636033f34f0a72e6e453ee197987f16be7",
        &[Model::Hp38g],
        "38G A1.67",
    ),
    (
        "b01c13e24a692f35e6087106d58ec205b4696d5b5e35d57f8f94015f8bb1f1ca",
        &[Model::Hp49g],
        "49G 2.15",
    ),
    (
        "58c3de6b7fc75a0ba65fca7437c4d49d8f26ca9e334a57e4d3bc4f8fb2dc8c11",
        &[Model::Hp49g],
        "49G 2.10",
    ),
    (
        "69220f42d5e90dd8825e7d1596d9eaca490ee6a7a52a3b8b96469a5f3d3f627f",
        &[Model::Hp39g, Model::Hp40g],
        "39G/40G (hpcalc rom3940)",
    ),
];

/// The revision of the image with SHA-256 `sha256`, if saturnus knows it.
pub fn revision(sha256: &str) -> Option<&'static str> {
    KNOWN
        .iter()
        .find(|(s, _, _)| *s == sha256)
        .map(|&(_, _, r)| r)
}

/// What an image is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    /// A known image: the models that run it and its revision.
    Exact {
        models: Vec<Model>,
        revision: &'static str,
    },
    /// Not known, but its size and form fit these models (one or more).
    Fits(Vec<Model>),
    /// No model takes it.
    Unknown,
}

impl Identity {
    /// The models the image could be for.
    pub fn models(&self) -> &[Model] {
        match self {
            Identity::Exact { models, .. } | Identity::Fits(models) => models,
            Identity::Unknown => &[],
        }
    }
}

/// An identified image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomId {
    pub sha256: String,
    pub identity: Identity,
}

impl RomId {
    /// The revision of an exact image.
    pub fn revision(&self) -> Option<&'static str> {
        match self.identity {
            Identity::Exact { revision, .. } => Some(revision),
            _ => None,
        }
    }
}

/// The models whose loader takes `rom`, by size and form: a 2 MB file is a
/// packed 49G flash or, when every byte is a nibble, an unpacked 39G/40G;
/// a 4 MB file is an unpacked 49G only when every byte is a nibble. The
/// 48GX and 38G both take 512 KB.
pub fn fitting_models(rom: &[u8]) -> Vec<Model> {
    const TWO_MB: usize = 2 * 1024 * 1024;
    let nibbles = || rom.iter().all(|&b| b < 16);
    let unpacked_2mb = rom.len() == TWO_MB && nibbles();
    Model::ALL
        .into_iter()
        .filter(|&m| m.accepts_rom_len(rom.len()))
        .filter(|&m| match m {
            Model::Hp49g if rom.len() == TWO_MB => !unpacked_2mb,
            Model::Hp49g => nibbles(),
            Model::Hp39g | Model::Hp40g if rom.len() == TWO_MB => unpacked_2mb,
            _ => true,
        })
        .collect()
}

/// Identify `rom` by its content.
pub fn identify(rom: &[u8]) -> RomId {
    let sha256 = sha256::hex_digest(rom);
    let identity = match KNOWN.iter().find(|(s, _, _)| *s == sha256) {
        Some(&(_, models, revision)) => Identity::Exact {
            models: models.to_vec(),
            revision,
        },
        None => match fitting_models(rom) {
            m if m.is_empty() => Identity::Unknown,
            m => Identity::Fits(m),
        },
    };
    RomId { sha256, identity }
}

/// A file of a batch for [`plan`].
#[derive(Clone, Debug)]
pub struct Candidate {
    /// The file's name, for the notice.
    pub name: String,
    pub identity: Identity,
    /// Chosen by the user (a dialog, the picker, a drop), not found
    /// beside the chosen file.
    pub chosen: bool,
}

/// What to do with a batch: indexes into the candidates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Slot `model` takes candidate `n`.
    pub assign: Vec<(Model, usize)>,
    /// Candidate `n` could be the ROM of these models; the user decides.
    pub offer: Vec<(Vec<Model>, usize)>,
    /// The model to boot: the selected one if it got a ROM, else the
    /// first a chosen file was assigned to.
    pub boot: Option<Model>,
    /// One line for the user: what was assigned besides the boot, what
    /// is offered, and why a chosen file was not taken.
    pub notice: String,
}

fn title(m: Model) -> String {
    m.name().to_uppercase()
}

/// "48GX", "39G and 40G", "48GX or 38G".
fn titles(models: &[Model], conjunction: &str) -> String {
    let t: Vec<String> = models.iter().map(|&m| title(m)).collect();
    match t.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} {conjunction} {last}", init.join(", ")),
    }
}

/// Assign a batch of identified files to the model slots, `selected`
/// being the model the user chose them for and `filled` the slots that
/// already hold a ROM. The rules:
///
/// - A chosen file that is exact fills every slot of its models (the user
///   gave it). One that fits fills `selected` if it fits it, or its one
///   model if it fits only one; one that fits several others is offered.
///   An unknown one is refused in the notice.
/// - A file found beside it that is exact fills the empty slots of its
///   models; one that only fits is offered for its empty slots; an
///   unknown one is left alone. Found files never replace a slot.
/// - Within a batch the first file for a slot wins (hosts sort the found
///   files by name).
pub fn plan(selected: Model, files: &[Candidate], filled: &[Model]) -> Plan {
    let mut plan = Plan::default();
    let taken = |plan: &Plan, m: Model| plan.assign.iter().any(|&(a, _)| a == m);
    let mut refused = Vec::new();
    let mut found = Vec::new();
    // Chosen files first, so they win over found ones.
    let order = files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.chosen)
        .chain(files.iter().enumerate().filter(|(_, f)| !f.chosen));
    for (n, f) in order {
        let empty: Vec<Model> = f
            .identity
            .models()
            .iter()
            .copied()
            .filter(|&m| !taken(&plan, m) && (f.chosen || !filled.contains(&m)))
            .collect();
        match (&f.identity, f.chosen) {
            (Identity::Unknown, true) => refused.push(format!(
                "{} is not a ROM image of a model saturnus runs",
                f.name
            )),
            (Identity::Unknown, false) => {}
            (Identity::Exact { models, .. }, _) => {
                if !f.chosen && !empty.is_empty() {
                    found.push(format!("{} for the {}", f.name, titles(&empty, "and")));
                }
                // A chosen file whose models an earlier chosen file took
                // (two 49G revisions at once) is named, not dropped.
                if f.chosen && empty.is_empty() {
                    refused.push(format!(
                        "{} was not used: another file in the batch is the {} ROM",
                        f.name,
                        titles(models, "and")
                    ));
                }
                plan.assign.extend(empty.iter().map(|&m| (m, n)));
            }
            (Identity::Fits(models), true) => {
                let target = if models.contains(&selected) {
                    Some(selected)
                } else if let [one] = models.as_slice() {
                    Some(*one)
                } else {
                    None
                };
                match target {
                    Some(m) if !taken(&plan, m) => plan.assign.push((m, n)),
                    Some(m) => refused.push(format!(
                        "{} was not used: another file in the batch is the {} ROM",
                        f.name,
                        title(m)
                    )),
                    None => plan.offer.push((models.clone(), n)),
                }
            }
            (Identity::Fits(_), false) => {
                if !empty.is_empty() {
                    plan.offer.push((empty, n));
                }
            }
        }
    }
    let chosen_models = || {
        plan.assign
            .iter()
            .filter(|&&(_, n)| files[n].chosen)
            .map(|&(m, _)| m)
    };
    plan.boot = if chosen_models().any(|m| m == selected) {
        Some(selected)
    } else {
        chosen_models().next()
    };
    let mut parts = Vec::new();
    if !found.is_empty() {
        parts.push(format!("Also found: {}.", found.join(", ")));
    }
    for (models, n) in &plan.offer {
        parts.push(format!(
            "{} could be the {} ROM.",
            files[*n].name,
            titles(models, "or")
        ));
    }
    for r in refused {
        parts.push(format!("{r}."));
    }
    plan.notice = parts.join(" ");
    plan
}

/// [`RomId`] as JSON: `{sha256, kind: "exact"|"fits"|"unknown", models,
/// revision}`.
pub fn rom_id_json(id: &RomId) -> Value {
    let kind = match id.identity {
        Identity::Exact { .. } => "exact",
        Identity::Fits(_) => "fits",
        Identity::Unknown => "unknown",
    };
    let models: Vec<&str> = id.identity.models().iter().map(|m| m.name()).collect();
    json!({"sha256": id.sha256, "kind": kind, "models": models, "revision": id.revision()})
}

fn identity_from_json(v: &Value) -> Result<Identity, String> {
    let models = v
        .get("models")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| crate::model_from_name(m.as_str().unwrap_or_default()))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(match v.get("kind").and_then(Value::as_str) {
        Some("exact") => {
            let sha = v.get("sha256").and_then(Value::as_str).unwrap_or_default();
            let revision = revision(sha).ok_or("an exact image of no known revision")?;
            Identity::Exact { models, revision }
        }
        Some("fits") => Identity::Fits(models),
        _ => Identity::Unknown,
    })
}

/// [`plan`] over JSON: `input` is `{selected, filled: [model], files:
/// [{name, chosen, id}]}` with `id` from [`rom_id_json`]; the result is
/// `{assign: [{model, file}], offer: [{models, file}], boot, notice}`.
pub fn plan_json(input: &Value) -> Result<Value, String> {
    let selected = crate::model_from_name(
        input
            .get("selected")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let filled = input
        .get("filled")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| crate::model_from_name(m.as_str().unwrap_or_default()))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let files = input
        .get("files")
        .and_then(Value::as_array)
        .ok_or("\"files\" is missing")?
        .iter()
        .map(|f| {
            Ok(Candidate {
                name: f
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                chosen: f.get("chosen").and_then(Value::as_bool).unwrap_or(false),
                identity: identity_from_json(f.get("id").unwrap_or(&Value::Null))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let p = plan(selected, &files, &filled);
    Ok(json!({
        "assign": p.assign.iter().map(|&(m, n)| json!({"model": m.name(), "file": n})).collect::<Vec<_>>(),
        "offer": p.offer.iter().map(|(ms, n)| json!({
            "models": ms.iter().map(|m| m.name()).collect::<Vec<_>>(),
            "file": n,
        })).collect::<Vec<_>>(),
        "boot": p.boot.map(Model::name),
        "notice": p.notice,
    }))
}

/// [`identify`] for the Worker, see [`rom_id_json`].
#[wasm_bindgen]
pub fn identify_rom(rom: &[u8]) -> Result<JsValue, JsValue> {
    crate::json_value(&rom_id_json(&identify(rom)).to_string())
}

/// [`plan_json`] for the Worker; `input` is JSON text.
#[wasm_bindgen]
pub fn plan_roms(input: &str) -> Result<JsValue, JsValue> {
    let v: Value = serde_json::from_str(input).map_err(|e| crate::js_err(e.to_string()))?;
    let out = plan_json(&v).map_err(crate::js_err)?;
    crate::json_value(&out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use Model::*;

    const KB: usize = 1024;

    fn fits(rom: &[u8]) -> Identity {
        identify(rom).identity
    }

    #[test]
    fn synthetic_images_fit_by_size_and_form() {
        assert_eq!(fits(&[0; 64 * KB]), Identity::Fits(vec![Hp42s]));
        assert_eq!(fits(&[0; 256 * KB]), Identity::Fits(vec![Hp48sx]));
        assert_eq!(fits(&[0; 512 * KB]), Identity::Fits(vec![Hp48gx, Hp38g]));
        assert_eq!(fits(&[0; 1024 * KB]), Identity::Fits(vec![Hp39g, Hp40g]));
        // 2 MB of nibbles: an unpacked 39G/40G; with a byte above 15, a
        // packed 49G flash.
        let mut two = vec![0x0f; 2048 * KB];
        assert_eq!(fits(&two), Identity::Fits(vec![Hp39g, Hp40g]));
        two[7] = 0x10;
        assert_eq!(fits(&two), Identity::Fits(vec![Hp49g]));
        // 4 MB: an unpacked 49G only if every byte is a nibble.
        let mut four = vec![0u8; 4096 * KB];
        assert_eq!(fits(&four), Identity::Fits(vec![Hp49g]));
        four[0] = 0xff;
        assert_eq!(fits(&four), Identity::Unknown);
        assert_eq!(fits(&[0; 1000]), Identity::Unknown);
        assert_eq!(fits(&[]), Identity::Unknown);
    }

    #[test]
    fn known_images_are_exact_and_fit_their_models() {
        for (sha, models, rev) in KNOWN {
            assert_eq!(sha.len(), 64);
            assert_eq!(revision(sha), Some(rev));
            assert!(!models.is_empty());
        }
        assert_eq!(revision(&"0".repeat(64)), None);
        let id = identify(b"abc");
        assert_eq!(
            id.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(id.revision(), None);
    }

    fn exact(models: &[Model]) -> Identity {
        Identity::Exact {
            models: models.to_vec(),
            revision: "test",
        }
    }

    fn file(name: &str, identity: Identity, chosen: bool) -> Candidate {
        Candidate {
            name: name.into(),
            identity,
            chosen,
        }
    }

    /// One ROM chosen for the 48SX in a folder holding the others: the
    /// exact ones fill their empty slots, the 39G/40G image both; the
    /// unknown file is left alone; the 42S dump only fits, so it is
    /// offered; the slot the user set (the 38G) is kept.
    #[test]
    fn a_chosen_rom_and_its_folder() {
        let files = [
            file("sxrom-j", exact(&[Hp48sx]), true),
            file("38G_A167.ROM", exact(&[Hp38g]), false),
            file("gxrom-r", exact(&[Hp48gx]), false),
            file("hp42s-c.rom", Identity::Fits(vec![Hp42s]), false),
            file("notes.bin", Identity::Unknown, false),
            file("rom-2.10.49g", exact(&[Hp49g]), false),
            file("rom.39g", exact(&[Hp39g, Hp40g]), false),
            file("rom.49g", exact(&[Hp49g]), false),
        ];
        let p = plan(Hp48sx, &files, &[Hp38g]);
        assert_eq!(
            p.assign,
            [(Hp48sx, 0), (Hp48gx, 2), (Hp49g, 5), (Hp39g, 6), (Hp40g, 6)]
        );
        assert_eq!(p.offer, [(vec![Hp42s], 3)]);
        assert_eq!(p.boot, Some(Hp48sx));
        assert_eq!(
            p.notice,
            "Also found: gxrom-r for the 48GX, rom-2.10.49g for the 49G, rom.39g for the 39G and \
             40G. hp42s-c.rom could be the 42S ROM."
        );
        // With every slot filled, a folder offers and assigns nothing.
        let all: Vec<Model> = Model::ALL.to_vec();
        let p = plan(Hp48sx, &files, &all);
        assert_eq!(p.assign, [(Hp48sx, 0)]);
        assert!(p.offer.is_empty());
        assert_eq!(p.notice, "");
    }

    #[test]
    fn chosen_files_by_identity() {
        // A 512 KB unknown image chosen for the 48GX goes there; for the
        // 48SX it is ambiguous: offered, not booted.
        let gx_or_38 = || file("x.rom", Identity::Fits(vec![Hp48gx, Hp38g]), true);
        let p = plan(Hp48gx, &[gx_or_38()], &[]);
        assert_eq!(
            (p.assign.as_slice(), p.boot),
            (&[(Hp48gx, 0)][..], Some(Hp48gx))
        );
        let p = plan(Hp48sx, &[gx_or_38()], &[]);
        assert!(p.assign.is_empty());
        assert_eq!(p.offer, [(vec![Hp48gx, Hp38g], 0)]);
        assert_eq!(p.boot, None);
        assert_eq!(p.notice, "x.rom could be the 48GX or 38G ROM.");
        // A known image chosen for another model boots its own, replacing
        // a slot the user set before.
        let p = plan(Hp48sx, &[file("g", exact(&[Hp48gx]), true)], &[Hp48gx]);
        assert_eq!(
            (p.assign.as_slice(), p.boot),
            (&[(Hp48gx, 0)][..], Some(Hp48gx))
        );
        // A file of one fitting model goes to it.
        let p = plan(Hp48sx, &[file("d", Identity::Fits(vec![Hp42s]), true)], &[]);
        assert_eq!(p.boot, Some(Hp42s));
        // Unknown: refused, nothing assigned.
        let p = plan(Hp48sx, &[file("a.txt", Identity::Unknown, true)], &[]);
        assert!(p.assign.is_empty() && p.boot.is_none());
        assert_eq!(
            p.notice,
            "a.txt is not a ROM image of a model saturnus runs."
        );
        // Several chosen at once (the page's picker): each to its model;
        // the selected one boots; the second file for a slot is not used.
        let files = [
            file("a", exact(&[Hp48gx]), true),
            file("b", exact(&[Hp48sx]), true),
            file("c", Identity::Fits(vec![Hp48sx]), true),
        ];
        let p = plan(Hp48sx, &files, &[]);
        assert_eq!(p.assign, [(Hp48gx, 0), (Hp48sx, 1)]);
        assert_eq!(p.boot, Some(Hp48sx));
        assert_eq!(
            p.notice,
            "c was not used: another file in the batch is the 48SX ROM."
        );
        // Two exact images of one model at once: the first is used, the
        // second named.
        let files = [
            file("rom.49g", exact(&[Hp49g]), true),
            file("rom-2.10.49g", exact(&[Hp49g]), true),
        ];
        let p = plan(Hp49g, &files, &[]);
        assert_eq!(p.assign, [(Hp49g, 0)]);
        assert_eq!(
            p.notice,
            "rom-2.10.49g was not used: another file in the batch is the 49G ROM."
        );
    }

    #[test]
    fn plan_over_json() {
        let id = rom_id_json(&identify(&[0; 256 * KB]));
        assert_eq!(id["kind"], "fits");
        assert_eq!(id["models"], json!(["48sx"]));
        assert_eq!(id["revision"], Value::Null);
        let out = plan_json(&json!({
            "selected": "48sx",
            "filled": [],
            "files": [{"name": "a", "chosen": true, "id": id}],
        }))
        .unwrap();
        assert_eq!(out["assign"], json!([{"model": "48sx", "file": 0}]));
        assert_eq!(out["boot"], "48sx");
        assert!(plan_json(&json!({"selected": "99x", "files": []})).is_err());
    }

    /// The seven images of `SATURNUS_ROM_DIR` (skipped without it): six
    /// are known exactly; the 42S has no published image, so its dump
    /// fits only the 42S.
    #[test]
    fn rom_dir_images() {
        let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let want: [(&str, Identity); 7] = [
            ("sxrom-j", exact(&[Hp48sx])),
            ("gxrom-r", exact(&[Hp48gx])),
            ("38G_A167.ROM", exact(&[Hp38g])),
            ("rom.49g", exact(&[Hp49g])),
            ("rom-2.10.49g", exact(&[Hp49g])),
            ("rom.39g", exact(&[Hp39g, Hp40g])),
            ("hp42s-c.rom", Identity::Fits(vec![Hp42s])),
        ];
        for (name, identity) in want {
            let Ok(rom) = std::fs::read(dir.join(name)) else {
                eprintln!("{name} not in SATURNUS_ROM_DIR, skipped");
                continue;
            };
            let got = identify(&rom).identity;
            match (&got, &identity) {
                (Identity::Exact { models: a, .. }, Identity::Exact { models: b, .. }) => {
                    assert_eq!(a, b, "{name}")
                }
                _ => assert_eq!(got, identity, "{name}"),
            }
        }
    }
}
