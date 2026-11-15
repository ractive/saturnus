//! What has the keyboard, on the real ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`,
//! `rom.49g`): each screen the AI tools name (the stack, a command line, a
//! halted program, an input form, a choose box, a message box, the
//! graphics screen, INPUT, the 48's EquationWriter, the interactive stack,
//! an object edited on the command line, the MatrixWriter, another loop)
//! opened by its keys or command, read by `uiState`, then left with
//! ATTN/CANCEL back to the stack, as the tools' `to_stack` does (wiki:
//! hardware/ui-state).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::transfer::Op;
use saturnus_host::typing::{Job, Verb};
use saturnus_objects::screen::{Kind, Screen, Source, screen};

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

/// A cold-booted machine at the stack, the 49G in RPN mode.
fn boot(model: Model, file: &str) -> Option<Machine> {
    let mut m = Machine::new(model, &rom(file)?).unwrap();
    settle(&mut m, 60_000);
    let n = if model == Model::Hp49g { 2 } else { 1 };
    for _ in 0..n {
        press(&mut m, Key::F);
    }
    let mut e = Emulator::from_machine(m);
    if model == Model::Hp49g {
        e.start_transfer(Op::SetFlag {
            flag: -95,
            on: false,
        })
        .unwrap();
        while !e.transfer_step(20.0).unwrap() {}
        e.transfer_result().unwrap();
    }
    Some(e.machine().clone())
}

/// `r:TEXT` types TEXT and ENTER; anything else is key names.
fn act(m: &mut Machine, steps: &[&str]) {
    for step in steps {
        if let Some(text) = step.strip_prefix("r:") {
            let mut job = Job::new(m, Verb::Run, text).unwrap();
            while !job.step(m, 50 * per_ms(m)).unwrap() {}
        } else {
            for k in step.split_whitespace() {
                press(m, Key::from_name(k).unwrap_or_else(|| panic!("no key {k}")));
            }
        }
    }
}

/// Press ATTN/CANCEL until the stack has the keyboard, at most five
/// times; the presses it took.
fn to_stack(m: &mut Machine) -> usize {
    for n in 0..=5 {
        if screen(m).unwrap().stack_in_control {
            return n;
        }
        press(m, Key::On);
    }
    panic!("still not at the stack: {:?}", screen(m).unwrap());
}

struct Case {
    what: &'static str,
    steps: &'static [&'static str],
    kind: Kind,
    from: Source,
    halted: u32,
}

