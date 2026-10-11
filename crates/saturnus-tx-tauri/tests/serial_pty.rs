//! The app's serial code ([`saturnus_tx_tauri::serial`]) against a
//! calculator on a pseudo-terminal pair (`serialport`'s `TTYPort::pair`):
//! the emulated 48GX in-process behind the master end (satx-core's
//! `SaturnusTransport`, its server started), the app's `Ports` on the slave
//! end opened by its name as a USB adapter is (on macOS, whose ptys
//! refuse the speed's ioctl, the open end handed over), and satx-core's Kermit
//! client over that link: a put, the listing, the get back byte for byte.
//! The page's own protocol (the wasm) is the same satx-core.
//!
//! ROM-gated: skipped unless `SATURNUS_ROM_DIR` holds `gxrom-r`; Linux and
//! macOS only (pseudo-terminals).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use saturnus_tx_tauri::serial::{Event, Ports, Settings};
use satx_core::transport::{EmulatorModel, SaturnusTransport, Transport};
use satx_core::{Calculator, Options, Session, TransferMode};
use serialport::{SerialPort, TTYPort};

fn rom() -> Option<PathBuf> {
    let rom = PathBuf::from(std::env::var_os("SATURNUS_ROM_DIR")?).join("gxrom-r");
    rom.is_file().then_some(rom)
}

/// satx-core's link over the app's open port: writes through `Ports`,
/// reads from what its reader handed the sink.
struct AppLink {
    ports: Arc<Ports>,
    id: u32,
    rx: Receiver<Event>,
    pending: VecDeque<u8>,
}

impl Transport for AppLink {
    fn write_packet(&mut self, packet: &[u8]) -> io::Result<()> {
        self.ports.write(self.id, packet).map_err(io::Error::other)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.rx.recv_timeout(timeout) {
                Ok(Event::Data(bytes)) => self.pending.extend(bytes),
                Ok(Event::Lost(why)) => {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, why));
                }
                Err(RecvTimeoutError::Timeout) => return Ok(0),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the reader ended",
                    ));
                }
            }
        }
        // Whatever else arrived meanwhile, without waiting.
        while let Ok(Event::Data(bytes)) = self.rx.try_recv() {
            self.pending.extend(bytes);
        }
        let n = buf.len().min(self.pending.len());
        for (dst, src) in buf.iter_mut().zip(self.pending.drain(..n)) {
            *dst = src;
        }
        Ok(n)
    }
}

#[test]
fn a_put_list_and_get_through_the_apps_serial_port() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/gxrom-r not found");
        return;
    };
    let (mut master, slave) = TTYPort::pair().expect("a pseudo-terminal pair");
    let name = slave.name().expect("the slave's name");
    master.set_timeout(Duration::from_millis(5)).unwrap();

    // The calculator: the emulated 48GX, its server running, on the master end.
    let mut calc =
        SaturnusTransport::open(EmulatorModel::Hp48gx, &rom).expect("the 48GX boots into SERVER");
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let bridge = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while !stopped.load(Ordering::Relaxed) {
            match master.read(&mut buf) {
                Ok(n) if n > 0 => calc.write_packet(&buf[..n]).unwrap(),
                Ok(_) => {}
                Err(e)
                    if e.kind() == io::ErrorKind::TimedOut
                        || e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("the master end failed: {e}"),
            }
            let n = calc.read(&mut buf, Duration::from_millis(5)).unwrap();
            if n > 0 {
                master.write_all(&buf[..n]).unwrap();
            }
        }
    });

    // The computer: the app's serial code on the slave end, opened by name.
    let ports = Arc::new(Ports::default());
    let (tx, rx) = channel();
    let sink = move |e| {
        let _ = tx.send(e);
    };
    // Linux opens a pty by name as an adapter; macOS's refuse the speed's
    // ioctl, so there the end already open is handed over.
    let id = if cfg!(target_os = "linux") {
        ports
            .open(&name, Settings::parse(9600, "none", 8).unwrap(), sink)
            .expect("the app opens the port")
    } else {
        let mut end = slave.try_clone_native().expect("the slave end");
        end.set_timeout(Duration::from_millis(50)).unwrap();
        ports
            .adopt(&name, Box::new(end), sink)
            .expect("the app runs the port")
    };
    let link = AppLink {
        ports: Arc::clone(&ports),
        id,
        rx,
        pending: VecDeque::new(),
    };
    let mut host = Calculator::new(Session::new(Box::new(link), Options::default()).unwrap());

    let text = b"%%HP: T(3)A(R)F(.);\r\n\"over a pseudo-terminal\"\r\n";
    host.put("PTY", text, TransferMode::Ascii).expect("put");
    let listing = host.list().expect("list");
    assert!(
        listing.entries.iter().any(|e| e.name == "PTY"),
        "{listing:?}"
    );
    let back = host.get("PTY", TransferMode::Ascii).expect("get");
    assert!(
        String::from_utf8_lossy(&back).contains("\"over a pseudo-terminal\""),
        "{}",
        String::from_utf8_lossy(&back)
    );
    host.remove("PTY").expect("purge");

    ports.close(id);
    stop.store(true, Ordering::Relaxed);
    bridge.join().unwrap();
    drop(slave);
}
