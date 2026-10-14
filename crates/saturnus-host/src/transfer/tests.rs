use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn mem_is_read_in_any_display_mode() {
    for (shown, n) in [
        ("12345.", 12345.0),
        ("12345", 12345.0),
        (" 12345.00 ", 12345.0),
        ("12345,5", 12345.5),
        ("1.2345E4", 12345.0),
        ("1,2345E4", 12345.0),
    ] {
        assert_eq!(parse_number(shown), Some(n), "{shown}");
    }
    assert_eq!(parse_number("\"x\""), None);
    assert_eq!(parse_number(""), None);
}

#[test]
fn directory_files_are_told_apart() {
    // HPHP48-R, then the prolog 02A96 low nibble first: 6 9 A 2 0.
    let mut dir = b"HPHP48-R".to_vec();
    dir.extend([0x96, 0x2A, 0x00]);
    assert!(holds_directory(&dir));
    let mut real = b"HPHP48-R".to_vec();
    real.extend([0x33, 0x29, 0x00]);
    assert!(!holds_directory(&real));
    assert!(!holds_directory(b"%%HP: T(3);\n1"));
    assert!(!holds_directory(b"HPHP48-R"));
}

#[test]
fn directories_and_names() {
    assert_eq!(components(&s(&["HOME", "A", "B"])).unwrap(), s(&["A", "B"]));
    assert_eq!(components(&s(&["A"])).unwrap(), s(&["A"]));
    assert!(components(&s(&["HOME", "A B"])).is_err());
    assert_eq!(cd_command(&s(&["A", "B"])), "HOME A B");
    assert_eq!(cd_command(&[]), "HOME");
    assert!(check_name("X1").is_ok());
    assert!(check_name("1X").is_err());
    assert!(check_name("A'B").is_err());
    assert!(inside(&s(&["A", "B"]), &[], "A"));
    assert!(!inside(&s(&["A"]), &s(&["A"]), "B"));
    assert!(!inside(&[], &[], "A"));
}

#[test]
fn files_to_store() {
    let bin = b"HPHP48-X\x33\x92\x02".to_vec();
    assert_eq!(store_bytes(&bin), (bin.clone(), true));
    // UTF-8 text becomes the calculator's set: « is 171 there.
    let (b, binary) = store_bytes("%%HP: T(3);\n« 1 »".as_bytes());
    assert!(!binary);
    assert!(b.ends_with(&[171, b' ', b'1', b' ', 187]));
    // Not UTF-8: sent as it is.
    assert_eq!(store_bytes(&[0xFF, 0x41]), (vec![0xFF, 0x41], false));
}

#[test]
fn fetched_files_lose_the_padding() {
    // A real 1.: prolog 02933, exponent 000, mantissa 1000..., sign 0.
    let real = saturnus_objects::Object::Real {
        value: saturnus_objects::Real::parse("1").unwrap(),
    };
    let file = tfile::encode_file(&real, tfile::Family::Hp48)
        .unwrap()
        .unwrap();
    let mut padded = file.clone();
    padded.extend([0, 0, 0]);
    assert_eq!(trim_fetched(padded), file);
    // Not an HP binary file: kept whole.
    assert_eq!(trim_fetched(b"text\0\0".to_vec()), b"text\0\0");
}

#[test]
fn models_without_a_server_are_refused() {
    let m = Machine::new(Model::Hp38g, &vec![0u8; Model::Hp38g.rom_bytes()]).unwrap();
    let e = Transfer::new(&m, Op::ChangeDir { dir: s(&["HOME"]) }).unwrap_err();
    assert!(e.to_string().contains("38G has no Kermit server"), "{e}");
}

