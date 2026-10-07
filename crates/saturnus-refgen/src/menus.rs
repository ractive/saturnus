//! Each command's menus, from the ROM's own menu definitions
//! (`saturnus_objects::menus`, read statically from the ROM image) and
//! the keyboard: which key opens which root menu is observed on the
//! running ROM by pressing it and reading the menu number from RAM (wiki:
//! hardware/hp48-system-ram, "Menu"); a submenu is named by its parent and
//! the label of the key that leads to it. No Kermit, no screen.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use saturnus::Model;
use saturnus::cpu::Bus as _;
use saturnus_mcp::emulator::Emulator;
use saturnus_objects::{Layout, NameTable, UserMemory, menus};
use serde_json::Value;

use crate::catalog::{self, Catalog, MenuKey};

/// A model's menu placement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// Root menus and the key legend that opens them, in keyboard order.
    pub roots: Vec<(u32, String)>,
    /// Each menu's name (`MTH HYP`), by number.
    pub paths: BTreeMap<u32, String>,
    /// Each command's menus.
    pub commands: BTreeMap<String, Vec<String>>,
}

/// The legend of the key that opens a root menu, used as a baseline.
const BASELINES: [&str; 2] = ["VAR", "MTH"];

/// The current menu number (0 when the pointer designates no menu).
fn menu(emu: &Emulator, defs: &menus::Menus) -> Result<u32> {
    emu.with_machine(|m| -> Result<u32> {
        let p = UserMemory::of(m)?.menu_pointer()?;
        Ok(defs.number(p, m).unwrap_or(0))
    })?
}

/// Set (or clear) system flag `-n` straight in RAM.
fn set_flag(emu: &Emulator, layout: Layout, n: u32, on: bool) -> Result<()> {
    let bit = n - 1;
    let addr = layout.system_flags + 16 * (bit / 64) + (bit % 64) / 4;
    emu.with_machine(|m| {
        let v = m.peek(addr);
        let mask = 1u8 << (bit % 4);
        m.hw.write_nibble(addr, if on { v | mask } else { v & !mask });
    })
}

/// The key script that presses a key with a shift (`""`, `leftshift`,
/// `rightshift`).
fn press(shift: &str, key: &str) -> String {
    if shift.is_empty() {
        format!("press {key}")
    } else {
        format!("press {shift}\npress {key}")
    }
}

/// Which key (with which shift) opens which menu: for every legend on the
/// model's keys, press it after each of two baseline keys that open
/// different menus; when both presses leave the same menu number, the key
/// opens that menu. ON is left out (it is OFF when shifted).
pub fn keyboard(emu: &mut Emulator, defs: &menus::Menus) -> Result<Vec<(u32, String)>> {
    let model = emu.model();
    let layout = Layout::of(model).context("no RAM layout for this model")?;
    if model == Model::Hp49g {
        // RPN, and soft menus instead of choose boxes (flags -95, -117).
        set_flag(emu, layout, 95, false)?;
        set_flag(emu, layout, 117, true)?;
    }
    let skin = saturnus_host::skins::skin(model);
    let legends: Vec<(&str, &str, &str)> = skin
        .keys
        .iter()
        .filter(|k| k.name != "on")
        .flat_map(|k| {
            [
                ("", k.label),
                ("leftshift", k.left),
                ("rightshift", k.right),
            ]
            .into_iter()
            .filter(|(_, l)| !l.is_empty())
            .map(move |(s, l)| (s, k.name, l))
        })
        .collect();
    let baseline = |legend: &str| {
        legends
            .iter()
            .find(|(_, _, l)| *l == legend)
            .map(|(s, k, _)| press(s, k))
            .with_context(|| format!("no {legend} key on the {}", model.name()))
    };
    let baselines = [baseline(BASELINES[0])?, baseline(BASELINES[1])?];
    let mut roots = Vec::new();
    for (shift, key, legend) in &legends {
        // A shifted key pressed after its shift; on the 48GX and 49G a
        // shifted key that opens an input form gives its menu when the
        // shift is held down while it is pressed.
        let mut scripts = vec![press(shift, key)];
        if !shift.is_empty() {
            scripts.push(format!("down {shift}\npress {key}\nup {shift}"));
        }
        for script in scripts {
            let mut seen = Vec::new();
            for b in &baselines {
                emu.press_keys(&format!("press on\n{b}"))?;
                emu.press_keys(&script)?;
                seen.push(menu(emu, defs)?);
                emu.press_keys("press on\npress on")?;
            }
            if seen[0] == seen[1] && seen[0] != 0 {
                if !roots.iter().any(|(n, _)| *n == seen[0]) {
                    roots.push((seen[0], (*legend).to_string()));
                }
                break;
            }
        }
    }
    Ok(roots)
}

