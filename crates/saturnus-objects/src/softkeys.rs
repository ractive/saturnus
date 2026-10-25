//! What the six menu keys show now (48SX, 48GX, 49G), read from RAM
//! without pressing a key: the current menu (its pointer, wiki:
//! hardware/hp48-system-ram, "Menu") and its page (the first key shown,
//! same section), the menu's keys from the ROM's own definition
//! ([`crate::menus`]), the VAR menu's from the current directory.
//!
//! Only while the stack has the keyboard ([`crate::screen`]): an input
//! form, a choose box or another application draws its own keys. A menu
//! the ROM builds in RAM (the 49G's TOOL menu, a form's menu), the CST
//! menu and a menu whose definition gives no keys read as unknown.

use saturnus_core::Machine;
use serde::Serialize;

use crate::error::Error;
use crate::menus::{MenuKey, Menus};
use crate::ram::{UserMemory, Variable};

type Result<T, E = Error> = std::result::Result<T, E>;

/// Keys on one page of a menu.
pub const PAGE: usize = 6;

/// The VAR menu's number (`2 MENU`).
pub const VAR_MENU: u32 = 2;

/// One menu key as the screen shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownKey {
    /// What its label says: the command's or variable's name, or the
    /// definition's own label.
    pub label: String,
    /// The one command it runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// It opens another menu.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub submenu: bool,
    /// A variable's type, as `G D` names it (`Real Number`, `Directory`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl ShownKey {
    fn of_key(k: &MenuKey) -> Option<ShownKey> {
        Some(ShownKey {
            label: k.text()?.to_string(),
            command: k.command.clone(),
            submenu: k.submenu.is_some(),
            kind: None,
        })
    }

    fn of_variable(v: &Variable) -> ShownKey {
        ShownKey {
            label: v.name.clone(),
            command: None,
            submenu: false,
            kind: Some(v.kind.clone()),
        }
    }
}

/// The page that starts at key `first` (from 0) of a menu of `count`
/// keys: the six keys' indexes, `None` past the end; `None` for a page
/// that does not start inside the menu.
fn page(first: usize, count: usize) -> Option<[Option<usize>; PAGE]> {
    if first >= count.max(1) || !first.is_multiple_of(PAGE) {
        return None;
    }
    Some(std::array::from_fn(|i| {
        let k = first + i;
        (k < count).then_some(k)
    }))
}

/// The six keys of menu `number` on the page starting at key `first`:
/// from the definition in `defs`, or for the VAR menu from `variables`
/// (the current directory's, newest first). `None` where the keys are
/// not known.
pub fn shown(
    defs: &Menus,
    number: u32,
    first: usize,
    variables: Option<&[Variable]>,
) -> Option<Vec<Option<ShownKey>>> {
    if number == VAR_MENU {
        let vars = variables?;
        let keys = page(first, vars.len())?;
        return Some(
            keys.iter()
                .map(|k| k.map(|i| ShownKey::of_variable(&vars[i])))
                .collect(),
        );
    }
    let def = defs.get(number)?;
    if def.keys.is_empty() {
        return None;
    }
    let keys = page(first, def.keys.len())?;
    Some(
        keys.iter()
            .map(|k| k.and_then(|i| ShownKey::of_key(&def.keys[i])))
            .collect(),
    )
}

/// What identifies the menu shown and what its keys depend on, read
/// cheaply: the menu pointer, the page, the current directory, the end
/// of HOME, the data stack's top and the return stack's top. When none
/// of these moved, [`read`] gives the same keys.
pub fn signature(machine: &Machine) -> Result<[u32; 6]> {
    let u = UserMemory::of(machine)?;
    let l = u.layout();
    let screen = crate::screen::ScreenLayout::of(machine.model())
        .ok_or(Error::NoCommandLine(machine.model()))?;
    let p = |a: u32| -> Result<u32> {
        (0..5).rev().try_fold(0u32, |v, i| {
            let n =
                crate::object::Memory::nibble(machine, a + i).ok_or(Error::Unreadable(a + i))?;
            Ok((v << 4) | u32::from(n))
        })
    };
    Ok([
        p(l.menu_ptr)?,
        p(l.menu_page)?,
        p(l.context_ptr)?,
        p(l.home_end_ptr)?,
        p(l.stack_ptr)?,
        p(screen.return_top_ptr)?,
    ])
}

/// The six menu keys the screen shows now, read from RAM while the ROM
/// waits for a key (48SX, 48GX, 49G); `None` where they are not known
/// (another screen has the keyboard, a menu built in RAM, CST). `defs`
/// are the ROM's menus ([`crate::menus::read`]).
pub fn read(machine: &Machine, defs: &Menus) -> Result<Option<Vec<Option<ShownKey>>>> {
    if !crate::screen::screen(machine)?.stack_in_control {
        return Ok(None);
    }
    let u = UserMemory::of(machine)?;
    let Some(number) = defs.number(u.menu_pointer()?, machine) else {
        return Ok(None);
    };
    let first = u.menu_first_key()?;
    let vars = if number == VAR_MENU {
        Some(u.current_variables()?)
    } else {
        None
    };
    Ok(shown(defs, number, first, vars.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, kind: &str) -> Variable {
        Variable {
            name: name.into(),
            kind: kind.into(),
            size: 0.0,
            checksum: 0,
            address: 0,
            variables: None,
        }
    }

    #[test]
    fn pages_of_six() {
        assert_eq!(
            page(0, 3),
            Some([Some(0), Some(1), Some(2), None, None, None])
        );
        assert_eq!(page(6, 8), Some([Some(6), Some(7), None, None, None, None]));
        assert_eq!(page(6, 6), None, "no second page");
        assert_eq!(page(3, 12), None, "not a page start");
        assert_eq!(page(0, 0), Some([None; PAGE]), "an empty menu");
    }

    #[test]
    fn the_var_menu_names_variables_and_their_types() {
        let defs = Menus::default();
        let vars: Vec<Variable> = (0..8)
            .map(|i| {
                var(
                    &format!("V{i}"),
                    if i == 7 { "Directory" } else { "Real Number" },
                )
            })
            .collect();
        let keys = shown(&defs, VAR_MENU, 6, Some(&vars)).unwrap();
        assert_eq!(keys.len(), PAGE);
        assert_eq!(
            keys[0],
            Some(ShownKey {
                label: "V6".into(),
                kind: Some("Real Number".into()),
                ..Default::default()
            })
        );
        assert_eq!(keys[1].as_ref().unwrap().kind.as_deref(), Some("Directory"));
        assert!(keys[2..].iter().all(Option::is_none));
        assert_eq!(shown(&defs, VAR_MENU, 0, None), None, "no variables read");
        assert_eq!(shown(&defs, 3, 0, None), None, "no definition");
    }

    #[test]
    fn serialized_as_the_page_reads_it() {
        let k = ShownKey {
            label: "VECTR".into(),
            submenu: true,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&k).unwrap(),
            r#"{"label":"VECTR","submenu":true}"#
        );
        let c = ShownKey {
            label: "EXPAN".into(),
            command: Some("EXPAN".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"label":"EXPAN","command":"EXPAN"}"#
        );
    }
}
