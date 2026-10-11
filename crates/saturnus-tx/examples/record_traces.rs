//! Records the trace tests' traces from saturnus in-process (`just
//! tx-traces DIR`): the session and the ops session of `tests/scenario` on
//! the emulated 48SX, 48GX and 49G, booted from the ROMs in DIR, into
//! `crates/saturnus-tx/traces/MODEL-session.trace` and `MODEL-ops.trace`.
//! The ops session restores its backup onto a second, freshly booted
//! calculator and types `SERVER` on it again.
//!
//! Then a copy between two devices, not recorded: a program put to an
//! emulated 48GX, got, and put to an emulated 49G after the plan's warning
//! (a 48 object going to a 49G).
//!
//! The device runs on the real clock against `satx-core`'s saturnus
//! transport, as it would against a cable. A trace holds the serial bytes
//! only; before writing one the recorder checks that no 16-byte run of the
//! data it carries (decoded from the Kermit packets) occurs in the ROM
//! image, the server's text replies aside.
//!
//! Usage: `record-traces ROM_DIR [OUT_DIR [WHAT...]]`, WHAT `48sx`, `48gx`,
//! `49g` or `copy` (default: all).

#![allow(clippy::print_stdout, clippy::print_stderr)]

#[path = "../tests/scenario/mod.rs"]
mod scenario;

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kermit_proto::codec::{BlockCheck, Deframer, parse_frame};
use kermit_proto::prefix::{self, Quoting};
use saturnus_drive::autostart::autostart_script;
use saturnus_drive::session::Session;
use saturnus_tx::copy;
use saturnus_tx::trace::{Step, Trace};
use saturnus_tx::{Action, Device, DeviceOptions, Format, Outcome, Request, TxError};
use satx_core::transport::{EmulatorModel, SaturnusTransport, Transport};

type Error = Box<dyn std::error::Error>;

/// Run one request on the real clock, logging the link into `trace`.
fn drive(
    device: &mut Device,
    transport: &mut dyn Transport,
    trace: &mut Trace,
    start: Instant,
    request: Request,
) -> Result<Outcome, TxError> {
    if device.start(Instant::now(), request).is_err() {
        return Err(TxError::Busy);
    }
    let mut buf = [0u8; 2048];
    loop {
        while let Some(action) = device.poll_action() {
            let at = start.elapsed();
            match action {
                Action::Open(settings) => trace.push(at, Step::Open(settings)),
                Action::Write(bytes) => {
                    trace.push(at, Step::Out(bytes.clone()));
                    if let Err(e) = transport.write_packet(&bytes) {
                        device.handle_link_error(Instant::now(), &e);
                    }
                }
            }
        }
        while device.poll_event().is_some() {}
        if let Some(result) = device.poll_result() {
            return result;
        }
        let wait = device
            .next_timeout()
            .map_or(Duration::from_millis(100), |d| {
                d.saturating_duration_since(Instant::now())
            });
        match transport.read(&mut buf, wait.max(Duration::from_millis(1))) {
            Ok(0) => device.handle_timeout(Instant::now()),
            Ok(n) => {
                let bytes = &buf[..n];
                trace.push(start.elapsed(), Step::In(bytes.to_vec()));
                device.handle_input(Instant::now(), bytes);
            }
            Err(e) => device.handle_link_error(Instant::now(), &e),
        }
    }
}

/// What one direction of a trace carried: each Kermit transfer's data,
/// decoded from its D packets, and whether it was a file (`F` header) or
/// server text (`X`, a host command's reply or a listing).
fn transfers(trace: &Trace, out: bool) -> Vec<(bool, Vec<u8>)> {
    let mut deframer = Deframer::new();
    for (_, step) in &trace.steps {
        match step {
            Step::Out(bytes) if out => deframer.push(bytes),
            Step::In(bytes) if !out => deframer.push(bytes),
            _ => {}
        }
    }
    let mut found = Vec::new();
    let mut current: Option<(bool, Vec<u8>)> = None;
    while let Some(frame) = deframer.next_frame() {
        let Some(packet) = [BlockCheck::Type1, BlockCheck::Type2, BlockCheck::Type3]
            .into_iter()
            .find_map(|check| parse_frame(&frame, check).ok())
        else {
            continue;
        };
        match packet.kind {
            b'F' => current = Some((false, Vec::new())),
            b'X' => current = Some((true, Vec::new())),
            b'D' => {
                if let (Some((_, data)), Ok(decoded)) = (
                    current.as_mut(),
                    prefix::decode(&packet.data, &Quoting::default()),
                ) {
                    data.extend(decoded);
                }
            }
            b'Z' | b'B' | b'E' => found.extend(current.take()),
            _ => {}
        }
    }
    found.extend(current);
    found
}

