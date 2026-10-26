//! The menu keys the screen shows on the 42S, 27S, 17BII, 17B, 19BII and
//! 28S, read from RAM and the LCD (`Emulator::menu_keys`, the `menu`
//! event; `saturnus_host::menulabels`) on the real ROMs, skipped unless
//! `SATURNUS_ROM_DIR` holds them.
//!
//! The Pioneers walk the key reference (`keys-<model>.json`, checked on
//! the screen by `saturnus-refgen keys`): every menu key a function's
//! keys press under a menu must read as the label the reference gives
//! it. Then the menus it does not reach: the Solver's variable menus,
//! the CFLO and SUM lists, the 19BII's TEXT menus, the 42S's program and
//! variable menus; no labels while no box is drawn. The 28S: each menu's
//! pages against its table; none in the cursor menu, USER, a custom menu
//! or SOLVR's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::menulabels;
use serde::Deserialize;

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

fn per_ms(m: &Machine) -> u64 {
    u64::from(m.model().clock_hz()) / 1000
}

/// Run until the screen has stayed the same for 300 ms with the CPU
/// asleep, at most `cap_ms`.
fn settle(m: &mut Machine, cap_ms: u64) {
    let end = m.cycles() + cap_ms * per_ms(m);
    let mut last = m.lcd();
    let mut since = m.cycles();
    while m.cycles() < end {
        m.run_cycles(5 * per_ms(m)).unwrap();
        let lcd = m.lcd();
        let lit = lcd.pixels.iter().any(|r| r.iter().any(|&p| p));
        if lcd != last {
            last = lcd;
            since = m.cycles();
        } else if m.is_shutdown() && lit && m.cycles() - since > 300 * per_ms(m) {
            return;
        }
    }
}

fn press(e: &mut Emulator, name: &str) {
    let k = Key::from_name(name).unwrap_or_else(|| panic!("no key {name}"));
    let m = e.machine_mut();
    m.key_down(k).unwrap();
    m.run_cycles(30 * per_ms(m)).unwrap();
    m.key_up(k).unwrap();
    m.run_cycles(50 * per_ms(m)).unwrap();
    settle(m, 10_000);
}

fn presses(e: &mut Emulator, keys: &str) {
    for k in keys.split_whitespace() {
        press(e, k);
    }
}

/// The labels shown, a blank key as `""`; `None` when none is known.
fn labels(e: &Emulator) -> Option<Vec<String>> {
    let keys = e.menu_keys().unwrap()?;
    Some(
        keys.iter()
            .map(|k| k.as_ref().map(|k| k.label.clone()).unwrap_or_default())
            .collect(),
    )
}

