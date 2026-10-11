//! Records the trace tests' traces from saturnus in-process (`just
//! tx-traces DIR`): the session of `tests/scenario` on the emulated 48SX,
//! 48GX and 49G, booted from the ROMs in DIR, into
//! `crates/saturnus-tx/traces/MODEL-session.trace`.
//!
//! The device runs on the real clock against `satx-core`'s saturnus
//! transport, as it would against a cable. A trace holds the serial bytes
//! only; before writing one the recorder checks that no 16-byte run of it
//! occurs in the ROM image (text the calculator shows aside).
//!
//! Usage: `record-traces ROM_DIR [OUT_DIR]`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

#[path = "../tests/scenario/mod.rs"]
mod scenario;

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use saturnus_tx::trace::{Step, Trace};
use saturnus_tx::{Action, Device, DeviceOptions, Outcome, Request, TxError};
use satx_core::transport::{EmulatorModel, Transport, open_saturnus};

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

/// Fail when a 16-byte run of the trace (either direction) is in the ROM,
/// except text the calculator shows (printable ASCII): the 49G's VERSION
/// line, `Copyright HP 2000`, is a ROM string.
fn check_no_rom(trace: &Trace, rom: &[u8]) -> Result<(), String> {
    const RUN: usize = 16;
    let windows: HashSet<&[u8]> = rom.windows(RUN).collect();
    let text = |w: &[u8]| w.iter().all(|b| (0x20..0x7f).contains(b));
    for (_, step) in &trace.steps {
        if let Step::In(bytes) | Step::Out(bytes) = step
            && let Some(w) = bytes.windows(RUN).find(|w| !text(w) && windows.contains(w))
        {
            return Err(format!("the trace holds ROM bytes: {w:02x?}"));
        }
    }
    Ok(())
}

fn record(model: &str, rom: &Path, out: &Path) -> Result<(), Error> {
    let emulated = EmulatorModel::from_name(model).ok_or("unknown model")?;
    eprintln!("{model}: booting {}", rom.display());
    let mut transport = open_saturnus(emulated, rom)?;
    let start = Instant::now();
    let mut options = DeviceOptions::default();
    if let Some(ms) = std::env::var("TX_LISTEN_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        options.listen = Duration::from_millis(ms);
    }
    let mut device = Device::new(start, options, scenario::SEED);
    let mut trace = Trace::new();
    let mut driver = |label: &str, request: Request| {
        eprintln!("{model}: {label}");
        trace.note(&format!("request {label}"));
        drive(&mut device, transport.as_mut(), &mut trace, start, request)
    };
    let run = scenario::run(model, &mut driver);
    if let Err(e) = run {
        eprint!("{trace}");
        return Err(format!("{model}: {e}").into());
    }
    let image = std::fs::read(rom)?;
    check_no_rom(&trace, &image).map_err(|e| format!("{model}: {e}"))?;
    let name = rom
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = format!(
        "# saturnus tx trace: the session of crates/saturnus-tx/tests/scenario on\n\
         # the emulated {} (ROM file {name}), recorded by `just tx-traces`.\n\
         # Serial bytes only; times in ms since the start.\n{trace}",
        model.to_uppercase()
    );
    let path = out.join(format!("{model}-session.trace"));
    std::fs::write(&path, text)?;
    eprintln!(
        "{model}: {} steps in {:.1?}, {}",
        trace.steps.len(),
        start.elapsed(),
        path.display()
    );
    Ok(())
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args_os().skip(1);
    let Some(roms) = args.next().map(PathBuf::from) else {
        return Err(io::Error::other("usage: record-traces ROM_DIR [OUT_DIR]").into());
    };
    let out = args.next().map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("traces"),
        PathBuf::from,
    );
    for (model, rom) in scenario::MODELS {
        record(model, &roms.join(rom), &out)?;
    }
    Ok(())
}
