//! Records the aplet server's traces from saturnus in-process (`just
//! tx-aplet-traces DIR`): the emulated 38G and 39G, booted from the ROMs in
//! DIR (`38G_A167.ROM`, `rom.39g`), as the clients of
//! [`saturnus_tx::aplets::ApletServer`] serving a folder in memory.
//!
//! On each calculator: list L1 is filled with twelve four-digit numbers, then
//! SEND, "Disk drive", "Send now" (the directory files asked for, L1 and the
//! calculator's new `HP38DIR.000` received), then RECEIVE, "Disk drive", L1
//! (served back). On the 39G also RECEIVE, "HP39/40 (Wire)" with L1 sent by
//! XModem: a list is no binary aplet, so the calculator ends with "Transfer
//! Failed" after taking every block. Lists rather than aplets, because a
//! new aplet is a copy of one in ROM and the traces must hold no ROM bytes
//! (checked before writing, against the ROM file and its packed image).
//!
//! The clock is the emulated one, so a recording is deterministic. Writes
//! `crates/saturnus-tx/traces/MODEL-aplets.trace` (with times) and
//! `crates/kermit-proto/traces/MODEL-serve.trace` (the Kermit part, no
//! times).
//!
//! Usage: `record-aplet-traces ROM_DIR [REPO_DIR [MODEL...]]`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use kermit_proto::codec::{BlockCheck, Deframer, parse_frame};
use kermit_proto::prefix::{self, Quoting};
use kermit_proto::trace::escape;
use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_drive::session::Session;
use saturnus_tx::aplets::{ApletEvent, ApletOptions, ApletServer, FolderFile, HEAD_LEN};
use saturnus_tx::trace::{Step, Trace};
use saturnus_tx::{LinkSettings, Parity};
use satx_core::time::{Duration, Instant};

type Error = Box<dyn std::error::Error>;

/// List L1's numbers.
const NUMBERS: [u32; 12] = [
    2718, 3141, 1414, 1732, 6022, 1602, 9109, 6626, 2997, 1380, 8314, 6674,
];

/// Emulated time per step of the loop.
const STEP_MS: u64 = 5;

struct Rig {
    session: Session,
    server: ApletServer,
    /// The folder: name to bytes.
    folder: BTreeMap<String, Vec<u8>>,
    trace: Trace,
    base: Instant,
    ms: u64,
    cycles_per_ms: u64,
    /// When the last transfer event happened (emulated ms).
    last_event: u64,
    wire_end: Option<ApletEvent>,
}

impl Rig {
    fn machine(&mut self) -> &mut Machine {
        &mut self.session.machine
    }

    fn now(&self) -> Instant {
        self.base + Duration::from_millis(self.ms)
    }

    fn files(&self) -> Vec<FolderFile> {
        self.folder
            .iter()
            .map(|(name, data)| FolderFile {
                name: name.clone(),
                head: if saturnus_tx::aplets::directory_file(name).is_some() {
                    data.clone()
                } else {
                    data[..data.len().min(HEAD_LEN)].to_vec()
                },
            })
            .collect()
    }

    /// Run `ms` of emulated time, serving the link.
    fn pump(&mut self, ms: u64) -> Result<(), Error> {
        let end = self.ms + ms;
        while self.ms < end {
            let n = STEP_MS * self.cycles_per_ms;
            self.machine()
                .run_cycles(n)
                .map_err(|h| format!("halt: {h:?}"))?;
            self.ms += STEP_MS;
            let now = self.now();
            let input = self.machine().serial_drain();
            if !input.is_empty() {
                self.trace
                    .push(Duration::from_millis(self.ms), Step::In(input.clone()));
                self.server.handle_input(now, &input);
            }
            if self.server.next_timeout().is_some_and(|t| t <= now) {
                self.server.handle_timeout(now);
            }
            self.events(now)?;
            while let Some(out) = self.server.poll_output(now) {
                self.trace
                    .push(Duration::from_millis(self.ms), Step::Out(out.clone()));
                self.machine().serial_push(&out);
            }
        }
        Ok(())
    }

    fn events(&mut self, now: Instant) -> Result<(), Error> {
        while let Some(e) = self.server.poll_event() {
            self.last_event = self.ms;
            match &e {
                ApletEvent::NeedFile { name } => {
                    let data = self.folder.get(name).cloned();
                    eprintln!("  serves {name}");
                    self.server.provide(now, name, data)?;
                }
                ApletEvent::Received { name, data, .. } => {
                    eprintln!("  received {name} ({} bytes)", data.len());
                    self.folder.insert(name.clone(), data.clone());
                }
                ApletEvent::Sent { name, .. } => eprintln!("  sent {name}"),
                ApletEvent::WireDone | ApletEvent::WireFailed(_) => {
                    eprintln!("  wire: {e:?}");
                    self.wire_end = Some(e.clone());
                }
                ApletEvent::Sending { .. }
                | ApletEvent::Receiving { .. }
                | ApletEvent::WireProgress { .. } => {}
                other => eprintln!("  {other:?}"),
            }
        }
        Ok(())
    }