/// Boot `model` and read its ROM's menu definitions.
fn read(model: Model, rom: &Path) -> Result<(Emulator, menus::Menus)> {
    let emu = catalog::boot(model, rom)?;
    let defs = emu
        .with_machine(|m| menus::read(m.rom_nibbles(), &NameTable::of(m)))?
        .context("the ROM's menu definitions were not found")?;
    if defs.truncated() {
        anyhow::bail!(
            "reading the ROM's menu definitions ran out of steps (MAX_STEPS): \
             the placement would be incomplete"
        );
    }
    Ok((emu, defs))
}

/// The placement the definitions `defs` give with the root menus
/// `roots`.
pub fn place(defs: &menus::Menus, roots: Vec<(u32, String)>) -> Placement {
    let paths = defs.paths(&roots);
    let commands = defs.placement(&paths);
    Placement {
        roots,
        paths,
        commands,
    }
}

/// The menu placement of `model` from the ROM at `rom`: the definitions
/// and the keyboard, observed.
pub fn generate(model: Model, rom: &Path) -> Result<Placement> {
    let (mut emu, defs) = read(model, rom)?;
    let roots = keyboard(&mut emu, &defs)?;
    Ok(place(&defs, roots))
}

/// The placement from the ROM's definitions with the root menus `cat`
/// already records (no keys pressed): what [`generate`] gives when the
/// keyboard is unchanged.
pub fn from_catalog(model: Model, rom: &Path, cat: &Catalog) -> Result<Placement> {
    let (_, defs) = read(model, rom)?;
    let roots = cat
        .menu_keys
        .iter()
        .map(|k| (k.menu, k.key.clone()))
        .collect();
    Ok(place(&defs, roots))
}

/// `catalog` with the root menus and each command's menus from `p`.
pub fn apply(catalog: &mut Catalog, p: &Placement) {
    // The method text the catalog step writes, which describes the menus.
    catalog.method = catalog::METHOD.to_string();
    catalog.menu_keys = p
        .roots
        .iter()
        .map(|(menu, key)| MenuKey {
            menu: *menu,
            key: key.clone(),
        })
        .collect();
    for c in &mut catalog.commands {
        c.menus = p.commands.get(&c.name).cloned().unwrap_or_default();
    }
}

/// The `menus` step: `catalog` (which must be `model`'s, checked before
/// booting) and `categories` with the menus read from the ROM at `rom`.
pub fn update(
    model: Model,
    rom: &Path,
    catalog: &mut Catalog,
    categories: &mut Value,
) -> Result<Placement> {
    let m = catalog::model_name(model);
    if catalog.model != m {
        anyhow::bail!(
            "the catalog is the {}'s, not the {m}'s: pass crates/saturnus-cli/data/commands/{m}.json",
            catalog.model
        );
    }
    let placement = generate(model, rom)?;
    apply(catalog, &placement);
    apply_categories(categories, m, catalog)?;
    Ok(placement)
}