/// A host command that failed leaves its arguments: what is above the
/// first reply's level count is dropped before anything else runs.
#[test]
fn a_failed_command_drops_what_it_left() {
    let m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
    let mut t = Transfer::planned(
        &m,
        VecDeque::from([Step::Host {
            text: "-35 CF".into(),
            cleanup: true,
        }]),
        TransferResult::default(),
    );
    let host = |text: &str| Step::Host {
        text: text.into(),
        cleanup: false,
    };
    let reply = |text: &str| Transcript {
        text: text.as_bytes().to_vec(),
        ..Transcript::default()
    };
    t.finished(host(""), reply("1: 42\r\n"));
    assert_eq!(t.baseline, Some(1));
    t.finished(
        host("'A' RCL 'B' STO"),
        reply("Error: Bad Argument Type\r\n3: 42\r\n2: 7\r\n1: 'B'\r\n"),
    );
    assert_eq!(
        t.error.as_deref(),
        Some("'A' RCL 'B' STO: Bad Argument Type")
    );
    let texts: Vec<_> = t
        .steps
        .iter()
        .map(|s| match s {
            Step::Host { text, cleanup } => (text.as_str(), *cleanup),
            _ => ("?", false),
        })
        .collect();
    assert_eq!(texts, [("2 DROPN", true), ("-35 CF", true)]);
    // Only cleanup steps run after an error.
    assert!(!host("X").cleanup() && Step::Finish.cleanup());
}

/// A stop while ON is held to end a dead server lets go of ON at once;
/// the server's end then runs in turns, each within its budget of
/// emulated time, not in one call.
#[test]
fn a_stop_lets_go_of_on_and_ends_the_server_in_turns() {
    let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
    let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
    t.server = true;
    t.running = t.abort(&mut m, "Kermit: timeout".into());
    assert!(matches!(t.running, Some(Running::EndServer { .. })));
    assert!(m.key_is_down(Key::On));
    let before = m.cycles();
    t.stop(&mut m);
    assert_eq!(m.cycles(), before, "a stop runs nothing itself");
    assert!(!m.key_is_down(Key::On), "ON released");
    assert!(!t.done);
    assert!(matches!(t.steps.front(), Some(Step::EndServer)));
    // Each turn keeps to its budget (one emulated millisecond's
    // worth of slack for the step in progress).
    let budget = ms_cycles(&m, 50);
    let mut turns = 0;
    loop {
        let at = m.cycles();
        let done = t.step(&mut m, budget).unwrap();
        assert!(
            m.cycles() - at <= budget + ms_cycles(&m, 10),
            "turn {turns}"
        );
        turns += 1;
        if done {
            break;
        }
        assert!(turns < 2_000, "the stop ends");
    }
    assert!(turns > 1, "in turns: {turns}");
    assert!(!m.key_is_down(Key::On));
    assert_eq!(t.error.as_deref(), Some("Kermit: timeout"));
}

/// A stopped backup removes what it made, and only that: the stack's
/// extra levels dropped first (a recalled archive keeps the port's
/// copy in use), then the port copy and the variable purged by typed
/// lines once the server has ended; nothing for a port name it did not
/// archive. A replacing store's kept temporary is named in the error.
#[test]
fn a_stopped_backup_removes_what_it_made() {
    let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
    let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
    t.leftovers.backup = Some("SATBKUP".into());
    t.baseline = Some(2);
    t.leftovers.started(":0:SATBKUP ARCHIVE");
    t.leftovers.started("'SATBKUP' STO");
    t.stop(&mut m);
    let tail: Vec<String> = t
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::DropTo(n) => Some(format!("drop to {n}")),
            Step::TypeCleanup(l) => Some(l.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        tail,
        [
            "drop to 2",
            "IFERR :0:SATBKUP PURGE THEN END",
            "IFERR 'SATBKUP' PGDIR THEN END"
        ]
    );
    // An ARCHIVE refused (the name taken in port 0): the port is not purged.
    let mut l = Leftovers {
        backup: Some("SATBKUP".into()),
        ..Leftovers::default()
    };
    l.started(":0:SATBKUP ARCHIVE");
    l.failed(":0:SATBKUP ARCHIVE");
    assert!(l.typed_cleanup().is_empty());
    let r = Leftovers {
        replace: Some(("SATREPL".into(), "X".into(), "HOME".into())),
        temp: true,
        ..Leftovers::default()
    };
    assert_eq!(
        r.note().as_deref(),
        Some("the new X is kept as SATREPL in { HOME }")
    );
}