    /// Press and release `key`, serving the link meanwhile, then run on.
    fn press(&mut self, key: &str, after_ms: u64) -> Result<(), Error> {
        let k = Key::from_name(key).ok_or_else(|| format!("no key {key}"))?;
        self.machine().key_down(k)?;
        self.pump(80)?;
        self.machine().key_up(k)?;
        self.pump(after_ms)
    }

    fn keys(&mut self, keys: &[&str]) -> Result<(), Error> {
        for k in keys {
            self.press(k, 400)?;
        }
        Ok(())
    }

    /// Run until no transfer event came for `quiet_ms` and the server is
    /// idle, at most `cap_ms`.
    fn settle(&mut self, quiet_ms: u64, cap_ms: u64) -> Result<(), Error> {
        let start = self.ms;
        self.last_event = self.ms;
        while self.ms - start < cap_ms {
            self.pump(100)?;
            if !self.server.is_busy() && self.ms - self.last_event >= quiet_ms {
                return Ok(());
            }
        }
        Err(format!("no end within {} s:\n{}", cap_ms / 1000, self.screen()).into())
    }

    fn screen(&self) -> String {
        self.session
            .machine
            .lcd()
            .pixels
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&p| if p { '#' } else { '.' })
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    }
}

/// Every Kermit transfer's data in one direction, decoded.
fn transfers(trace: &Trace, out: bool) -> Vec<Vec<u8>> {
    let mut deframer = Deframer::new();
    for (_, step) in &trace.steps {
        match step {
            Step::Out(b) if out => deframer.push(b),
            Step::In(b) if !out => deframer.push(b),
            _ => {}
        }
    }
    let mut found = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    while let Some(frame) = deframer.next_frame() {
        let Some(p) = [BlockCheck::Type1, BlockCheck::Type3]
            .into_iter()
            .find_map(|c| parse_frame(&frame, c).ok())
        else {
            continue;
        };
        match p.kind {
            b'F' => current = Some(Vec::new()),
            b'D' => {
                if let (Some(d), Ok(x)) = (
                    current.as_mut(),
                    prefix::decode(&p.data, &Quoting::default()),
                ) {
                    d.extend(x);
                }
            }
            b'Z' | b'B' | b'E' => found.extend(current.take()),
            _ => {}
        }
    }
    found
}

/// Fail when a 16-byte run of a transferred file (or of the XModem bytes)
/// is in the ROM file or in its packed image, runs of one or two byte
/// values aside.
fn check_no_rom(trace: &Trace, images: &[&[u8]]) -> Result<(), Error> {
    const RUN: usize = 16;
    let windows: HashSet<&[u8]> = images.iter().flat_map(|i| i.windows(RUN)).collect();
    let mut data: Vec<Vec<u8>> = transfers(trace, true);
    data.extend(transfers(trace, false));
    data.push(
        trace
            .steps
            .iter()
            .filter_map(|(_, s)| match s {
                Step::Out(b) if b.first() == Some(&1) && b.len() == 132 => Some(b.clone()),
                _ => None,
            })
            .flatten()
            .collect(),
    );
    for d in data {
        // Runs of one or two byte values (the XModem padding) say nothing.
        let varied = |w: &[u8]| w.iter().collect::<HashSet<_>>().len() > 2;
        if let Some(w) = d.windows(RUN).find(|w| varied(w) && windows.contains(w)) {
            return Err(format!("the trace holds ROM bytes: {w:02x?}").into());
        }
    }
    Ok(())
}