/// Fail when a 16-byte run of what the trace transferred (either
/// direction, decoded from the Kermit packets) is in the ROM, except text
/// the server replies with (printable ASCII): the 49G's VERSION line,
/// `Copyright HP 2000`, is a ROM string.
fn check_no_rom(trace: &Trace, rom: &[u8]) -> Result<(), String> {
    const RUN: usize = 16;
    let windows: HashSet<&[u8]> = rom.windows(RUN).collect();
    let printable = |w: &[u8]| w.iter().all(|b| (0x20..0x7f).contains(b));
    for out in [true, false] {
        for (n, (text, data)) in transfers(trace, out).into_iter().enumerate() {
            if let Some((at, w)) = data
                .windows(RUN)
                .enumerate()
                .find(|(_, w)| !(text && printable(w)) && windows.contains(w))
            {
                let way = if out { "sent" } else { "received" };
                return Err(format!(
                    "the trace holds ROM bytes: {w:02x?} at {at} of the {way} transfer {n} \
                     ({} bytes, starting {:02x?})",
                    data.len(),
                    &data[..data.len().min(16)]
                ));
            }
        }
    }
    Ok(())
}

/// The ops session's calculator: a device on an emulator, recorded.
struct Recorder<'a> {
    model: &'a str,
    rom: &'a Path,
    transport: SaturnusTransport,
    device: Device,
    trace: Trace,
    start: Instant,
}

fn boot(model: &str, rom: &Path) -> Result<SaturnusTransport, Error> {
    let emulated = EmulatorModel::from_name(model).ok_or("unknown model")?;
    eprintln!("{model}: booting {}", rom.display());
    Ok(SaturnusTransport::open(emulated, rom)?)
}

impl scenario::Env for Recorder<'_> {
    fn run(&mut self, label: &str, request: Request) -> Result<Outcome, TxError> {
        eprintln!("{}: {label}", self.model);
        self.trace.note(&format!("request {label}"));
        drive(
            &mut self.device,
            &mut self.transport,
            &mut self.trace,
            self.start,
            request,
        )
    }

    fn fresh(&mut self) {
        match boot(self.model, self.rom) {
            Ok(t) => self.transport = t,
            Err(e) => panic!("{}: {e}", self.model),
        }
        self.device = Device::new(Instant::now(), DeviceOptions::default(), scenario::SEED);
        self.trace.note("fresh calculator");
    }

    fn server(&mut self) {
        let machine = self.transport.machine().clone();
        let script = match autostart_script(machine.model(), false) {
            Ok(s) => s,
            Err(e) => panic!("{}: {e:#}", self.model),
        };
        let mut session = Session::new(machine, 0, false);
        session.set_echo_warnings(false);
        // The warm start's own screen first.
        if let Err(e) = session.wait_idle(10_000, 0) {
            panic!("{}: {e:#}", self.model);
        }
        for line in &script {
            if let Err(e) = session.apply(line) {
                panic!("{}: {e:#}", self.model);
            }
        }
        *self.transport.machine() = session.machine;
        self.trace.note("SERVER typed on the calculator");
    }
}

/// Write `trace` as `MODEL-KIND.trace` after the ROM check.
fn save(model: &str, kind: &str, rom: &Path, trace: &Trace, out: &Path) -> Result<(), Error> {
    let image = std::fs::read(rom)?;
    check_no_rom(trace, &image).map_err(|e| format!("{model} {kind}: {e}"))?;
    let checked: Vec<usize> = [true, false]
        .into_iter()
        .map(|out| transfers(trace, out).iter().map(|(_, d)| d.len()).sum())
        .collect();
    eprintln!(
        "{model} {kind}: no ROM run in {} bytes sent, {} bytes received (decoded)",
        checked[0], checked[1]
    );
    let name = rom
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let what = match kind {
        "session" => "the session",
        _ => "the ops session",
    };
    let text = format!(
        "# saturnus tx trace: {what} of crates/saturnus-tx/tests/scenario on\n\
         # the emulated {} (ROM file {name}), recorded by `just tx-traces`.\n\
         # Serial bytes only; times in ms since the start.\n{trace}",
        model.to_uppercase()
    );
    let path = out.join(format!("{model}-{kind}.trace"));
    std::fs::write(&path, text)?;
    eprintln!(
        "{model} {kind}: {} steps, {}",
        trace.steps.len(),
        path.display()
    );
    Ok(())
}

fn record(model: &str, rom: &Path, out: &Path) -> Result<(), Error> {
    let mut transport = boot(model, rom)?;
    let start = Instant::now();
    let mut device = Device::new(start, DeviceOptions::default(), scenario::SEED);
    let mut trace = Trace::new();
    let mut driver = |label: &str, request: Request| {
        eprintln!("{model}: {label}");
        trace.note(&format!("request {label}"));
        drive(&mut device, &mut transport, &mut trace, start, request)
    };
    let run = scenario::run(model, &mut driver);
    if let Err(e) = run {
        eprint!("{trace}");
        return Err(format!("{model}: {e}").into());
    }
    save(model, "session", rom, &trace, out)?;

    let start = Instant::now();
    let mut env = Recorder {
        model,
        rom,
        transport: boot(model, rom)?,
        device: Device::new(start, DeviceOptions::default(), scenario::SEED),
        trace: Trace::new(),
        start,
    };
    if let Err(e) = scenario::ops(model, &mut env) {
        eprint!("{}", env.trace);
        return Err(format!("{model} ops: {e}").into());
    }
    eprintln!("{model} ops: {:.1?}", start.elapsed());
    if model == "49g" {
        // Every backup of a 49G ends in a list of ROM objects (a 20-byte
        // run of the ROM, 2026-10-11): the backup ran and was restored
        // above, but the trace keeps the ops before it.
        let mut kept = env.trace.before_note(scenario::BACKUP_PART);
        kept.note("the backup part is not recorded on the 49G: its backup holds ROM bytes");
        return save(model, "ops", rom, &kept, out);
    }
    save(model, "ops", rom, &env.trace, out)
}