/// The probe after ON counts a Kermit packet from the calculator as a
/// running server's answer, and nothing as an ended server.
#[test]
fn a_packet_is_an_answer_and_silence_is_not() {
    use kermit_proto::BlockCheck;
    use kermit_proto::codec::{Framing, Packet};
    let packet = |kind: u8, data: &[u8]| {
        Packet::new(0, kind, data.to_vec())
            .encode(BlockCheck::Type1, &Framing::default())
            .unwrap()
    };
    let ack = packet(b'Y', &kermit_proto::InitParams::default().encode());
    assert!(is_answer(&ack), "its ACK of the I packet");
    assert!(is_answer(&packet(b'N', b"")), "a NAK");
    assert!(is_answer(&packet(b'E', b"Bad")), "an E packet");
    assert!(!is_answer(&[]), "silence");
}

/// After ON, a calculator that does not answer the I packet has ended
/// its server: no `G F` follows, and the I packet's bytes are dropped.
#[test]
fn a_silent_calculator_after_on_has_ended_its_server() {
    let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
    let mut t = Transfer::planned(
        &m,
        VecDeque::from([Step::EndServer]),
        TransferResult::default(),
    );
    t.server = true;
    let start = m.cycles();
    let mut probed = false;
    let turn = ms_cycles(&m, 50);
    while !t.step(&mut m, turn).unwrap() {
        probed |= matches!(t.running, Some(Running::Probe { .. }));
    }
    assert!(probed, "an I packet asked");
    assert!(!t.server, "ended");
    assert!(t.steps.is_empty(), "no G F: {:?}", t.steps);
    assert_eq!(m.serial_pending(), 0, "the probe's bytes are dropped");
    assert!(!m.key_is_down(Key::On));
    let ms = (m.cycles() - start) / ms_cycles(&m, 1);
    let least = ON_HOLD_MS + u64::try_from(FINISH_SETTLE.as_millis()).unwrap();
    assert!(ms >= least && ms <= least + PROBE_CAP_MS + 100, "{ms} ms");
}

/// On the real ROMs (`SATURNUS_ROM_DIR`), each case on its own result:
/// ON on a server silent since its one NAK ends it, and its probe
/// queues no `G F`; a server silent since its NAK answers the probe,
/// and its `G F` exchange completes; back at the stack the probe hears
/// nothing.
#[test]
fn the_probe_tells_a_silent_running_server_from_an_ended_one() {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        return;
    };
    for (model, file) in [
        (Model::Hp48sx, "sxrom-j"),
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp49g, "rom-2.10.49g"),
    ] {
        let Ok(rom) = std::fs::read(std::path::Path::new(&dir).join(file)) else {
            eprintln!("skipped: no {file}");
            continue;
        };
        let mut m = Machine::new(model, &rom).unwrap();
        boot(&mut m);
        let name = model.name();
        // 1. ON on a server silent since its NAK: the probe after it
        // hears nothing, so no G F is queued.
        let mut t = silent_server(&mut m);
        let mut running = Some(t.press_on(&mut m));
        let mut probed = false;
        while let Some(r) = running.take() {
            probed |= matches!(r, Running::Probe { .. });
            let stop = m.cycles() + ms_cycles(&m, 50);
            running = t.advance(&mut m, r, stop).unwrap();
        }
        assert!(probed, "{name}: probed after ON");
        assert!(!t.server, "{name}: ON ended it");
        assert!(t.steps.is_empty(), "{name}: no G F: {:?}", t.steps);
        assert!(!t.probe_finish && t.error.is_none(), "{name}");
        // 2. A server silent since its NAK answers the probe, and its
        // G F completes.
        let mut t = silent_server(&mut m);
        assert!(probe_answers(&mut t, &mut m), "{name}: it answered");
        let finish = exchange(&mut t, &mut m, Command::Finish);
        assert!(finish.is_ok(), "{name}: G F: {:?}", finish.err());
        m.run_cycles(cycles(&m, FINISH_SETTLE + Duration::from_secs(1)))
            .unwrap();
        // 3. Back at the stack, the probe hears nothing.
        assert!(!probe_answers(&mut t, &mut m), "{name}: ended");
        let line = cmdline::command_line(&m).unwrap();
        assert!(!line.active, "{name}: at its stack");
    }
}

