//! The typing tables, generated and checked on the ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`).
//!
//! `SATURNUS_TYPING_REGEN=1 cargo test --release -p saturnus-host --test
//! typing -- regenerate` surveys every key, plain and shifted, in alpha
//! mode and in program entry mode, and every accent key after every
//! letter, in all entry modes, reads what each inserted from RAM, and
//! writes `src/typing/<model>.tsv`. Without the variable `regenerate`
//! (with `SATURNUS_TYPING_SURVEY=1`) checks that a fresh survey gives the
//! committed file. The round trip types every character of each model's
//! set through the engine and reads the line back, starting in immediate
//! entry, and with `SATURNUS_TYPING_FULL=1` also after `'` and `«` (and in
//! the 49G's RPN mode); a release build runs all of it in a minute.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use saturnus::cpu::Bus as _;
use saturnus::io::Key;
use saturnus::{Machine, Model};
use saturnus_host::typing::{self, GAP_MS, HOLD_MS, Job, Method, Verb};
use saturnus_objects::charset;
use saturnus_objects::cmdline::{self, Editor};

const MODELS: [(Model, &str); 3] = [
    (Model::Hp48sx, "sxrom-j"),
    (Model::Hp48gx, "gxrom-r"),
    (Model::Hp49g, "rom-2.10.49g"),
];

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

fn per_ms(m: &Machine) -> u64 {
    u64::from(m.model().clock_hz()) / 1000
}

fn run_ms(m: &mut Machine, ms: u64) {
    let n = ms * per_ms(m);
    m.run_cycles(n).unwrap();
}

/// Press `k` as the engine does: hold, release, wait for sleep, gap.
fn press(m: &mut Machine, k: Key) {
    m.key_down(k).unwrap();
    run_ms(m, HOLD_MS);
    m.key_up(k).unwrap();
    let cap = m.cycles() + 5000 * per_ms(m);
    while !m.is_shutdown() && m.cycles() < cap {
        m.run_cycles(per_ms(m) / 4).unwrap();
    }
    run_ms(m, GAP_MS * 5);
}

fn keys(m: &mut Machine, ks: &[Key]) {
    for &k in ks {
        press(m, k);
    }
}

