//! Replacing a variable with a long name on the emulated 48GX: the swap
//! of a replacing put does not fit in one host command. Gated by
//! `SATURNUS_ROM_DIR` (`gxrom-r`; the test policy).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stderr)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use saturnus_tx::{Action, Device, DeviceOptions, Format, Outcome, Request, TxError};
use satx_core::transport::{EmulatorModel, SaturnusTransport, Transport};

const OLD: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 1 \\>>\r\n";
const NEW: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 2 \\>>\r\n";

/// The device on an emulator, on the real clock, as a cable would have it.
struct Calculator {
    device: Device,
    transport: SaturnusTransport,
}

impl Calculator {
    fn boot() -> Option<Self> {
        let rom = std::env::var_os("SATURNUS_ROM_DIR").map(PathBuf::from)?;
        let rom = rom.join("gxrom-r");
        if !rom.is_file() {
            eprintln!("skipped: {} not found", rom.display());
            return None;
        }
        let model = EmulatorModel::from_name("48gx").unwrap();
        let mut calculator = Self {
            device: Device::new(Instant::now(), DeviceOptions::default(), 1),
            transport: SaturnusTransport::open(model, &rom).unwrap(),
        };
        let connected = calculator.run(Request::Connect(Default::default()));
        assert!(
            matches!(connected, Ok(Outcome::Connected(_))),
            "{connected:?}"
        );
        Some(calculator)
    }

    fn run(&mut self, request: Request) -> Result<Outcome, TxError> {
        assert!(self.device.start(Instant::now(), request).is_ok());
        let mut buf = [0u8; 2048];
        loop {
            while let Some(action) = self.device.poll_action() {
                if let Action::Write(bytes) = action
                    && let Err(e) = self.transport.write_packet(&bytes)
                {
                    self.device.handle_link_error(Instant::now(), &e);
                }
            }
            while self.device.poll_event().is_some() {}
            if let Some(result) = self.device.poll_result() {
                return result;
            }
            let wait = self
                .device
                .next_timeout()
                .map_or(Duration::from_millis(100), |d| {
                    d.saturating_duration_since(Instant::now())
                });
            match self
                .transport
                .read(&mut buf, wait.max(Duration::from_millis(1)))
            {
                Ok(0) => self.device.handle_timeout(Instant::now()),
                Ok(n) => self.device.handle_input(Instant::now(), &buf[..n]),
                Err(e) => self.device.handle_link_error(Instant::now(), &e),
            }
        }
    }

    fn put(&mut self, name: &str, data: &[u8], replace: bool) -> Result<Outcome, TxError> {
        self.run(Request::Put {
            dir: vec!["HOME".into()],
            name: name.into(),
            data: data.to_vec(),
            replace,
            as_string: false,
        })
    }

    fn text(&mut self, name: &str) -> Vec<u8> {
        let got = self.run(Request::Get {
            dir: vec!["HOME".into()],
            name: name.into(),
            format: Format::Text,
        });
        match got {
            Ok(Outcome::File(file)) => file.data,
            other => panic!("get {name}: {other:?}"),
        }
    }

    /// The free memory, in bytes.
    fn memory(&mut self) -> f64 {
        match self.run(Request::Memory) {
            Ok(Outcome::Memory(m)) => m.free,
            other => panic!("memory: {other:?}"),
        }
    }

    fn names(&mut self) -> Vec<String> {
        match self.run(Request::List { dir: vec![] }) {
            Ok(Outcome::Listing(l)) => l.entries.into_iter().map(|e| e.name).collect(),
            other => panic!("list: {other:?}"),
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A name too long for the one command goes in its three parts: the
/// variable holds the new object, and the temporary name is gone.
#[test]
fn a_long_name_is_swapped_in_three_commands() {
    let Some(mut calc) = Calculator::boot() else {
        return;
    };
    let name = "A".repeat(50);
    assert!(matches!(
        calc.put(&name, OLD, false),
        Ok(Outcome::Stored(_))
    ));
    let replaced = calc.put(&name, NEW, true);
    assert!(
        matches!(&replaced, Ok(Outcome::Stored(n)) if *n == name),
        "{replaced:?}"
    );
    assert!(contains(&calc.text(&name), b" 2\r\n"), "the new object");
    let names = calc.names();
    assert!(names.contains(&name), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("SATXPUT")), "{names:?}");
}

/// A name too long for `STO` alone is refused unsent: the old variable
/// stays, and neither the temporary name nor the recalled copy does.
#[test]
fn a_name_too_long_for_its_store_keeps_the_old_variable() {
    let Some(mut calc) = Calculator::boot() else {
        return;
    };
    let name = "B".repeat(72);
    assert!(matches!(
        calc.put(&name, OLD, false),
        Ok(Outcome::Stored(_))
    ));
    let free = calc.memory();
    let replaced = calc.put(&name, NEW, true);
    assert!(
        matches!(
            replaced,
            Err(TxError::Calc(satx_core::Error::CommandTooLong { .. }))
        ),
        "{replaced:?}"
    );
    assert!(contains(&calc.text(&name), b" 1\r\n"), "the old object");
    let names = calc.names();
    assert!(!names.iter().any(|n| n.starts_with("SATXPUT")), "{names:?}");
    assert_eq!(calc.memory(), free, "nothing stays on the stack");
}