/// Run `x` to its outcome, at most until the cycle count `cap`.
fn exchange_out(
    t: &mut Transfer,
    m: &mut Machine,
    x: &mut Exchange,
    cap: u64,
) -> Option<Result<Transcript, Failure>> {
    while m.cycles() < cap {
        let stop = (m.cycles() + ms_cycles(m, 50)).min(cap);
        if let Some(outcome) = x.step(&mut t.wire, m, stop).unwrap() {
            return Some(outcome);
        }
    }
    None
}

/// The probe's I exchange alone: did the calculator answer?
fn probe_answers(t: &mut Transfer, m: &mut Machine) -> bool {
    let mut x = t.probe_exchange(m).unwrap();
    let cap = m.cycles() + ms_cycles(m, PROBE_CAP_MS);
    exchange_out(t, m, &mut x, cap);
    t.wire.discard(m);
    m.serial_clear_inbound();
    x.answered
}

/// One transaction to its end (a minute of emulated time at most).
fn exchange(t: &mut Transfer, m: &mut Machine, command: Command) -> Result<Transcript, Failure> {
    let mut x = Exchange::new(&mut t.wire, m, command).unwrap();
    let cap = m.cycles() + ms_cycles(m, 60_000);
    exchange_out(t, m, &mut x, cap).unwrap_or(Err(Failure::Link("no end".into())))
}

/// A machine whose CPU only sleeps (SHUTDN at #00000, then RTNSXM
/// back to it): the link's clock runs, nothing ever answers.
fn sleeper() -> Machine {
    let mut rom = vec![0u8; Model::Hp48sx.rom_bytes()];
    rom[0] = 0x08;
    rom[1] = 0x07;
    Machine::new(Model::Hp48sx, &rom).unwrap()
}

/// A `G F` that fails after the probe was answered gets ON and the
/// probe again; a silent second probe ends it without an error.
#[test]
fn a_failed_g_f_after_an_answer_gets_on_and_the_probe_again() {
    let mut m = sleeper();
    let steps = VecDeque::from([Step::Finish, Step::Settle]);
    let mut t = Transfer::planned(&m, steps, TransferResult::default());
    t.server = true;
    t.probe_finish = true;
    t.probes = 1;
    run_out(&mut t, &mut m);
    assert_eq!(t.probes, 2, "ON and the probe again");
    assert!(!t.server);
    assert!(t.error.is_none(), "{:?}", t.error);
    assert!(!m.key_is_down(Key::On));
}

/// [`Op::Server`]'s question to a calculator that never answers: no
/// server, no `G F`, no ON, the I packet's bytes dropped, within
/// [`ASK_CAP_MS`].
#[test]
fn a_silent_calculator_runs_no_server() {
    for end in [false, true] {
        let mut m = sleeper();
        let steps = VecDeque::from([Step::Ask { end }]);
        let mut t = Transfer::planned(&m, steps, TransferResult::default());
        let start = m.cycles();
        run_out(&mut t, &mut m);
        assert_eq!(t.result.server, Some(false), "end: {end}");
        assert!(!t.server && t.error.is_none(), "{:?}", t.error);
        assert_eq!(t.probes, 0, "no ON round");
        assert_eq!(m.serial_pending(), 0, "the I packet's bytes are dropped");
        assert!(!m.key_is_down(Key::On));
        let ms = (m.cycles() - start) / ms_cycles(&m, 1);
        assert!(ms <= ASK_CAP_MS + 100, "{ms} ms");
    }
}