/// Entry `i`'s characters as codes, for a message.
fn raw(e: &Emulator, i: usize) -> String {
    let m = e.machine();
    let Some(at) = menulabels::buffer(m.model()) else {
        return String::new();
    };
    let a = at + 11 * i as u32;
    let n = m.peek(a);
    (0..u32::from(n))
        .map(|k| {
            format!(
                "{:02X}",
                m.peek(a + 1 + 2 * k) | (m.peek(a + 2 + 2 * k) << 4)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn shown(e: &Emulator) -> String {
    labels(e).map_or("(none)".into(), |l| l.join(" "))
}

#[derive(Deserialize)]
struct Step {
    key: String,
    label: Option<String>,
}

#[derive(Deserialize)]
struct Command {
    name: String,
    #[serde(default)]
    keys: Vec<Step>,
}

#[derive(Deserialize)]
struct KeyFile {
    menu_keys: Vec<String>,
    #[serde(default)]
    ready: Vec<Step>,
    commands: Vec<Command>,
}

fn key_file(model: &str) -> KeyFile {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../saturnus-cli/data/commands")
        .join(format!("keys-{model}.json"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// The model booted, `ready` pressed (the language chosen), at its
/// normal state.
fn boot(model: Model, file: &str) -> Option<(Emulator, KeyFile)> {
    let mut m = Machine::new(model, &rom(file)?).unwrap();
    settle(&mut m, 20_000);
    let mut e = Emulator::from_machine(m);
    let kf = key_file(model.name());
    for s in &kf.ready {
        press(&mut e, &s.key);
    }
    Some((e, kf))
}

const PIONEERS: [(Model, &str); 5] = [
    (Model::Hp42s, "hp42s.rom"),
    (Model::Hp27s, "hp27s.rom"),
    (Model::Hp17bii, "hp17bii.rom"),
    (Model::Hp17b, "hp17b.rom"),
    (Model::Hp19bii, "hp19bii.rom"),
];

/// Whether the buffer's `label` is what the reference's `want` names:
/// the same once the reference's mode mark goes and its minus sign is the
/// ROM's hyphen, or with the buffer's spaces taken out (`C X,Y`), or cut
/// short by the box (`TRANS` shows as `TRAN`).
fn names(label: &str, want: &str) -> bool {
    let want = want.trim_end_matches('■').replace('−', "-");
    label == want
        || label.replace(' ', "") == want
        || (want.chars().count() >= 3 && label.len() > want.len() && label.starts_with(&want))
}

#[test]
fn the_pioneers_show_the_reference_labels() {
    for (model, file) in PIONEERS {
        let Some((mut e, kf)) = boot(model, file) else {
            continue;
        };
        let start = e.machine().save_state();
        let (mut checked, mut bad) = (0, Vec::new());
        let mut seen = std::collections::BTreeSet::new();
        for c in &kf.commands {
            if c.keys.is_empty() {
                continue;
            }
            e.machine_mut().load_state(&start).unwrap();
            let mut shifted = false;
            let mut path = Vec::new();
            for s in &c.keys {
                let pos = kf.menu_keys.iter().position(|k| *k == s.key);
                if let (Some(i), Some(label), false) = (pos, &s.label, shifted)
                    && let Some(now) = labels(&e)
                {
                    let want = label.as_str();
                    // Each (menu path, key) once.
                    if seen.insert((path.clone(), i)) {
                        checked += 1;
                        if !names(&now[i], want) {
                            bad.push(format!(
                                "{} {}: key {} shows {:?}, not {want:?} [{}] codes {}",
                                c.name,
                                path.join(" "),
                                i + 1,
                                now[i],
                                now.join("|"),
                                raw(&e, i)
                            ));
                        }
                    }
                }
                shifted = s.key == "shift";
                path.push(s.label.clone().unwrap_or_else(|| s.key.clone()));
                press(&mut e, &s.key);
            }
        }
        eprintln!("{}: {checked} menu keys checked", model.name());
        assert!(checked > 40, "{}: {checked}", model.name());
        assert!(bad.is_empty(), "{}:\n{}", model.name(), bad.join("\n"));
    }
}

/// Type `text` (capitals and `=`) through the ALPHA menu of the 27S,
/// 17BII or 17B: the group's key, then the letter's.
fn alpha_menu(e: &mut Emulator, text: &str) {
    const GROUPS: [&str; 6] = ["ABCDE", "FGHI", "JKLM", "NOPQ", "RSTUV", "WXYZ"];
    const KEYS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];
    for c in text.chars() {
        if c == '=' {
            press(e, "equals");
            continue;
        }
        let g = GROUPS.iter().position(|g| g.contains(c)).unwrap();
        let i = GROUPS[g].find(c).unwrap();
        press(e, KEYS[g]);
        press(e, KEYS[i]);
    }
}

/// Type `text` on the 19BII's letter keys.
fn letters(e: &mut Emulator, text: &str) {
    for c in text.chars() {
        match c {
            '=' => press(e, "equals"),
            _ => press(e, &format!("letter{}", c.to_ascii_lowercase())),
        }
    }
}

fn first_two(e: &Emulator) -> Vec<String> {
    labels(e).map(|l| l[..2].to_vec()).unwrap_or_default()
}

#[test]
fn the_pioneers_name_what_the_user_named() {
    for (model, file) in PIONEERS {
        if model == Model::Hp42s {
            continue;
        }
        let Some((mut e, _)) = boot(model, file) else {
            continue;
        };
        let start = e.machine().save_state();
        let typed = |e: &mut Emulator, text: &str| {
            if model == Model::Hp19bii {
                letters(e, text);
            } else {
                alpha_menu(e, text);
            }
        };
        // The Solver's variable menu: SOLVE NEW, A=B, INPUT, CALC.
        let solve = if model == Model::Hp27s {
            "shift 7"
        } else {
            "e"
        };
        presses(&mut e, &format!("{solve} d"));
        typed(&mut e, "A=B");
        presses(&mut e, "input a");
        assert_eq!(first_two(&e), ["A", "B"], "{} SOLVE CALC", model.name());
        let keys = e.menu_keys().unwrap().unwrap();
        assert!(
            keys[2..].iter().all(Option::is_none),
            "{}: blank keys",
            model.name()
        );
        eprintln!("{} SOLVE CALC: {}", model.name(), shown(&e));
        // The lists' GET menus with a list named AB.
        let lists: &[(&str, &str)] = match model {
            Model::Hp27s => &[("STAT", "shift 8")],
            Model::Hp19bii => &[("CFLO", "a c"), ("SUM", "c"), ("TEXT", "f")],
            _ => &[("CFLO", "a c"), ("SUM", "c")],
        };
        for (what, keys) in lists {
            e.machine_mut().load_state(&start).unwrap();
            presses(&mut e, keys);
            eprintln!("{} {what}: {}", model.name(), shown(&e));
            press(&mut e, "d"); // NAME
            typed(&mut e, "AB");
            presses(&mut e, "input e"); // GET
            assert_eq!(first_two(&e), ["*NEW", "AB"], "{} {what} GET", model.name());
        }
    }
}

#[test]
fn no_labels_without_their_boxes() {
    // The 27S after EXIT: no menu, the buffer still holds old labels.
    if let Some((mut e, _)) = boot(Model::Hp27s, "hp27s.rom") {
        assert_eq!(e.menu_keys().unwrap(), None, "the 27S's normal state");
        presses(&mut e, "shift 9");
        assert_eq!(labels(&e).unwrap()[0], "N", "TVM");
        press(&mut e, "exit");
        assert_eq!(e.menu_keys().unwrap(), None, "TVM left");
    }
    if let Some((mut e, _)) = boot(Model::Hp42s, "hp42s.rom") {
        assert_eq!(e.menu_keys().unwrap(), None, "the 42S's stack");
        presses(&mut e, "shift 4");
        let l = labels(&e).unwrap();
        // The buffer holds A...F for the box's A…F, and DECM where the box
        // shows DEC■.
        assert_eq!(
            l,
            ["A...F", "HEXM", "DECM", "OCTM", "BINM", "LOGIC"],
            "BASE"
        );
        press(&mut e, "on");
        assert_eq!(e.menu_keys().unwrap(), None, "BASE left");
    }
}

#[test]
fn the_menu_event_comes_once_per_change() {
    let Some((mut e, _)) = boot(Model::Hp17bii, "hp17bii.rom") else {
        return;
    };
    let ev = e.menu_if_changed().expect("MAIN");
    let first: Vec<String> = ev
        .keys
        .unwrap()
        .iter()
        .flatten()
        .map(|k| k.label.clone())
        .collect();
    assert_eq!(first, ["FIN", "BUS", "SUM", "TIME", "SOLVE"]);
    assert_eq!(e.menu_if_changed(), None, "nothing moved");
    press(&mut e, "a");
    let ev = e.menu_if_changed().expect("FIN");
    assert_eq!(ev.keys.unwrap()[0].as_ref().unwrap().label, "TVM");
    assert_eq!(e.menu_if_changed(), None);
    e.reshow();
    assert!(e.menu_if_changed().is_some(), "sent again after reshow");
}

/// Type `text` (capitals) through the 42S's ALPHA menu: shift ENTER
/// starts it, then each letter's group and letter keys; ENTER ends it.
fn alpha42(e: &mut Emulator, text: &str) {
    const GROUPS: [&str; 6] = ["ABCDE", "FGHI", "JKLM", "NOPQ", "RSTUV", "WXYZ"];
    const KEYS: [&str; 6] = ["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"];
    presses(e, "shift enter");
    for c in text.chars() {
        let g = GROUPS.iter().position(|g| g.contains(c)).unwrap();
        let i = GROUPS[g].find(c).unwrap();
        press(e, KEYS[g]);
        press(e, KEYS[i]);
    }
    press(e, "enter");
}

#[test]
fn the_42s_names_programs_and_variables() {
    let Some((mut e, _)) = boot(Model::Hp42s, "hp42s.rom") else {
        return;
    };
    // A program: LBL "S", MVAR "X", MVAR "Y", typed in program entry.
    press(&mut e, "shift");
    press(&mut e, "rs"); // PRGM
    presses(&mut e, "shift 3 sigmaplus"); // PGM.FCN LBL
    alpha42(&mut e, "S");
    for v in ["X", "Y"] {
        presses(&mut e, "shift 3 down down down sigmaplus"); // MVAR
        alpha42(&mut e, v);
    }
    presses(&mut e, "on on");
    assert_eq!(e.menu_keys().unwrap(), None, "program entry left");
    // The XEQ menu: the program's label.
    press(&mut e, "xeq");
    assert_eq!(labels(&e).unwrap()[0], "S", "XEQ: {}", shown(&e));
    eprintln!("42s XEQ: {}", shown(&e));
    presses(&mut e, "on on");
    // The Solver: its program menu, then the variables from MVAR.
    presses(&mut e, "shift 7");
    eprintln!("42s SOLVER: {}", shown(&e));
    assert_eq!(labels(&e).unwrap()[0], "S");
    press(&mut e, "sigmaplus");
    eprintln!("42s SOLVER S: {}", shown(&e));
    assert_eq!(first_two(&e), ["X", "Y"]);
    presses(&mut e, "on on on");
    // VARMENU "S" from the keyboard.
    presses(&mut e, "shift 3 down down down inv"); // VARM
    alpha42(&mut e, "S");
    eprintln!("42s VARMENU S: {}", shown(&e));
    assert_eq!(first_two(&e), ["X", "Y"]);
    presses(&mut e, "on on on");
    // The RCL prompt's variable menu, with a variable A stored.
    presses(&mut e, "5 sto");
    alpha42(&mut e, "A");
    press(&mut e, "rcl");
    eprintln!("42s RCL: {}", shown(&e));
    assert!(
        labels(&e).unwrap().contains(&"A".to_string()),
        "RCL: {}",
        shown(&e)
    );
}

#[derive(Deserialize)]
struct Menu28 {
    name: String,
    keys: String,
    labels: Vec<Option<Key28>>,
}

#[derive(Deserialize)]
struct Key28 {
    label: String,
}

#[derive(Deserialize)]
struct Table28 {
    menus: std::collections::BTreeMap<u32, Menu28>,
}

#[test]
fn the_28s_shows_its_menus_page_by_page() {
    let Some(rom) = rom("hp28s.rom") else {
        return;
    };
    let mut m = Machine::new(Model::Hp28s, &rom).unwrap();
    settle(&mut m, 20_000);
    let mut e = Emulator::from_machine(m);
    press(&mut e, "on"); // ATTN: "Memory Lost" goes
    assert_eq!(e.menu_keys().unwrap(), None, "no menu after the boot");
    let start = e.machine().save_state();
    let table: Table28 =
        serde_json::from_str(include_str!("../src/menulabels/menus-28s.json")).unwrap();
    assert_eq!(table.menus.len(), 20);
    for (n, menu) in &table.menus {
        e.machine_mut().load_state(&start).unwrap();
        presses(&mut e, &menu.keys);
        let pages = menu.labels.len().div_ceil(6);
        for p in 0..pages {
            let want: Vec<String> = (0..6)
                .map(|i| {
                    menu.labels
                        .get(p * 6 + i)
                        .and_then(|k| k.as_ref())
                        .map(|k| k.label.clone())
                        .unwrap_or_default()
                })
                .collect();
            assert_eq!(
                labels(&e),
                Some(want),
                "{} (menu {n}) page {}",
                menu.name,
                p + 1
            );
            press(&mut e, "nxt");
        }
        // NEXT wraps to the first page.
        assert_eq!(
            labels(&e).unwrap()[0],
            menu.labels[0].as_ref().unwrap().label,
            "{} wraps",
            menu.name
        );
    }
    // STACK, as the Reference Manual lists it; ATTN leaves it shown.
    e.machine_mut().load_state(&start).unwrap();
    presses(&mut e, "shift letterg");
    assert_eq!(shown(&e), "DUP OVER DUP2 DROP2 ROT LIST→");
    let keys = e.menu_keys().unwrap().unwrap();
    assert_eq!(keys[5].as_ref().unwrap().command.as_deref(), Some("LIST→"));
    press(&mut e, "on");
    assert_eq!(shown(&e), "DUP OVER DUP2 DROP2 ROT LIST→", "after ATTN");
    // The cursor menu and the USER menu are not in the table.
    press(&mut e, "cursor");
    assert_eq!(e.menu_keys().unwrap(), None, "the cursor menu");
    // Menus not in the table: USER with a variable (23), a custom menu
    // (`{ A B } MENU`, 254), the Solver's variables (SOLVR, 24).
    e.machine_mut().load_state(&start).unwrap();
    presses(&mut e, "1 enter alpha quote letterz quote enter sto user");
    assert_eq!(e.menu_keys().unwrap(), None, "USER");
    presses(
        &mut e,
        "alpha lbrace lettera space letterb shift lbrace space letterm lettere lettern letteru enter",
    );
    assert_eq!(e.menu_keys().unwrap(), None, "a custom menu");
    presses(
        &mut e,
        "alpha quote lettera equals letterb quote space letters lettert lettere letterq enter",
    );
    presses(&mut e, "solv");
    assert_eq!(shown(&e), "STEQ RCEQ SOLVR ISOL QUAD SHOW");
    press(&mut e, "c");
    assert_eq!(e.menu_keys().unwrap(), None, "SOLVR");
}