/// Run until the screen has stayed the same for 300 ms with the CPU
/// asleep, at most `cap_ms`.
fn settle(m: &mut Machine, cap_ms: u64) {
    let end = m.cycles() + cap_ms * per_ms(m);
    let mut last = m.lcd();
    let mut since = m.cycles();
    while m.cycles() < end {
        run_ms(m, 5);
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

/// A cold-booted machine at the stack (memory cleared).
fn boot(model: Model, file: &str) -> Option<Machine> {
    let mut m = Machine::new(model, &rom(file)?).unwrap();
    settle(&mut m, 60_000);
    let n = if model == Model::Hp49g { 2 } else { 1 };
    for _ in 0..n {
        press(&mut m, Key::F);
        settle(&mut m, 10_000);
    }
    Some(m)
}

fn editor(m: &Machine) -> Editor {
    cmdline::editor(m).unwrap()
}

/// Type `text` with the engine.
fn send(m: &mut Machine, verb: Verb, text: &str) -> Result<typing::Outcome, String> {
    let mut job = Job::new(m, verb, text)?;
    while !job.step(m, 50 * per_ms(m))? {}
    Ok(job.outcome().clone())
}

/// The 49G's algebraic operating mode is flag -95 (system word 2, bit 30;
/// test set-up only, the engine never writes RAM).
fn set_rpn(m: &mut Machine) {
    let at = 0x80F19;
    let v = m.peek(at) & !4;
    m.hw.write_nibble(at, v);
}

fn quote(model: Model) -> Vec<Key> {
    if model == Model::Hp49g {
        vec![Key::RightShift, Key::Eqw]
    } else {
        vec![Key::Quote]
    }
}

fn program(model: Model) -> Vec<Key> {
    if model == Model::Hp49g {
        vec![Key::RightShift, Key::Plus]
    } else {
        vec![Key::LeftShift, Key::Minus]
    }
}

/// What `ks` inserted relative to `base`: (before the cursor, after it).
fn inserted(base: &Editor, e: &Editor) -> Option<(Vec<u8>, Vec<u8>)> {
    let cur = base.cursor;
    let after = base.text.len() - cur;
    let ok = e.active
        && e.text.len() >= base.text.len()
        && e.text[..cur] == base.text[..cur]
        && e.text[e.text.len() - after..] == base.text[cur..]
        && e.cursor >= cur
        && e.cursor <= e.text.len() - after;
    ok.then(|| {
        (
            e.text[cur..e.cursor].to_vec(),
            e.text[e.cursor..e.text.len() - after].to_vec(),
        )
    })
}

/// Start states for a survey: a line holding "1" in immediate entry, one
/// after `'`, one after `«` (both shifts of the 49G's operating mode).
fn starts(model: Model, m: &mut Machine) -> Vec<(String, Vec<u8>)> {
    let fresh = m.save_state();
    let mut out = Vec::new();
    let variants: &[bool] = if model == Model::Hp49g {
        &[false, true]
    } else {
        &[false]
    };
    for &rpn in variants {
        for (name, ks) in [
            ("immediate", vec![Key::One]),
            ("algebraic", quote(model)),
            ("program", program(model)),
        ] {
            m.load_state(&fresh).unwrap();
            if rpn {
                set_rpn(m);
            }
            keys(m, &ks);
            let tag = format!("{name}{}", if rpn { " rpn" } else { "" });
            out.push((tag, m.save_state()));
        }
    }
    m.load_state(&fresh).unwrap();
    out
}

fn surveyed_keys(model: Model) -> Vec<Key> {
    let layout = saturnus_host::layout::layout(model);
    layout
        .iter()
        .filter_map(|k| Key::from_name(k.name))
        .filter(|k| !matches!(k, Key::On | Key::Alpha | Key::LeftShift | Key::RightShift))
        .collect()
}

const ACCENTS: [[Key; 2]; 6] = [
    [Key::LeftShift, Key::Seven],
    [Key::LeftShift, Key::Eight],
    [Key::LeftShift, Key::Nine],
    [Key::RightShift, Key::Seven],
    [Key::RightShift, Key::Eight],
    [Key::RightShift, Key::Nine],
];

fn names(ks: &[Key]) -> String {
    ks.iter().map(|k| k.name()).collect::<Vec<_>>().join(" ")
}

/// Survey `model` and give its table file.
fn survey(model: Model, m: &mut Machine) -> String {
    let starts = starts(model, m);
    let keys_all = surveyed_keys(model);
    let prefixes: [&[Key]; 3] = [&[], &[Key::LeftShift], &[Key::RightShift]];
    // (keys, what they inserted) per start state, in alpha mode.
    let try_in = |m: &mut Machine, state: &[u8], setup: &[Key], ks: &[Key]| {
        m.load_state(state).unwrap();
        keys(m, setup);
        let base = editor(m);
        keys(m, ks);
        inserted(&base, &editor(m))
    };
    let lock = [Key::Alpha, Key::Alpha];
    let mut methods: BTreeMap<u8, String> = BTreeMap::new();
    // Alpha and pair keys: the same insert in every start state.
    for pre in prefixes {
        for &k in &keys_all {
            let mut ks = pre.to_vec();
            ks.push(k);
            let mut seen = None;
            let mut same = true;
            for (_, state) in &starts {
                let got = try_in(m, state, &lock, &ks);
                match (&seen, got) {
                    (_, None) => same = false,
                    (None, Some(g)) => seen = Some(g),
                    (Some(s), Some(g)) => same &= *s == g,
                }
                if !same {
                    break;
                }
            }
            let Some((before, behind)) = seen.filter(|_| same) else {
                continue;
            };
            match (before.as_slice(), behind.as_slice()) {
                ([c], []) => {
                    methods
                        .entry(*c)
                        .or_insert_with(|| format!("alpha\t{}", names(&ks)));
                }
                ([o], [c]) => {
                    for x in [*o, *c] {
                        methods
                            .entry(x)
                            .or_insert_with(|| format!("pair:{o},{c}\t{}", names(&ks)));
                    }
                }
                _ => {}
            }
        }
    }
    // Program entry keys (alpha off): the first character other than a
    // space, the same in program and in algebraic/program entry.
    let entry = [Key::RightShift, Key::Alpha];
    for pre in prefixes {
        for &k in &keys_all {
            let mut ks = pre.to_vec();
            ks.push(k);
            let mut first = None;
            let mut same = true;
            for (name, state) in &starts {
                let setup: Vec<Key> = match name.split(' ').next() {
                    Some("immediate") => entry.to_vec(),
                    Some("program") => quote(model),
                    _ => continue,
                };
                // The key may put a space before the character too.
                let got = try_in(m, state, &setup, &ks)
                    .and_then(|(b, _)| b.iter().copied().find(|&c| c != b' '));
                match (first, got) {
                    (_, None) => same = false,
                    (None, Some(c)) => first = Some(c),
                    (Some(a), Some(c)) => same &= a == c,
                }
                if !same {
                    break;
                }
            }
            if let Some(c) = first.filter(|_| same) {
                methods
                    .entry(c)
                    .or_insert_with(|| format!("entry\t{}", names(&ks)));
            }
        }
    }
    // Accents after each alpha character.
    let alpha: Vec<(u8, Vec<Key>)> = methods
        .iter()
        .filter_map(|(&c, m)| {
            let ks = m.strip_prefix("alpha\t")?;
            Some((
                c,
                ks.split(' ').map(|n| Key::from_name(n).unwrap()).collect(),
            ))
        })
        .collect();
    let (_, plain) = &starts[0];
    for (base, bks) in &alpha {
        for acc in ACCENTS {
            m.load_state(plain).unwrap();
            keys(m, &lock);
            keys(m, bks);
            let before = editor(m);
            keys(m, &acc);
            let e = editor(m);
            let ok = e.active
                && e.cursor >= 1
                && e.text.len() == before.text.len()
                && e.cursor == before.cursor
                && e.text[..e.cursor - 1] == before.text[..e.cursor - 1];
            if !ok {
                continue;
            }
            let c = e.text[e.cursor - 1];
            if c != *base {
                methods
                    .entry(c)
                    .or_insert_with(|| format!("accent:{base}\t{}", names(&acc)));
            }
        }
    }
    let mut out = format!(
        "# How the {} types each character into its command line: code, method, keys.\n\
         # Generated by crates/saturnus-host/tests/typing.rs from the ROM; do not edit.\n\
         # alpha: the keys in alpha mode; pair:O,C: they insert O and C around the cursor;\n\
         # entry: the keys in program entry mode, alpha off (the first non-space they insert);\n\
         # accent:B: character B, then the keys; chars: the CHARS application; none: no way.\n",
        model.name().to_uppercase()
    );
    for c in 1..=255u8 {
        let method = methods.get(&c).cloned().unwrap_or_else(|| {
            if model == Model::Hp48sx {
                "none".to_string()
            } else {
                "chars".to_string()
            }
        });
        out.push_str(&format!("{c}\t{method}\n"));
    }
    out.push_str("0\tnone\n");
    out
}

fn table_path(model: Model) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/typing")
        .join(format!("{}.tsv", model.name()))
}

#[test]
fn regenerate() {
    let regen = std::env::var_os("SATURNUS_TYPING_REGEN").is_some();
    if !regen && std::env::var_os("SATURNUS_TYPING_SURVEY").is_none() {
        // The survey takes minutes in a debug build; the round trip below
        // checks every entry of the committed tables.
        eprintln!("skipped: set SATURNUS_TYPING_REGEN (write) or SATURNUS_TYPING_SURVEY (compare)");
        return;
    }
    for (model, file) in MODELS {
        let Some(mut m) = boot(model, file) else {
            eprintln!("skipped {}: no ROM", model.name());
            continue;
        };
        let table = survey(model, &mut m);
        if regen {
            std::fs::write(table_path(model), &table).unwrap();
        } else {
            let committed = std::fs::read_to_string(table_path(model)).unwrap();
            assert_eq!(
                committed,
                table,
                "{} table differs from the ROM",
                model.name()
            );
        }
    }
}

/// Every character a model can type, in code order.
fn typable(model: Model) -> Vec<u8> {
    let t = typing::table(model).unwrap();
    (1..=255u8)
        .filter(|&c| t[usize::from(c)] != Method::None)
        .collect()
}

#[test]
fn every_character_round_trips() {
    for (model, file) in MODELS {
        let Some(mut m) = boot(model, file) else {
            eprintln!("skipped {}: no ROM", model.name());
            continue;
        };
        let set = typable(model);
        let text = charset::decode(&set);
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for c in 0..=255u8 {
            let kind = match &typing::table(model).unwrap()[usize::from(c)] {
                Method::None => "none",
                Method::Alpha(_) => "alpha",
                Method::Pair { .. } => "pair",
                Method::Entry(_) => "entry",
                Method::Accent { .. } => "accent",
                Method::Chars => "chars",
            };
            *counts.entry(kind).or_default() += 1;
        }
        eprintln!("{}: {counts:?}", model.name());
        // Every start state takes about a minute of a debug build on the
        // 48GX (its CHARS characters); by default only immediate entry.
        let full = std::env::var_os("SATURNUS_TYPING_FULL").is_some();
        for (name, state) in starts(model, &mut m) {
            if !full && name != "immediate" {
                continue;
            }
            m.load_state(&state).unwrap();
            let before = editor(&m);
            let t = std::time::Instant::now();
            let out = send(&mut m, Verb::Insert, &text)
                .unwrap_or_else(|e| panic!("{} from {name}: {e}", model.name()));
            let e = editor(&m);
            let mut want = before.text.clone();
            want.splice(before.cursor..before.cursor, set.iter().copied());
            assert_eq!(
                charset::decode(&e.text),
                charset::decode(&want),
                "{} from {name}",
                model.name()
            );
            assert_eq!(e.cursor, before.cursor + set.len());
            eprintln!(
                "{} from {name}: {} characters, {} keys, {:.0} ms emulated, {:?} wall",
                model.name(),
                out.typed,
                out.presses,
                out.emulated_ms,
                t.elapsed()
            );
        }
    }
}

fn stack(m: &Machine) -> Vec<String> {
    let names = saturnus_objects::NameTable::of(m);
    let mem = saturnus_objects::UserMemory::of(m)
        .unwrap()
        .with_names(&names);
    let st = saturnus_objects::Settings::standard(m.model());
    mem.stack()
        .unwrap()
        .iter()
        .map(|o| saturnus_objects::display(o, &st))
        .collect()
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Each model, freshly booted, in RPN (the 49G boots algebraic).
fn rpn_machines() -> Vec<Machine> {
    MODELS
        .into_iter()
        .filter_map(|(model, file)| {
            let mut m = boot(model, file)?;
            if model == Model::Hp49g {
                set_rpn(&mut m);
            }
            Some(m)
        })
        .collect()
}

#[test]
fn insert_starts_a_line_and_leaves_the_keyboard_as_it_was() {
    for mut m in rpn_machines() {
        let model = m.model();
        // Alpha and lowercase locked by the user, no line open.
        keys(
            &mut m,
            &[Key::Alpha, Key::Alpha, Key::LeftShift, Key::Alpha],
        );
        let e = editor(&m);
        assert!(e.alpha_lock && e.lowercase && !e.active, "{}", model.name());
        send(&mut m, Verb::Insert, "Ab1(x)").unwrap();
        let e = editor(&m);
        assert_eq!(charset::decode(&e.text), "Ab1(x)", "{}", model.name());
        assert_eq!(e.cursor, 6);
        assert!(
            e.alpha_lock && e.lowercase,
            "{} locks restored",
            model.name()
        );
        // A pending left shift survives an insert.
        keys(&mut m, &[Key::LeftShift]);
        let before = editor(&m);
        assert!(before.left_shift, "{}", model.name());
        send(&mut m, Verb::Insert, " 2").unwrap();
        let e = editor(&m);
        assert_eq!(charset::decode(&e.text), "Ab1(x) 2");
        let keyboard = |e: &Editor| {
            (
                e.alpha,
                e.alpha_lock,
                e.lowercase,
                e.left_shift,
                e.right_shift,
            )
        };
        assert_eq!(keyboard(&e), keyboard(&before), "{}", model.name());
    }
}

#[test]
fn run_evaluates_and_reports_the_calculators_error() {
    for mut m in rpn_machines() {
        let model = m.model().name();
        let out = send(&mut m, Verb::Run, "« 1 2 + » EVAL").unwrap();
        assert_eq!(out.closed, Some(true), "{model}");
        assert_eq!(out.error, None, "{model}");
        assert_eq!(stack(&m), ["3"], "{model}");
        let out = send(&mut m, Verb::Run, "'1+").unwrap();
        assert_eq!(out.closed, Some(false), "{model}");
        assert_eq!(out.error.as_deref(), Some("Invalid Syntax"), "{model}");
        let e = editor(&m);
        assert!(e.active, "{model}: the line stays open");
        assert_eq!(charset::decode(&e.text), "'1+", "{model}");
        // A run-time error closes the line and shows its message.
        send(&mut m, Verb::Replace, "").unwrap();
        let out = send(&mut m, Verb::Run, "DROP DROP").unwrap();
        assert_eq!(out.closed, Some(true), "{model}");
        assert_eq!(
            out.error.as_deref(),
            Some("DROP Error: Too Few Arguments"),
            "{model}"
        );
    }
}

#[test]
fn replace_stays_in_an_edit_session() {
    for mut m in rpn_machines() {
        let model = m.model();
        send(&mut m, Verb::Run, "7 123").unwrap();
        // EDIT level 1: left shift +/- on the 48, the down arrow on the 49G.
        let edit = if model == Model::Hp49g {
            vec![Key::Down]
        } else {
            vec![Key::LeftShift, Key::Neg]
        };
        keys(&mut m, &edit);
        assert_eq!(charset::decode(&editor(&m).text), "123", "{}", model.name());
        send(&mut m, Verb::Replace, "« 4 5 * »").unwrap();
        let out = send(&mut m, Verb::Run, "").unwrap();
        assert_eq!(out.closed, Some(true));
        assert_eq!(stack(&m), ["« 4 5 * »", "7"], "{}", model.name());
    }
}

#[test]
fn a_program_reads_back_as_typed() {
    let program = "« → n\n  « 1 1 n\n    FOR i i * NEXT\n    IF DUP 100 > THEN \"big\" ELSE \"small\" END\n  »\n»";
    for mut m in rpn_machines() {
        let model = m.model().name();
        let out = send(&mut m, Verb::Run, &format!("{program} 'P1' STO")).unwrap();
        assert_eq!(out.error, None, "{model}");
        let names = saturnus_objects::NameTable::of(&m);
        let mem = saturnus_objects::UserMemory::of(&m)
            .unwrap()
            .with_names(&names);
        let st = saturnus_objects::Settings::standard(m.model());
        let var = mem
            .tree()
            .unwrap()
            .into_iter()
            .find(|v| v.name == "P1")
            .expect("P1 stored");
        let obj = mem.object_at(var.address).unwrap();
        assert_eq!(
            squash(&saturnus_objects::display(&obj, &st)),
            squash(program),
            "{model}"
        );
        send(&mut m, Verb::Run, "5 P1").unwrap();
        assert_eq!(stack(&m), ["\"big\"", "120"], "{model}");
    }
}

/// Typing speed per model (`SATURNUS_TYPING_SPEED=1`, best in a release
/// build): characters per second of emulated and of wall time, for plain
/// text and for a program full of shifted characters.
#[test]
fn speed() {
    if std::env::var_os("SATURNUS_TYPING_SPEED").is_none() {
        return;
    }
    let plain = "THE QUICK BROWN FOX 1234 JUMPS OVER 5678 LAZY DOGS 90 ".repeat(4);
    let program = "« → a b « IF a b ≤ THEN { a b } ELSE [ 1 2 ] END 'a+b' \"x\" » »\n".repeat(3);
    for mut m in rpn_machines() {
        for (name, text) in [("plain", &plain), ("program", &program)] {
            let state = m.save_state();
            let t = std::time::Instant::now();
            let out = send(&mut m, Verb::Insert, text).unwrap();
            let wall = t.elapsed().as_secs_f64();
            let n = out.typed as f64;
            eprintln!(
                "{} {name}: {} chars, {} keys, {:.1} s emulated = {:.1} chars/s, {:.3} s wall = {:.0} chars/s",
                m.model().name(),
                out.typed,
                out.presses,
                out.emulated_ms / 1000.0,
                n / (out.emulated_ms / 1000.0),
                wall,
                n / wall
            );
            m.load_state(&state).unwrap();
        }
    }
}