/// A stop during `probeServer`'s question presses nothing and ends no
/// server; during `endServer`'s it ends one as after ON.
#[test]
fn a_stop_in_the_question_ends_a_server_only_for_end_server() {
    for end in [false, true] {
        let mut m = sleeper();
        let steps = VecDeque::from([Step::Ask { end }]);
        let mut t = Transfer::planned(&m, steps, TransferResult::default());
        let turn = ms_cycles(&m, 50);
        while !matches!(t.running, Some(Running::Probe { .. })) {
            assert!(!t.step(&mut m, turn).unwrap());
        }
        t.stop(&mut m);
        assert_eq!(
            matches!(t.steps.front(), Some(Step::EndServer)),
            end,
            "end: {end}"
        );
        run_out(&mut t, &mut m);
        assert!(!m.key_is_down(Key::On));
        assert_eq!(m.serial_pending(), 0);
    }
}

/// After [`END_ROUNDS`] the failed `G F` is an error.
#[test]
fn a_server_that_cannot_be_ended_is_an_error() {
    let mut m = sleeper();
    let steps = VecDeque::from([Step::Finish, Step::Settle]);
    let mut t = Transfer::planned(&m, steps, TransferResult::default());
    t.server = true;
    t.probe_finish = true;
    t.probes = END_ROUNDS;
    run_out(&mut t, &mut m);
    assert_eq!(t.probes, END_ROUNDS, "no further round");
    assert!(!t.server);
    let e = t.error.as_deref().unwrap_or_default();
    assert!(e.starts_with("the server could not be ended"), "{e}");
}

/// A write given up during the probe leaves no inbound bytes behind.
#[test]
fn an_abandoned_probe_drops_its_inbound_bytes() {
    let mut m = sleeper();
    let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
    t.server = true;
    let mut running = t.probe(&mut m).unwrap();
    // Past the turnaround, until the I packet is on its way.
    for _ in 0..1_000 {
        if m.serial_pending() > 0 {
            break;
        }
        let stop = m.cycles() + ms_cycles(&m, 1);
        running = t.advance(&mut m, running, stop).unwrap().unwrap();
    }
    assert!(m.serial_pending() > 0, "the I packet is on its way");
    t.running = Some(running);
    t.abandon(&mut m);
    assert_eq!(m.serial_pending(), 0);
}

/// Cold boot to the stack (the start-up prompts answered with F).
fn boot(m: &mut Machine) {
    m.run_cycles(ms_cycles(m, 10_000)).unwrap();
    let presses = if m.model() == Model::Hp49g { 2 } else { 1 };
    for _ in 0..presses {
        m.key_down(Key::F).unwrap();
        m.run_cycles(ms_cycles(m, 30)).unwrap();
        m.key_up(Key::F).unwrap();
        m.run_cycles(ms_cycles(m, 3_000)).unwrap();
    }
}

/// `SERVER` typed and its NAK seen, then 20 s of emulated time in
/// which the running server sends nothing.
fn silent_server(m: &mut Machine) -> Transfer {
    let steps = VecDeque::from([Step::Type("SERVER".into()), Step::WaitNak]);
    let mut t = Transfer::planned(m, steps, TransferResult::default());
    run_out(&mut t, m);
    assert!(t.server && t.error.is_none(), "{:?}", t.error);
    m.run_cycles(ms_cycles(m, 20_000)).unwrap();
    assert!(m.serial_drain().is_empty(), "silent after its one NAK");
    t.done = false;
    t
}

fn run_out(t: &mut Transfer, m: &mut Machine) {
    let turn = ms_cycles(m, 50);
    while !t.step(m, turn).unwrap() {}
}

