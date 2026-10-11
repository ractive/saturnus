//! Native serial ports for the page (kb/docs/saturnus-tx.md, "Devices"):
//! the list with each adapter's name, and open ports that move bytes
//! only. The protocol stays in the page's wasm; this side opens a port at
//! the settings asked for, writes what the page hands it and passes every
//! byte read to a sink, from a reader thread per open port. A read error
//! that is not a timeout (the adapter unplugged) ends the reader and is
//! said to the sink once.
//!
//! Independent of Tauri, so `tests/serial_pty.rs` drives it over a
//! pseudo-terminal pair.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;
use serialport::{SerialPortInfo, SerialPortType};

/// How long a read waits before the reader looks at its stop flag again.
const READ_TIMEOUT: Duration = Duration::from_millis(50);
/// Largest single read.
const READ_CHUNK: usize = 4096;

/// What the page says when the port is held elsewhere (web/tx/links.js's
/// `PORT_IN_USE`, the same words as the browser's).
pub const PORT_IN_USE: &str =
    "The serial port is in use by another tab or program; close it there and try again.";

/// A port as the page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Port {
    /// What the OS opens: `/dev/cu.usbserial-A10K`, `COM3`.
    pub name: String,
    /// What the list shows: `FT232R USB UART (FTDI) on /dev/cu.usbserial-A10K`.
    pub label: String,
    /// Whether it is a USB adapter (listed first).
    pub usb: bool,
}

/// `info` as the page lists it: the adapter's product and maker where the
/// OS gives them, else its USB ids, else the kind of port; always the
/// name the OS opens.
pub fn port_of(info: &SerialPortInfo) -> Port {
    let name = info.port_name.clone();
    let (what, usb) = match &info.port_type {
        SerialPortType::UsbPort(u) => {
            let product = u
                .product
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let maker = u
                .manufacturer
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let what = match (product, maker) {
                (Some(p), Some(m)) if !p.contains(m) => format!("{p} ({m})"),
                (Some(p), _) => p.to_string(),
                (None, Some(m)) => format!("{m} USB serial adapter"),
                (None, None) => format!("USB serial adapter {:04x}:{:04x}", u.vid, u.pid),
            };
            (what, true)
        }
        SerialPortType::BluetoothPort => ("Bluetooth".to_string(), false),
        SerialPortType::PciPort => ("Serial port".to_string(), false),
        SerialPortType::Unknown => ("Serial port".to_string(), false),
    };
    Port {
        label: format!("{what} on {name}"),
        name,
        usb,
    }
}

/// The ports of `infos` as the page lists them: USB adapters first, then
/// by name. On macOS each port is there twice, as `/dev/tty.X` (whose
/// open waits for the carrier) and `/dev/cu.X`; only the `cu` one is kept.
pub fn list_of(infos: &[SerialPortInfo]) -> Vec<Port> {
    let names: Vec<&str> = infos.iter().map(|i| i.port_name.as_str()).collect();
    let mut ports: Vec<Port> = infos
        .iter()
        .filter(|i| {
            i.port_name
                .strip_prefix("/dev/tty.")
                .is_none_or(|rest| !names.contains(&format!("/dev/cu.{rest}").as_str()))
        })
        .map(port_of)
        .collect();
    ports.sort_by(|a, b| b.usb.cmp(&a.usb).then_with(|| a.name.cmp(&b.name)));
    ports.dedup_by(|a, b| a.name == b.name);
    ports
}

/// The serial ports of this computer.
pub fn ports() -> Result<Vec<Port>, String> {
    let infos = serialport::available_ports()
        .map_err(|e| format!("The serial ports could not be listed: {e}"))?;
    Ok(list_of(&infos))
}

/// The line settings the page opens a port at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Bits per second.
    pub baud: u32,
    /// `none`, `even` or `odd`, as the page's settings name them.
    pub parity: serialport::Parity,
    /// 7 or 8.
    pub data_bits: serialport::DataBits,
}

impl Settings {
    /// From the page's names: parity `none`, `even` or `odd`; 7 or 8 data bits.
    pub fn parse(baud: u32, parity: &str, data_bits: u8) -> Result<Self, String> {
        let parity = match parity {
            "none" => serialport::Parity::None,
            "even" => serialport::Parity::Even,
            "odd" => serialport::Parity::Odd,
            other => return Err(format!("unknown parity {other:?}")),
        };
        let data_bits = match data_bits {
            7 => serialport::DataBits::Seven,
            8 => serialport::DataBits::Eight,
            other => return Err(format!("{other} data bits are not offered")),
        };
        if baud == 0 {
            return Err("a speed of 0 baud".into());
        }
        Ok(Settings {
            baud,
            parity,
            data_bits,
        })
    }
}