/// `categories` (the parsed `categories.json`) with the ROM's menus of
/// `model` from `catalog`: per command and model a `menus` list next to
/// the manual's statement; a model entry without either is dropped.
pub fn apply_categories(categories: &mut Value, model: &str, catalog: &Catalog) -> Result<()> {
    let commands = categories
        .get_mut("commands")
        .and_then(Value::as_object_mut)
        .context("categories.json has no commands")?;
    // Menus of commands the catalog no longer has go first.
    let stale: Vec<String> = commands
        .keys()
        .filter(|n| !catalog.commands.iter().any(|c| &c.name == *n))
        .cloned()
        .collect();
    for name in stale {
        let Some(per_model) = commands.get_mut(&name).and_then(Value::as_object_mut) else {
            continue;
        };
        if let Some(o) = per_model.get_mut(model).and_then(Value::as_object_mut) {
            o.remove("menus");
            if o.is_empty() {
                per_model.remove(model);
            }
        }
        if per_model.is_empty() {
            commands.remove(&name);
        }
    }
    for c in &catalog.commands {
        let entry = commands
            .entry(c.name.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(per_model) = entry.as_object_mut() else {
            continue;
        };
        let slot = per_model
            .entry(model.to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(o) = slot.as_object_mut() {
            if c.menus.is_empty() {
                o.remove("menus");
            } else {
                o.insert("menus".into(), serde_json::json!(c.menus));
            }
        }
        if slot.as_object().is_some_and(serde_json::Map::is_empty) {
            per_model.remove(model);
        }
        let empty = entry.as_object().is_some_and(serde_json::Map::is_empty);
        if empty {
            commands.remove(&c.name);
        }
    }
    // Sorted keys, as the script writes them.
    let sorted: serde_json::Map<String, Value> = std::mem::take(commands)
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .map(|(k, mut v)| {
            if let Some(o) = v.as_object_mut() {
                let s: BTreeMap<String, Value> = std::mem::take(o).into_iter().collect();
                *o = s.into_iter().collect();
            }
            (k, v)
        })
        .collect();
    *commands = sorted;
    Ok(())
}

/// `categories.json`'s text as `scripts/manual-categories.py` writes it:
/// `method` first, then `commands`, two-space indents, a final newline.
pub fn categories_text(categories: &Value) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Doc<'a> {
        method: &'a Value,
        commands: &'a Value,
    }
    let doc = Doc {
        method: &categories["method"],
        commands: &categories["commands"],
    };
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Command;
    use serde_json::json;

    #[test]
    fn placement_goes_into_the_catalog_and_the_categories() {
        let mut cat = Catalog {
            model: "48sx".into(),
            method: String::new(),
            menu_keys: Vec::new(),
            commands: vec![
                Command {
                    name: "ABS".into(),
                    xlib: vec![[2, 1]],
                    menus: Vec::new(),
                },
                Command {
                    name: "SIN".into(),
                    xlib: vec![[2, 2]],
                    menus: vec!["OLD".into()],
                },
            ],
        };
        let p = Placement {
            roots: vec![(3, "MTH".into())],
            commands: BTreeMap::from([("ABS".into(), vec!["MTH PARTS".into()])]),
            ..Placement::default()
        };
        apply(&mut cat, &p);
        assert!(cat.method.contains("menus"));
        assert_eq!(
            cat.menu_keys,
            [MenuKey {
                menu: 3,
                key: "MTH".into()
            }]
        );
        assert_eq!(cat.commands[0].menus, ["MTH PARTS"]);
        assert!(cat.commands[1].menus.is_empty());
        let mut categories = json!({"method": "m", "commands": {
            "SIN": {"48sx": {"category": "Keyboard", "manual": "x", "page": 1, "menus": ["OLD"]}}
        }});
        // GONE left the catalog: its menus go, and its entry with them;
        // OLD keeps its manual statement.
        categories["commands"]["GONE"] =
            json!({"48sx": {"menus": ["X"]}, "48gx": {"menus": ["Y"]}});
        categories["commands"]["OLD"] = json!({"48sx": {"menus": ["X"], "category": "MTH"}});
        apply_categories(&mut categories, "48sx", &cat).unwrap();
        assert_eq!(
            categories["commands"],
            json!({
                "ABS": {"48sx": {"menus": ["MTH PARTS"]}},
                "GONE": {"48gx": {"menus": ["Y"]}},
                "OLD": {"48sx": {"category": "MTH"}},
                "SIN": {"48sx": {"category": "Keyboard", "manual": "x", "page": 1}}
            })
        );
    }

    #[test]
    fn a_catalog_of_another_model_is_refused_before_booting() {
        let mut cat = Catalog {
            model: "48sx".into(),
            method: String::new(),
            menu_keys: Vec::new(),
            commands: Vec::new(),
        };
        let mut categories = json!({"method": "m", "commands": {}});
        let e = update(
            Model::Hp49g,
            Path::new("/no/such/rom"),
            &mut cat,
            &mut categories,
        )
        .unwrap_err()
        .to_string();
        assert!(
            e.contains("the catalog is the 48sx's, not the 49g's"),
            "{e}"
        );
    }
}