/// Setting -95 again can fail: that is reported, also with another
/// error and after a stop, never dropped.
#[test]
fn a_failed_return_to_algebraic_mode_is_reported() {
    let m = Machine::new(Model::Hp49g, &vec![0u8; Model::Hp49g.rom_bytes()]).unwrap();
    let failed = |error: Option<&str>, stopped: bool| {
        let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
        t.error = error.map(str::to_string);
        t.stopped = stopped;
        t.restored(&m, Some("the calculator refused it: Invalid Syntax".into()));
        t.finish(&m).unwrap_err().to_string()
    };
    let why = "the 49G stays in RPN mode: the calculator refused it: Invalid Syntax";
    assert_eq!(failed(None, false), why);
    assert_eq!(
        failed(Some("CRDIR: Bad Argument Type"), false),
        format!("CRDIR: Bad Argument Type; {why}")
    );
    assert_eq!(failed(Some(STOPPED), true), why);
    // A stop with nothing else to say: stopped.
    let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
    t.error = Some(STOPPED.into());
    t.stopped = true;
    assert_eq!(t.finish(&m).unwrap_err().to_string(), STOPPED);
}

/// The 49G in algebraic mode: -95 cleared by keys before `SERVER`, set
/// again by keys after the server's end; that runs after a refusal (an
/// E packet, a failed command) and after a dead server ended with ON.
#[test]
fn algebraic_mode_is_set_again_on_every_path() {
    let mut m = Machine::new(Model::Hp49g, &vec![0u8; Model::Hp49g.rom_bytes()]).unwrap();
    let plan = || {
        let flags = Flags {
            system: vec![0; 2],
            user: vec![0; 2],
        };
        let steps = VecDeque::from([Step::Host {
            text: "'D' CRDIR".into(),
            cleanup: false,
        }]);
        let mut t = Transfer::serve(&m, &flags, steps, TransferResult::default()).unwrap();
        t.in_rpn(2);
        t
    };
    let name = |s: &Step| match s {
        Step::Type(t) => format!("type {t}"),
        Step::WaitNak => "nak".into(),
        Step::Host { text, .. } => format!("host {text}"),
        Step::Finish => "finish".into(),
        Step::Settle => "settle".into(),
        Step::DropTo(n) => format!("drop to {n}"),
        Step::Algebraic => "algebraic".into(),
        Step::EndServer => "end server".into(),
        _ => "?".into(),
    };
    let t = plan();
    assert_eq!(t.algebraic, Some(2), "a stop sets -95 again too");
    let all: Vec<String> = t.steps.iter().map(name).collect();
    assert_eq!(
        all,
        [
            "type CF(-95)",
            "drop to 2",
            "type SERVER",
            "nak",
            "host ",
            "host 'D' CRDIR",
            "finish",
            "settle",
            "algebraic",
            "drop to 2",
        ]
    );
    // After an error (the calculator's E packet, a failed command, a
    // refused line) only the cleanup runs: the server's end, then -95.
    let cleanup: Vec<String> = t.steps.iter().filter(|s| s.cleanup()).map(name).collect();
    assert_eq!(
        cleanup,
        ["drop to 2", "finish", "settle", "algebraic", "drop to 2"]
    );
    // A dead server (a timeout): ON, then -95 all the same.
    let mut t = plan();
    for _ in 0..6 {
        t.steps.pop_front();
    }
    t.server = true;
    t.running = t.abort(&mut m, "Kermit: timeout".into());
    assert!(matches!(t.running, Some(Running::EndServer { .. })));
    let left: Vec<String> = t.steps.iter().map(name).collect();
    assert_eq!(left, ["algebraic", "drop to 2"]);
    // A stop then: ON let go, the server's end and -95 still to come.
    t.stop(&mut m);
    assert!(!m.key_is_down(Key::On));
    let left: Vec<String> = t.steps.iter().map(name).collect();
    // (What a command left goes in RPN first, then again after -95.)
    assert_eq!(
        left,
        [
            "end server",
            "drop to 2",
            "settle",
            "algebraic",
            "drop to 2"
        ]
    );
}

