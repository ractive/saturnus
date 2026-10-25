//! The menu keys the screen shows, read from RAM without pressing a key
//! (`Emulator::menu_keys`, the `menu` event), on the real ROMs (skipped
//! unless `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`,
//! and `rom.49g` for the 49G's ROM 1.19-6): MTH, a second page after NXT,
//! the VAR menu of a directory with variables, a choose box (no keys
//! known), and the event sent once per change (wiki:
//! hardware/hp48-system-ram, "Menu").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::Instant;

use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::transfer::Op;
use saturnus_host::typing::{Job, Verb};
use saturnus_objects::softkeys::ShownKey;

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

fn press(m: &mut Machine, k: Key) {
    m.key_down(k).unwrap();
    m.run_cycles(30 * per_ms(m)).unwrap();
    m.key_up(k).unwrap();
    let cap = m.cycles() + 5000 * per_ms(m);
    while !m.is_shutdown() && m.cycles() < cap {
        m.run_cycles(per_ms(m) / 4).unwrap();
    }
    m.run_cycles(100 * per_ms(m)).unwrap();
    settle(m, 10_000);
}

fn set_flag(e: &mut Emulator, flag: i32, on: bool) {
    e.start_transfer(Op::SetFlag { flag, on }).unwrap();
    while !e.transfer_step(20.0).unwrap() {}
    e.transfer_result().unwrap();
}

/// A cold-booted calculator at the stack; the 49G in RPN mode with soft
/// menus (flags -95 clear, -117 set) instead of choose boxes.
fn boot(model: Model, file: &str) -> Option<Emulator> {
    let mut m = Machine::new(model, &rom(file)?).unwrap();
    settle(&mut m, 60_000);
    let n = if model == Model::Hp49g { 2 } else { 1 };
    for _ in 0..n {
        press(&mut m, Key::F);
    }
    let mut e = Emulator::from_machine(m);
    if model == Model::Hp49g {
        set_flag(&mut e, -95, false);
        set_flag(&mut e, -117, true);
    }
    Some(e)
}

/// `r:TEXT` types TEXT and ENTER; anything else is key names.
fn act(e: &mut Emulator, steps: &[&str]) {
    let m = e.machine_mut();
    for step in steps {
        if let Some(text) = step.strip_prefix("r:") {
            let mut job = Job::new(m, Verb::Run, text).unwrap();
            while !job.step(m, 50 * per_ms(m)).unwrap() {}
            settle(m, 10_000);
        } else {
            for k in step.split_whitespace() {
                press(m, Key::from_name(k).unwrap_or_else(|| panic!("no key {k}")));
            }
        }
    }
}

/// The six labels, `-` for a blank key; `None` when not known.
fn labels(e: &Emulator) -> Option<Vec<String>> {
    let keys = e.menu_keys().unwrap()?;
    assert_eq!(keys.len(), 6);
    Some(
        keys.iter()
            .map(|k| k.as_ref().map_or("-".to_string(), |k| k.label.clone()))
            .collect(),
    )
}

fn key(e: &Emulator, i: usize) -> ShownKey {
    e.menu_keys().unwrap().unwrap()[i].clone().unwrap()
}

struct Rom {
    model: Model,
    file: &'static str,
    /// The keys that open MTH.
    mth: &'static str,
    /// MTH's first and second page.
    mth_pages: [&'static [&'static str]; 2],
}

const ROMS: [Rom; 4] = [
    Rom {
        model: Model::Hp48sx,
        file: "sxrom-j",
        mth: "mth",
        // The 48SX's MTH has one page: NXT shows it again.
        mth_pages: [
            &["PARTS", "PROB", "HYP", "MATRX", "VECTR", "BASE"],
            &["PARTS", "PROB", "HYP", "MATRX", "VECTR", "BASE"],
        ],
    },
    Rom {
        model: Model::Hp48gx,
        file: "gxrom-r",
        mth: "mth",
        mth_pages: [
            &["VECTR", "MATRX", "LIST", "HYP", "REAL", "BASE"],
            &["PROB", "FFT", "CMPLX", "CONST", "-", "-"],
        ],
    },
    Rom {
        model: Model::Hp49g,
        file: "rom-2.10.49g",
        mth: "leftshift symb",
        mth_pages: [
            &["VECTR", "MATRX", "LIST", "HYP", "REAL", "BASE"],
            &["PROB", "FFT", "CMPLX", "CONST", "SPECIAL FUNCTIONS", "-"],
        ],
    },
    Rom {
        model: Model::Hp49g,
        file: "rom.49g",
        mth: "leftshift symb",
        mth_pages: [
            &["VECTR", "MATRX", "LIST", "HYP", "REAL", "BASE"],
            &["PROB", "FFT", "CMPLX", "CONST", "SPECIAL FUNCTIONS", "-"],
        ],
    },
];