/// A program put to an emulated 48GX, got, and copied to an emulated 49G
/// by the copy plan, which warns first. Prints what it did.
fn copy_48gx_to_49g(roms: &Path) -> Result<(), Error> {
    let start = Instant::now();
    let mut gx = boot("48gx", &roms.join("gxrom-r"))?;
    let mut g49 = boot("49g", &roms.join("rom.49g"))?;
    let mut scratch = Trace::new();
    let mut a = Device::new(start, DeviceOptions::default(), 1);
    let mut b = Device::new(start, DeviceOptions::default(), 2);
    let mut on_a = |r: Request| drive(&mut a, &mut gx, &mut scratch, start, r);
    let home = vec!["HOME".to_string()];
    let connected = |o: Result<Outcome, TxError>| match o {
        Ok(Outcome::Connected(c)) => Ok(c.model),
        other => Err(format!("connect: {other:?}")),
    };
    let from = connected(on_a(Request::Connect(Default::default())))?;
    on_a(Request::Put {
        dir: home.clone(),
        name: "PRG".into(),
        data: scenario::PROGRAM.to_vec(),
        replace: false,
        as_string: false,
    })?;
    let Outcome::Listing(source) = on_a(Request::List { dir: home.clone() })? else {
        return Err("48GX: no listing".into());
    };
    let mut scratch_b = Trace::new();
    let mut on_b = |r: Request| drive(&mut b, &mut g49, &mut scratch_b, start, r);
    let to = connected(on_b(Request::Connect(Default::default())))?;
    let Outcome::Listing(target) = on_b(Request::List { dir: home.clone() })? else {
        return Err("49G: no listing".into());
    };
    let selection: Vec<_> = source
        .entries
        .iter()
        .filter(|e| e.name == "PRG")
        .cloned()
        .collect();
    let plan = copy::plan(Some(from), to, &selection, &target.entries);
    let warning = plan
        .mismatch
        .ok_or("the plan does not warn about a 48 object on a 49G")?;
    eprintln!("copy 48GX -> 49G: the plan warns: {}", warning.message());
    let step = &plan.steps[0];
    let got = on_a(step.get(&home))?;
    let Outcome::File(file) = &got else {
        return Err(format!("48GX get: {got:?}").into());
    };
    if file.format != Format::Text || !file.data.starts_with(b"%%HP:") {
        return Err(format!("the 48GX's PRG did not come as text: {file:?}").into());
    }
    // The binary form would have been a string on the 49G.
    let Outcome::File(binary) = on_a(Request::Get {
        dir: home.clone(),
        name: "PRG".into(),
        format: Format::Binary,
    })?
    else {
        return Err("48GX: no binary PRG".into());
    };
    let Some(file_warning) = copy::header_check(&binary.data, to) else {
        return Err("the binary file's header does not tell the 48 family".into());
    };
    eprintln!(
        "copy 48GX -> 49G: a binary 48 file: {}",
        file_warning.file_message()
    );
    let stored = on_b(step.put(&home, got.clone())?)?;
    let Outcome::Listing(after) = on_b(Request::List { dir: home.clone() })? else {
        return Err("49G: no listing after".into());
    };
    let checksum = |l: &[saturnus_tx::Entry]| {
        l.iter()
            .find(|e| e.name == "PRG")
            .map(|e| (e.kind.clone(), e.checksum))
    };
    eprintln!(
        "copy 48GX -> 49G: {stored:?}; PRG on the 48GX {:?}, on the 49G {:?}",
        checksum(&source.entries),
        checksum(&after.entries)
    );
    if checksum(&after.entries).map(|(kind, _)| kind).as_deref() != Some("Program") {
        return Err("PRG is not a program on the 49G".into());
    }
    on_a(Request::Disconnect)?;
    on_b(Request::Disconnect)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1);
    let Some(roms) = args.next().map(PathBuf::from) else {
        return Err(io::Error::other(
            "usage: record-traces ROM_DIR [OUT_DIR [48sx|48gx|49g|copy...]]",
        )
        .into());
    };
    let out = args.next().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("traces"),
        PathBuf::from,
    );
    // The rest: what to do (default all), models and `copy`.
    let only: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();
    let wanted = |what: &str| only.is_empty() || only.iter().any(|o| o == what);
    for (model, rom) in scenario::MODELS {
        if wanted(model) {
            record(model, &roms.join(rom), &out)?;
        }
    }
    if wanted("copy") {
        copy_48gx_to_49g(&roms)?;
    }
    Ok(())
}