/// `copy` and `move`: `RCL` then `STO`; a move checks the copy's size
/// and checksum before it purges the original. Refused: the same
/// directory, a directory into itself, a taken name unless replaced
/// (and never a directory, nor by one), moving the directory that
/// holds the current one.
#[test]
fn copies_and_moves_are_planned() {
    let texts = |steps: Vec<Step>| -> Vec<String> {
        steps
            .into_iter()
            .map(|s| match s {
                Step::Host { text, cleanup } => {
                    assert!(!cleanup, "{text}");
                    text
                }
                _ => "?".into(),
            })
            .collect()
    };
    let case = CopyCase::default();
    let a = s(&["A"]);
    let b = s(&["B", "C"]);
    assert_eq!(
        texts(copy_plan(&a, "X", &b, &case).unwrap()),
        ["HOME A", "'X' RCL", "HOME B C", "'X' STO"]
    );
    let mv = CopyCase {
        remove: true,
        ..case
    };
    assert_eq!(
        texts(copy_plan(&a, "X", &[], &mv).unwrap()),
        [
            "HOME A",
            "'X' RCL",
            "HOME",
            "'X' STO",
            "'X' RCL BYTES",
            "HOME A",
            "'X' RCL BYTES",
            "ROT == 3 ROLLD == AND",
            "« 'X' PURGE » « \"Copy differs\" DOERR » IFTE",
        ]
    );
    let dir = CopyCase {
        is_dir: true,
        remove: true,
        ..case
    };
    assert!(texts(copy_plan(&[], "D", &a, &dir).unwrap())[8].contains("'D' PGDIR"));
    let err = |from: &[String], into: &[String], c: CopyCase| {
        copy_plan(from, "D", into, &c).unwrap_err().to_string()
    };
    assert_eq!(err(&a, &a, case), "D is in { HOME A } already");
    assert_eq!(
        err(&[], &s(&["D", "E"]), dir),
        "D cannot go into itself or a directory inside it"
    );
    assert!(
        copy_plan(&[], "D", &s(&["DD"]), &dir).is_ok(),
        "only D itself"
    );
    assert!(
        err(
            &[],
            &a,
            CopyCase {
                holds_here: true,
                ..dir
            }
        )
        .contains("holds the current directory")
    );
    // A copy of the directory that holds the current one is fine.
    assert!(
        copy_plan(
            &[],
            "D",
            &a,
            &CopyCase {
                holds_here: true,
                is_dir: true,
                ..case
            }
        )
        .is_ok()
    );
    let taken = CopyCase {
        taken: Some(false),
        ..case
    };
    assert_eq!(err(&[], &a, taken), "D already exists in { HOME A }");
    assert!(
        copy_plan(
            &[],
            "D",
            &a,
            &CopyCase {
                replace: true,
                ..taken
            }
        )
        .is_ok()
    );
    assert!(
        err(
            &[],
            &a,
            CopyCase {
                replace: true,
                taken: Some(true),
                ..case
            }
        )
        .contains("is a directory")
    );
    assert!(
        err(
            &[],
            &a,
            CopyCase {
                replace: true,
                taken: Some(false),
                is_dir: true,
                ..case
            }
        )
        .contains("a directory does not replace")
    );
}