/// What an open port tells the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Bytes read.
    Data(Vec<u8>),
    /// The port went away (why, in plain words); nothing follows.
    Lost(String),
}

/// One open port: the handle writes go through, and its reader.
struct Open {
    port: Mutex<Box<dyn serialport::SerialPort>>,
    stop: Arc<AtomicBool>,
    reader: Mutex<Option<JoinHandle<()>>>,
}

/// The open ports, by the id `open` gave.
#[derive(Default)]
pub struct Ports {
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<Open>>>,
}

impl std::fmt::Debug for Ports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.open.lock().map(|m| m.len()).unwrap_or_default();
        f.debug_struct("Ports").field("open", &n).finish()
    }
}

/// An open error in the page's words.
fn open_error(name: &str, e: &serialport::Error) -> String {
    let text = e.to_string();
    let lower = text.to_lowercase();
    if lower.contains("busy") || lower.contains("in use") || lower.contains("access is denied") {
        PORT_IN_USE.to_string()
    } else if lower.contains("permission denied") {
        format!(
            "This user may not open {name}. On Linux, add yourself to the dialout group (uucp on Arch) and log in again."
        )
    } else if matches!(e.kind, serialport::ErrorKind::NoDevice) || lower.contains("no such file") {
        format!("{name} is not there any more: the adapter was unplugged.")
    } else {
        format!("{name} could not be opened: {text}")
    }
}

impl Ports {
    /// Open port `name` at `settings` (one stop bit, no flow control) and
    /// start its reader, which hands every read to `sink`. Resolves to the
    /// port's id for `write` and `close`.
    pub fn open(
        &self,
        name: &str,
        settings: Settings,
        sink: impl Fn(Event) + Send + 'static,
    ) -> Result<u32, String> {
        let port = serialport::new(name, settings.baud)
            .data_bits(settings.data_bits)
            .parity(settings.parity)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(READ_TIMEOUT)
            .open()
            .map_err(|e| open_error(name, &e))?;
        self.adopt(name, port, sink)
    }