fn record(model: &str, rom_dir: &Path, repo: &Path) -> Result<(), Error> {
    let (m, file, extra_down) = match model {
        "38g" => (Model::Hp38g, "38G_A167.ROM", false),
        "39g" => (Model::Hp39g, "rom.39g", true),
        _ => return Err(format!("no model {model}").into()),
    };
    let rom_path = rom_dir.join(file);
    let raw = std::fs::read(&rom_path)?;
    let image = saturnus_drive::rom::load(m, &rom_path).map_err(|e| format!("{e:#}"))?;
    let machine = Machine::new(m, &image)?;
    let cycles_per_ms = u64::from(machine.model().clock_hz()) / 1000;
    let mut session = Session::new(machine, 0, false);
    session.set_echo_warnings(false);
    eprintln!("{model}: booting {}", rom_path.display());
    session.wait_idle(60_000, 0)?;
    let mut rig = Rig {
        session,
        server: ApletServer::new(ApletOptions::default(), Vec::new()),
        folder: BTreeMap::new(),
        trace: Trace::new(),
        base: Instant::now(),
        ms: 0,
        cycles_per_ms,
        last_event: 0,
        wire_end: None,
    };
    // The host's port: 9600 baud, no parity, as the page opens it.
    rig.trace.push(
        Duration::ZERO,
        Step::Open(LinkSettings {
            baud: 9600,
            parity: Parity::None,
        }),
    );
    // "Memory Clear": OK.
    rig.press("f", 1500)?;
    // L1: twelve numbers no ROM holds as a pair (small integers next to
    // each other do: the ROM's own constants).
    rig.keys(&["shift", "7", "a"])?;
    for n in NUMBERS {
        for d in n.to_string().chars() {
            rig.press(&d.to_string(), 200)?;
        }
        rig.press("enter", 300)?;
    }
    rig.keys(&["shift", "7"])?;
    let disk: &[&str] = if extra_down {
        &["down", "down"]
    } else {
        &["down"]
    };

    rig.trace.note("send L1: SEND, Disk drive, Send now");
    eprintln!("{model}: send L1");
    rig.keys(&["d"])?;
    rig.keys(disk)?;
    rig.keys(&["f", "f"])?;
    rig.settle(3000, 60_000)?;
    let l1 = rig
        .folder
        .keys()
        .find(|n| n.starts_with("L1"))
        .cloned()
        .ok_or("L1 did not arrive")?;

    rig.server.set_files(rig.files());
    rig.trace.note("receive L1: RECEIVE, Disk drive, L1");
    eprintln!("{model}: receive {l1}");
    rig.keys(&["e"])?;
    rig.keys(disk)?;
    rig.keys(&["f"])?;
    rig.settle(1500, 30_000)?;
    rig.keys(&["f"])?;
    rig.settle(3000, 60_000)?;

    if model == "39g" {
        rig.trace
            .note("wire: L1 by XModem, RECEIVE, HP39/40 (Wire)");
        eprintln!("{model}: wire {l1}");
        let data = rig.folder.get(&l1).cloned().ok_or("no L1")?;
        let now = rig.now();
        rig.server.send_wire(now, data)?;
        rig.keys(&["e", "down", "f"])?;
        rig.settle(3000, 60_000)?;
        if rig.wire_end != Some(ApletEvent::WireDone) {
            return Err(format!("the wire send ended with {:?}", rig.wire_end).into());
        }
    }
    check_no_rom(&rig.trace, &[&raw, &image])?;

    let header = format!(
        "# saturnus tx trace: the aplet server and the emulated {} (ROM file {file}),\n\
         # recorded by `just tx-aplet-traces`: list L1 sent to the folder and\n\
         # received back{}. Serial bytes only; times in emulated ms.\n",
        model.to_uppercase(),
        if model == "39g" {
            ", then sent by XModem"
        } else {
            ""
        }
    );
    let tx = repo.join(format!("crates/saturnus-tx/traces/{model}-aplets.trace"));
    std::fs::write(&tx, format!("{header}{}", rig.trace))?;
    let mut kermit = format!(
        "# kermit-proto server trace: the emulated {} (ROM file {file}) as the\n\
         # client, recorded with saturnus-tx's aplet server (`just tx-aplet-traces`):\n\
         # list L1 sent, then received back. Serial bytes only.\n",
        model.to_uppercase()
    );
    for (_, step) in &rig.trace.before_note("wire").steps {
        match step {
            Step::Note(n) => kermit += &format!("# {n}\n"),
            Step::Out(b) => kermit += &format!("> {}\n", escape(b)),
            Step::In(b) => kermit += &format!("< {}\n", escape(b)),
            Step::Open(_) => {}
        }
    }
    let kp = repo.join(format!("crates/kermit-proto/traces/{model}-serve.trace"));
    std::fs::write(&kp, kermit)?;
    eprintln!("{model}: wrote {} and {}", tx.display(), kp.display());
    Ok(())
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rom_dir = PathBuf::from(
        args.first()
            .ok_or("usage: record-aplet-traces ROM_DIR [REPO_DIR [MODEL...]]")?,
    );
    let repo = args
        .get(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let models: Vec<&str> = if args.len() > 2 {
        args[2..].iter().map(String::as_str).collect()
    } else {
        vec!["38g", "39g"]
    };
    for m in models {
        record(m, &rom_dir, &repo)?;
    }
    Ok(())
}