#[test]
fn menu_keys_follow_the_screen() {
    for r in ROMS {
        let Some(mut e) = boot(r.model, r.file) else {
            eprintln!("skipped: {} not in SATURNUS_ROM_DIR", r.file);
            continue;
        };
        let what = format!("{:?} {}", r.model, r.file);
        let t0 = Instant::now();
        // The first look reads the ROM's menus.
        let _ = e.menu_keys().unwrap();
        eprintln!("{what}: menus read in {:?}", t0.elapsed());
        act(
            &mut e,
            &[
                "r:'DIR1' CRDIR DIR1 1 'A1' STO 2 'A2' STO 3 'A3' STO 4 'A4' STO { 5 } 'A5' STO 6 'A6' STO 7.5 'A7' STO 'SUBD' CRDIR",
            ],
        );

        act(&mut e, &[r.mth]);
        assert_eq!(labels(&e).unwrap(), r.mth_pages[0], "{what}: MTH");
        assert!(key(&e, 0).submenu, "{what}: MTH's keys open menus");
        act(&mut e, &["nxt"]);
        assert_eq!(labels(&e).unwrap(), r.mth_pages[1], "{what}: MTH NXT");
        act(&mut e, &["leftshift nxt"]);
        assert_eq!(labels(&e).unwrap(), r.mth_pages[0], "{what}: PREV");

        // A menu of commands, two pages: MTH's first submenu, NXT.
        act(&mut e, &[r.mth, "a"]);
        let first = labels(&e).unwrap();
        let k = key(&e, 0);
        assert_eq!(
            k.command.as_deref(),
            Some(k.label.as_str()),
            "{what}: {k:?}"
        );
        assert!(!k.submenu);
        act(&mut e, &["nxt"]);
        let second = labels(&e).unwrap();
        assert_ne!(first, second, "{what}: NXT shows the next page");
        match r.model {
            Model::Hp48sx => {
                assert_eq!(first, ["ABS", "SIGN", "CONJ", "ARG", "RE", "IM"]);
                assert_eq!(second, ["MIN", "MAX", "MOD", "%", "%CH", "%T"]);
            }
            _ => {
                assert_eq!(first, ["ABS", "DOT", "CROSS", "V→", "→V2", "→V3"], "{what}");
                assert_eq!(second[..3], ["RECT", "CYLIN", "SPHERE"], "{what}");
            }
        }

        // The VAR menu of a directory with eight variables, newest first,
        // and its second page.
        act(&mut e, &["var"]);
        assert_eq!(
            labels(&e).unwrap(),
            ["SUBD", "A7", "A6", "A5", "A4", "A3"],
            "{what}: VAR in DIR1"
        );
        assert_eq!(key(&e, 0).kind.as_deref(), Some("Directory"));
        assert_eq!(key(&e, 3).kind.as_deref(), Some("List"));
        assert_eq!(key(&e, 1).kind.as_deref(), Some("Real Number"));
        act(&mut e, &["nxt"]);
        assert_eq!(
            labels(&e).unwrap(),
            ["A2", "A1", "-", "-", "-", "-"],
            "{what}: VAR NXT"
        );

        // The event: once per change, none while nothing moved.
        let _ = e.menu_if_changed();
        assert_eq!(e.menu_if_changed(), None, "{what}: nothing moved");
        act(&mut e, &["leftshift nxt"]);
        let ev = e.menu_if_changed().expect("the page changed");
        let keys = ev.keys.unwrap();
        assert_eq!(keys[0].as_ref().unwrap().label, "SUBD");
        let json = serde_json::to_value(ev_json(&keys)).unwrap();
        assert_eq!(json["type"], "menu");
        assert_eq!(e.menu_if_changed(), None);

        // Another screen draws its own keys: none known.
        let choose = match r.model {
            Model::Hp48sx => None,
            // The 48GX's ← 7 (SOLVE) opens a choose box, the 49G's APPS.
            Model::Hp48gx => Some("rightshift 7"),
            _ => Some("apps"),
        };
        if let Some(keys) = choose {
            act(&mut e, &[keys]);
            assert_eq!(labels(&e), None, "{what}: a choose box");
            let ev = e.menu_if_changed().expect("the keys went");
            assert_eq!(ev.keys, None);
            act(&mut e, &["on"]);
        }
    }
}

fn ev_json(keys: &[Option<ShownKey>]) -> saturnus_host::host::MenuKeys {
    saturnus_host::host::MenuKeys {
        keys: Some(keys.to_vec()),
    }
}

/// The tooltip strings the page builds from these keys, for the report:
/// a few menus per model (`--nocapture`).
#[test]
fn print_some_menus() {
    for r in ROMS {
        let Some(mut e) = boot(r.model, r.file) else {
            continue;
        };
        let algebra = match r.model {
            Model::Hp48sx => "leftshift 9",
            Model::Hp48gx => "r:93 MENU",
            _ => "r:93 MENU",
        };
        for step in [r.mth, "nxt", algebra, "var"] {
            act(&mut e, &[step]);
            let keys = e.menu_keys().unwrap();
            eprintln!(
                "{:?} {} [{step}]: {}",
                r.model,
                r.file,
                serde_json::to_string(&keys).unwrap()
            );
        }
    }
}