#[test]
fn texts_for_store_text() {
    assert_eq!(text_refusal("« { 1 } \"}\" @ } @ »"), None);
    assert_eq!(text_refusal("{ 1 } @ comment }\n"), None);
    assert_eq!(text_refusal("{\"a\"} \"a\"\"b\" (1,2) 'X' @c@ »"), None);
    assert_eq!(text_refusal("\"a@b\" 3 @ \" @ 2"), None);
    assert!(
        text_refusal("} 'P' PURGE {")
            .unwrap()
            .contains("closes a list")
    );
    // The calculator reads `@` and `"` inside a word as a new token.
    for t in ["X@ } 'P' PURGE {", "A\"B } 'P' PURGE {", "1@ 2", "A\"B\" C"] {
        assert!(
            text_refusal(t).unwrap().contains("right after a word"),
            "{t}"
        );
    }
    // An open string would swallow the wrapper's end.
    assert!(
        text_refusal("\"} 'P' PURGE {")
            .unwrap()
            .contains("string open")
    );
    assert!(text_refusal("« \"a »").unwrap().contains("string open"));
    assert_eq!(wrapped("« 1 »"), "{\n« 1 »\n}");
    assert_eq!(reply_count("1"), Some(1));
    assert_eq!(reply_count(" 1.000"), Some(1));
    assert_eq!(reply_count("0,"), Some(0));
    assert_eq!(reply_count("2."), Some(2));
    assert_eq!(reply_count("\"x\""), None);
    let v = |name: &str| Variable {
        name: name.into(),
        kind: "Real Number".into(),
        size: 10.5,
        checksum: 0,
        address: 0,
        variables: None,
    };
    assert_eq!(free_name(&[v("A")], TEXT_VARIABLE), TEXT_VARIABLE);
    assert_eq!(
        free_name(
            &[v(TEXT_VARIABLE), v(&format!("{TEXT_VARIABLE}2"))],
            TEXT_VARIABLE
        ),
        format!("{TEXT_VARIABLE}3")
    );
    assert_eq!(
        free_name(&[v(TEXT_VARIABLE), v(SCREEN_VARIABLE)], SCREEN_VARIABLE),
        format!("{SCREEN_VARIABLE}2")
    );
}

/// The compile's reply: the calculator's error, or the count of the
/// objects in the text; what it left on the stack is dropped first.
#[test]
fn a_compile_reports_the_calculators_refusal() {
    let m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
    let reply = |text: &str| Transcript {
        text: text.as_bytes().to_vec(),
        ..Transcript::default()
    };
    let compile = || Step::Compile {
        text: "'T' RCL 'T' PURGE STR→ DUP SIZE 2 MIN".into(),
    };
    let fresh = || {
        let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
        t.baseline = Some(1);
        t
    };
    let mut t = fresh();
    t.finished(compile(), reply("3: 42\r\n2: { « 1 » }\r\n1: 1.00\r\n"));
    assert_eq!(t.result.error, None);
    assert!(t.steps.is_empty());

    let mut t = fresh();
    t.finished(
        compile(),
        reply("Error: Invalid Syntax\r\n2: 42\r\n1: \"{ « }\"\r\n"),
    );
    assert_eq!(t.result.error.as_deref(), Some("Invalid Syntax"));
    assert_eq!(t.error, None, "the transfer itself went well");
    assert!(matches!(&t.steps[0], Step::Host { text, cleanup: true } if text == "1 DROPN"));
    // The place step is skipped, the cleanup is not.
    assert!(
        !Step::Host {
            text: "x".into(),
            cleanup: false
        }
        .cleanup()
    );

    let mut t = fresh();
    t.finished(compile(), reply("3: 42\r\n2: { 1 2 }\r\n1: 2\r\n"));
    assert_eq!(
        t.result.error.as_deref(),
        Some("the text holds more than one object")
    );
    assert!(matches!(&t.steps[0], Step::Host { text, .. } if text == "2 DROPN"));
    let mut t = fresh();
    t.finished(compile(), reply("3: 42\r\n2: { }\r\n1: 0\r\n"));
    assert_eq!(t.result.error.as_deref(), Some("the text holds no object"));
}

#[test]
fn a_directory_must_exist() {
    let tree = vec![Variable {
        name: "D".into(),
        kind: "Directory".into(),
        size: 10.0,
        checksum: 0,
        address: 0,
        variables: Some(vec![]),
    }];
    assert!(directory(&tree, &s(&["D"])).is_ok());
    let e = directory(&tree, &s(&["D", "E"])).unwrap_err();
    assert_eq!(e.to_string(), "no directory { HOME D E }");
}