    /// Run `port`, already open (named `name`), as `open` runs the ports
    /// it opens: its reader starts, and it gets an id. The pseudo-terminal
    /// test hands its end over here where `open` cannot set its speed
    /// (macOS's ptys refuse the ioctl).
    pub fn adopt(
        &self,
        name: &str,
        port: Box<dyn serialport::SerialPort>,
        sink: impl Fn(Event) + Send + 'static,
    ) -> Result<u32, String> {
        let mut reading = port
            .try_clone()
            .map_err(|e| format!("{name} could not be read: {e}"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let reader = std::thread::Builder::new()
            .name(format!("serial {name}"))
            .spawn(move || {
                let mut buf = vec![0u8; READ_CHUNK];
                while !stopped.load(Ordering::Acquire) {
                    match reading.read(&mut buf) {
                        Ok(0) => {}
                        Ok(n) => sink(Event::Data(buf[..n].to_vec())),
                        Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                        Err(e) => {
                            if !stopped.load(Ordering::Acquire) {
                                sink(Event::Lost(format!(
                                    "The serial port failed ({e}): was the cable or adapter unplugged?"
                                )));
                            }
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("no reader for {name}: {e}"))?;
        let id = self.next.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let open = Arc::new(Open {
            port: Mutex::new(port),
            stop,
            reader: Mutex::new(Some(reader)),
        });
        self.open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, open);
        Ok(id)
    }

    fn get(&self, id: u32) -> Result<Arc<Open>, String> {
        self.open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .cloned()
            .ok_or_else(|| "the serial port is not open".to_string())
    }

    /// Write all of `bytes` to port `id`; returns once they are handed to
    /// the OS.
    pub fn write(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
        let open = self.get(id)?;
        let mut port = open.port.lock().unwrap_or_else(PoisonError::into_inner);
        port.write_all(bytes)
            .and_then(|()| port.flush())
            .map_err(|e| {
                format!("The serial port failed ({e}): was the cable or adapter unplugged?")
            })
    }

    /// Close port `id`: its reader stops (no `Lost` follows), the port is
    /// let go. Closing a port that is not open does nothing.
    pub fn close(&self, id: u32) {
        let open = self
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
        if let Some(open) = open {
            open.stop.store(true, Ordering::Release);
            let reader = open
                .reader
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(reader) = reader {
                let _ = reader.join();
            }
        }
    }

    /// Close every port (the window closed or reloaded).
    pub fn close_all(&self) {
        let ids: Vec<u32> = self
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .copied()
            .collect();
        for id in ids {
            self.close(id);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serialport::UsbPortInfo;

    fn usb(name: &str, product: Option<&str>, maker: Option<&str>) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.into(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid: 0x0403,
                pid: 0x6001,
                serial_number: None,
                manufacturer: maker.map(Into::into),
                product: product.map(Into::into),
            }),
        }
    }

    fn other(name: &str, port_type: SerialPortType) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.into(),
            port_type,
        }
    }

    /// The list names the adapter: product and maker, the product alone
    /// when it names the maker, the maker, or the USB ids.
    #[test]
    fn a_port_is_named_after_its_adapter() {
        let p = port_of(&usb(
            "/dev/cu.usbserial-A1",
            Some("FT232R USB UART"),
            Some("FTDI"),
        ));
        assert_eq!(p.label, "FT232R USB UART (FTDI) on /dev/cu.usbserial-A1");
        assert!(p.usb);
        let p = port_of(&usb(
            "COM3",
            Some("Prolific USB-to-Serial"),
            Some("Prolific"),
        ));
        assert_eq!(p.label, "Prolific USB-to-Serial on COM3");
        let p = port_of(&usb("COM4", None, Some("FTDI")));
        assert_eq!(p.label, "FTDI USB serial adapter on COM4");
        let p = port_of(&usb("COM5", Some("  "), None));
        assert_eq!(p.label, "USB serial adapter 0403:6001 on COM5");
        let p = port_of(&other("/dev/ttyS0", SerialPortType::PciPort));
        assert_eq!(p.label, "Serial port on /dev/ttyS0");
        assert!(!p.usb);
        let p = port_of(&other("/dev/rfcomm0", SerialPortType::BluetoothPort));
        assert_eq!(p.label, "Bluetooth on /dev/rfcomm0");
    }

    /// USB first, then by name; macOS's `tty.` twin of a `cu.` port left out.
    #[test]
    fn the_list_puts_usb_first_and_drops_the_macos_dial_in_twins() {
        let infos = vec![
            other("/dev/tty.Bluetooth-Incoming-Port", SerialPortType::Unknown),
            other("/dev/cu.Bluetooth-Incoming-Port", SerialPortType::Unknown),
            usb(
                "/dev/tty.usbserial-A1",
                Some("FT232R USB UART"),
                Some("FTDI"),
            ),
            usb(
                "/dev/cu.usbserial-A1",
                Some("FT232R USB UART"),
                Some("FTDI"),
            ),
            other("/dev/ttyS0", SerialPortType::PciPort),
            other("/dev/tty.only", SerialPortType::Unknown),
        ];
        let names: Vec<String> = list_of(&infos).into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "/dev/cu.usbserial-A1",
                "/dev/cu.Bluetooth-Incoming-Port",
                "/dev/tty.only",
                "/dev/ttyS0"
            ]
        );
    }

    #[test]
    fn the_settings_are_the_pages_names() {
        let s = Settings::parse(9600, "none", 8).unwrap();
        assert_eq!(s.parity, serialport::Parity::None);
        assert_eq!(s.data_bits, serialport::DataBits::Eight);
        assert_eq!(
            Settings::parse(2400, "even", 7).unwrap().parity,
            serialport::Parity::Even
        );
        assert_eq!(
            Settings::parse(1200, "odd", 8).unwrap().parity,
            serialport::Parity::Odd
        );
        assert!(Settings::parse(9600, "mark", 8).is_err());
        assert!(Settings::parse(9600, "none", 6).is_err());
        assert!(Settings::parse(0, "none", 8).is_err());
    }

    #[test]
    fn a_port_that_is_not_there_says_so() {
        let ports = Ports::default();
        let e = ports
            .open(
                "/dev/saturnus-tx-no-such-port",
                Settings::parse(9600, "none", 8).unwrap(),
                |_| {},
            )
            .unwrap_err();
        assert!(e.contains("saturnus-tx-no-such-port"), "{e}");
        assert!(ports.write(7, b"x").is_err(), "not open");
        ports.close(7);
    }
}