const fn case(what: &'static str, steps: &'static [&'static str], kind: Kind) -> Case {
    Case {
        what,
        steps,
        kind,
        from: Source::Memory,
        halted: 0,
    }
}

const fn shown(what: &'static str, steps: &'static [&'static str], kind: Kind) -> Case {
    Case {
        from: Source::Display,
        ..case(what, steps, kind)
    }
}

const fn halted(what: &'static str, steps: &'static [&'static str], kind: Kind, n: u32) -> Case {
    Case {
        halted: n,
        ..case(what, steps, kind)
    }
}

/// The 48SX's and 48GX's EquationWriter: entry, selection and scrolling
/// mode.
const WRITER: &[Case] = &[
    case(
        "the equation writer",
        &["leftshift enter"],
        Kind::EquationWriter,
    ),
    case(
        "the equation writer's selection mode",
        &["leftshift enter", "1 plus 2", "left"],
        Kind::EquationWriter,
    ),
    case(
        "EDIT in the equation writer's selection mode",
        &["leftshift enter", "1 plus 2", "left", "b"],
        Kind::Input,
    ),
    case(
        "PICTURE in the equation writer",
        &["leftshift enter", "1 plus 2", "leftshift left"],
        Kind::EquationView,
    ),
    case(
        "down on an algebraic",
        &["r:'A+B'", "down"],
        Kind::EquationView,
    ),
];

/// The 48SX's and 48GX's interactive stack, edits and MatrixWriter (VIEW is
/// menu key b in the interactive stack, ◀EDIT is left shift and +/-).
const STACK_48: &[Case] = &[
    case(
        "the matrix writer",
        &["rightshift enter"],
        Kind::MatrixWriter,
    ),
    case(
        "a cell typed in the matrix writer",
        &["rightshift enter", "1 enter 2"],
        Kind::MatrixWriter,
    ),
    case(
        "EDIT on a real",
        &["r:1 2 3.5", "leftshift neg"],
        Kind::ObjectEdit,
    ),
    case(
        "VIEW in the interactive stack",
        &["r:1 2 3", "up up b"],
        Kind::ObjectEdit,
    ),
    case(
        "EDIT in the interactive stack",
        &["r:1 2 3", "up up leftshift neg"],
        Kind::ObjectEdit,
    ),
    case(
        "VIEW on a matrix in the interactive stack",
        &["r:[[1 2][3 4]] 7", "up up b"],
        Kind::MatrixWriter,
    ),
];

/// Screens all three models have.
const COMMON: &[Case] = &[
    case("the stack", &[], Kind::Stack),
    case("after an error", &["r:DROP DROP"], Kind::Stack),
    case("a command line", &["1 2"], Kind::CommandLine),
    halted(
        "a halted program",
        &["r:« 1 HALT 2 » EVAL"],
        Kind::Halted,
        1,
    ),
    halted(
        "two halted programs",
        &["r:« 1 HALT 2 » EVAL", "r:« 3 HALT 4 » EVAL"],
        Kind::Halted,
        2,
    ),
    halted(
        "a command line over a halted program",
        &["r:« 1 HALT 2 » EVAL", "1"],
        Kind::CommandLine,
        1,
    ),
    halted("PROMPT", &["r:« \"SAY\" PROMPT 5 » EVAL"], Kind::Halted, 1),
    case("INPUT", &["r:« \"ASK\" \"\" INPUT » EVAL"], Kind::Input),
    case(
        "INPUT with options",
        &["r:« \"ASK\" { \"\" V } INPUT » EVAL"],
        Kind::Input,
    ),
    case(
        "INPUT from a menu key",
        &["r:« \"ASK\" \"\" INPUT » 'P' STO", "var a"],
        Kind::Input,
    ),
    case(
        "INPUT in a program with local variables",
        &["r:5 « → a « \"ASK\" \"\" INPUT » » EVAL"],
        Kind::Input,
    ),
    case("PVIEW", &["r:{ } PVIEW"], Kind::Graphics),
    case(
        "the interactive stack",
        &["r:1 2 3", "up"],
        Kind::InteractiveStack,
    ),
    case(
        "the interactive stack's pointer moved",
        &["r:1 2 3", "up up"],
        Kind::InteractiveStack,
    ),
    case("down on a real", &["r:1 2 3.5", "down"], Kind::ObjectEdit),
    case("down on a string", &["r:\"ABC\"", "down"], Kind::ObjectEdit),
    case(
        "down on a program",
        &["r:« 1 2 + »", "down"],
        Kind::ObjectEdit,
    ),
    case("down on a list", &["r:{ 1 2 }", "down"], Kind::ObjectEdit),
    case("down on a name", &["r:'X'", "down"], Kind::ObjectEdit),
    halted(
        "down over a halted program",
        &["r:« 1 HALT 2 » EVAL", "r:4.5", "down"],
        Kind::ObjectEdit,
        1,
    ),
    case(
        "down on a matrix",
        &["r:[[1 2][3 4]]", "down"],
        Kind::MatrixWriter,
    ),
    case("WAIT", &["r:« 0 WAIT » EVAL"], Kind::Unknown),
];

/// The `COMMON` cases, then `own` and `more`, on `model`.
fn check(model: Model, file: &str, own: &[Case], more: &[&[Case]]) {
    let Some(base) = boot(model, file) else {
        eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
        return;
    };
    for c in COMMON
        .iter()
        .chain(own)
        .chain(more.iter().copied().flatten())
    {
        let mut m = base.clone();
        act(&mut m, c.steps);
        let s: Screen = screen(&m).unwrap();
        let what = format!("{model:?} {file} {}: {s:?}", c.what);
        assert_eq!(s.kind, c.kind, "{what}");
        assert_eq!(s.from, c.from, "{what}");
        assert_eq!(s.halted, c.halted, "{what}");
        let in_control = matches!(c.kind, Kind::Stack | Kind::CommandLine | Kind::Halted);
        assert_eq!(s.stack_in_control, in_control, "{what}");
        if !in_control {
            let presses = to_stack(&mut m);
            assert!(presses >= 1, "{what}");
            assert_eq!(
                screen(&m).unwrap().halted,
                c.halted,
                "{what}: CANCEL left halted programs alone"
            );
        }
    }
}

#[test]
fn hp48sx_screens() {
    check(
        Model::Hp48sx,
        "sxrom-j",
        &[
            case("GRAPH", &["leftshift left"], Kind::Graphics),
            case(
                "VISIT",
                &["r:5 'X' STO 'X'", "rightshift neg"],
                Kind::ObjectEdit,
            ),
        ],
        &[WRITER, STACK_48],
    );
}

#[test]
fn hp48gx_screens() {
    check(
        Model::Hp48gx,
        "gxrom-r",
        &[
            case("the PLOT form", &["rightshift 8"], Kind::InputForm),
            case("the MODES form", &["rightshift cst"], Kind::InputForm),
            case(
                "a field typed into",
                &["rightshift 8", "down down alpha tan"],
                Kind::InputForm,
            ),
            case(
                "INFORM",
                &["r:\"T\" { \"A:\" } { } { } { } INFORM"],
                Kind::InputForm,
            ),
            // The owner's report: UPDIR and ENTER typed into the PLOT
            // form's first field: a message box over the form.
            shown(
                "UPDIR typed into the form",
                &[
                    "rightshift 8",
                    "alpha alpha tan left d cst right alpha enter",
                ],
                Kind::Message,
            ),
            shown(
                "a choose box over the form",
                &["rightshift 8", "b"],
                Kind::ChooseBox,
            ),
            shown(
                "CHOOSE",
                &["r:\"PICK\" { 1 2 3 } 1 CHOOSE"],
                Kind::ChooseBox,
            ),
            shown("the SOLVE choose box", &["rightshift 7"], Kind::ChooseBox),
            shown("MSGBOX", &["r:\"HELLO\" MSGBOX"], Kind::Message),
            case("PICTURE", &["leftshift left"], Kind::Graphics),
            case(
                "the equation writer from a form's field",
                &["rightshift 8", "down down", "leftshift enter"],
                Kind::EquationWriter,
            ),
            case(
                "VIEW on a real",
                &["r:1 2 3.5", "leftshift down"],
                Kind::ObjectEdit,
            ),
            case(
                "EDIT on a name (VISIT)",
                &["r:5 'X' STO 'X'", "leftshift neg"],
                Kind::ObjectEdit,
            ),
            case(
                "EDIT in the statistics form",
                &["rightshift 5", "enter", "a"],
                Kind::MatrixWriter,
            ),
        ],
        &[WRITER, STACK_48],
    );
}

/// The 49G's interactive stack, edits and MatrixWriter (in the
/// interactive stack's menu EDIT is key c, ROLL key e; TOOL's EDIT is key
/// a).
const STACK_49: &[Case] = &[
    case("HIST", &["r:1 2 3", "hist"], Kind::InteractiveStack),
    case("MTRW", &["leftshift eqw"], Kind::MatrixWriter),
    case(
        "a cell typed in MTRW",
        &["leftshift eqw", "1 enter 2"],
        Kind::MatrixWriter,
    ),
    case("TOOL EDIT", &["r:1 2 3.5", "tool a"], Kind::ObjectEdit),
    case(
        "EDIT in the interactive stack",
        &["r:1 2 3", "up up c"],
        Kind::ObjectEdit,
    ),
    case(
        "EDIT in the statistics form",
        &["rightshift 5", "enter", "a"],
        Kind::MatrixWriter,
    ),
];

#[test]
fn hp49g_screens() {
    for file in ["rom-2.10.49g", "rom.49g"] {
        hp49g(file);
    }
}

fn hp49g(file: &str) {
    check(
        Model::Hp49g,
        file,
        &[
            case("the MODE form", &["mode"], Kind::InputForm),
            case(
                "INFORM",
                &["r:\"T\" { \"A:\" } { } { } { } INFORM"],
                Kind::InputForm,
            ),
            case("APPS", &["apps"], Kind::ChooseBox),
            case(
                "CHOOSE",
                &["r:\"PICK\" { 1 2 3 } 1 CHOOSE"],
                Kind::ChooseBox,
            ),
            shown("MSGBOX", &["r:\"HELLO\" MSGBOX"], Kind::Message),
            shown("the error box", &["1 point 2 point 3 enter"], Kind::Message),
            // Its editor line is the command line's: told as INPUT is.
            case("the equation writer", &["eqw"], Kind::Input),
            case("down on an algebraic", &["r:'A+B'", "down"], Kind::Input),
        ],
        &[STACK_49],
    );
}

/// The tools' rule for the 48's EquationWriter: one CANCEL in scrolling
/// mode returns to the writer; opened by down from the stack, a second
/// CANCEL closes it and leaves the algebraic where it was.
#[test]
fn cancel_leaves_the_equation_view_for_the_writer() {
    for (model, file) in [(Model::Hp48sx, "sxrom-j"), (Model::Hp48gx, "gxrom-r")] {
        let Some(base) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let mut m = base.clone();
        act(&mut m, &["leftshift enter", "1 plus 2", "leftshift left"]);
        assert_eq!(screen(&m).unwrap().kind, Kind::EquationView, "{model:?}");
        press(&mut m, Key::On);
        assert_eq!(screen(&m).unwrap().kind, Kind::EquationWriter, "{model:?}");

        let mut m = base.clone();
        act(&mut m, &["r:'A+B'", "down"]);
        assert_eq!(screen(&m).unwrap().kind, Kind::EquationView, "{model:?}");
        press(&mut m, Key::On);
        assert_eq!(screen(&m).unwrap().kind, Kind::EquationWriter, "{model:?}");
        press(&mut m, Key::On);
        let s = screen(&m).unwrap();
        assert_eq!(s.kind, Kind::Stack, "{model:?}: {s:?}");
        let stack = Emulator::from_machine(m).stack().unwrap();
        assert_eq!(stack.len(), 1, "{model:?}: {stack:?}");
        assert_eq!(stack[0]["text"], "'A+B'", "{model:?}: {stack:?}");
    }
}

/// The ROMs of the three models, with the 49G's two.
const ROMS: [(Model, &str); 4] = [
    (Model::Hp48sx, "sxrom-j"),
    (Model::Hp48gx, "gxrom-r"),
    (Model::Hp49g, "rom-2.10.49g"),
    (Model::Hp49g, "rom.49g"),
];

/// The stack's levels' texts, level 1 first.
fn texts(m: &Machine) -> Vec<String> {
    let stack = Emulator::from_machine(m.clone()).stack().unwrap();
    stack
        .iter()
        .map(|o| o["text"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// The manuals: CANCEL leaves the interactive stack and keeps the
/// changes made there (a ROLL of level 3).
#[test]
fn cancel_keeps_what_the_interactive_stack_did() {
    for (model, file) in ROMS {
        let Some(base) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let roll = if model == Model::Hp49g { "e" } else { "d" };
        let mut m = base.clone();
        act(&mut m, &["r:1 2 3", "up up up", roll]);
        assert_eq!(screen(&m).unwrap().kind, Kind::InteractiveStack, "{file}");
        press(&mut m, Key::On);
        let s = screen(&m).unwrap();
        assert_eq!(s.kind, Kind::Stack, "{file}: {s:?}");
        assert_eq!(texts(&m), ["1", "3", "2"], "{file}");
    }
}

/// ▼ takes the level into the command line; CANCEL puts the original back
/// and ENTER the edited object.
#[test]
fn cancel_restores_an_edited_level() {
    for (model, file) in ROMS {
        let Some(base) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let mut m = base.clone();
        act(&mut m, &["r:1 2 3.5", "down", "9"]);
        assert_eq!(screen(&m).unwrap().kind, Kind::ObjectEdit, "{file}");
        assert_eq!(
            texts(&m).len(),
            2,
            "{file}: the edited level is off the stack"
        );
        let mut entered = m.clone();
        press(&mut m, Key::On);
        assert_eq!(screen(&m).unwrap().kind, Kind::Stack, "{file}");
        assert_eq!(texts(&m)[0], "3.5", "{file}");
        assert_eq!(texts(&m).len(), 3, "{file}");
        press(&mut entered, Key::Enter);
        assert_eq!(screen(&entered).unwrap().kind, Kind::Stack, "{file}");
        assert_ne!(
            texts(&entered)[0],
            "3.5",
            "{file}: ENTER puts the edit back"
        );
        assert_eq!(texts(&entered).len(), 3, "{file}");
    }
}

/// ENTER with nothing typed closes the MatrixWriter and puts the matrix
/// on the stack; CANCEL drops a new one.
#[test]
fn enter_puts_the_matrix_writers_matrix_on_the_stack() {
    for (model, file) in ROMS {
        let Some(base) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let open = if model == Model::Hp49g {
            "leftshift eqw"
        } else {
            "rightshift enter"
        };
        let mut m = base.clone();
        act(&mut m, &[open, "1 enter 2 enter"]);
        assert_eq!(screen(&m).unwrap().kind, Kind::MatrixWriter, "{file}");
        let mut cancelled = m.clone();
        press(&mut m, Key::Enter);
        assert_eq!(screen(&m).unwrap().kind, Kind::Stack, "{file}");
        let stack = texts(&m);
        assert_eq!(stack.len(), 1, "{file}: {stack:?}");
        assert!(stack[0].starts_with('['), "{file}: {stack:?}");
        press(&mut cancelled, Key::On);
        assert_eq!(screen(&cancelled).unwrap().kind, Kind::Stack, "{file}");
        assert!(texts(&cancelled).is_empty(), "{file}");
    }
}
