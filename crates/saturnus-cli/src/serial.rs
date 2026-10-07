//! `run`'s serial bridge: the calculator's serial port to a TCP client or
//! to stdin/stdout, while the machine runs paced to wall-clock time on the
//! machine thread (`saturnus_drive::runner`), which calls the bridge
//! between its passes.
//!
//! Kermit timeouts on both ends are wall-clock, so the emulated calculator
//! must run at its real speed: the runner keeps the emulated cycle count at
//! `clock_hz` cycles per real second, and wakes a sleeping CPU when bytes
//! arrive.

use std::fmt::Write as _;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use saturnus::Machine;
use saturnus_drive::runner;

/// Where the serial port goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialSpec {
    /// Listen on this `host:port` and serve one client at a time.
    Tcp(String),
    /// Bytes from stdin go to the calculator, its output to stdout.
    Stdio,
}

impl SerialSpec {
    /// Parse `tcp:PORT`, `tcp:HOST:PORT` or `stdio`.
    pub fn parse(s: &str) -> Result<Self> {
        if s == "stdio" {
            return Ok(Self::Stdio);
        }
        let Some(rest) = s.strip_prefix("tcp:") else {
            bail!("bad --serial {s:?}: expected tcp:PORT, tcp:HOST:PORT or stdio");
        };
        let (host, port) = match rest.rsplit_once(':') {
            Some((h, p)) => (h, p),
            None => ("127.0.0.1", rest),
        };
        let port: u16 = port
            .parse()
            .with_context(|| format!("bad port in --serial {s:?}"))?;
        if host.is_empty() {
            bail!("empty host in --serial {s:?}");
        }
        Ok(Self::Tcp(format!("{host}:{port}")))
    }
}

/// Most bytes queued towards the calculator. The UART takes about 840
/// bytes/s at 9600 baud, so a sender faster than that must wait: above
/// this the bridge stops reading the peer and TCP flow control (or the
/// bounded stdin channel) holds the sender back. Several Kermit packets.
pub const INBOUND_HIGH_WATER: usize = 2048;
/// Largest single read from the peer.
const READ_CHUNK: usize = 1024;
/// Calculator output a peer may leave unread before it is dropped as stuck.
const OUTBOUND_LIMIT: usize = 1 << 20;

/// How many bytes to read from the peer now, with `pending` bytes still
/// queued for the calculator: zero means leave them in the peer's buffer.
pub fn read_budget(pending: usize) -> usize {
    INBOUND_HIGH_WATER.saturating_sub(pending).min(READ_CHUNK)
}

/// Whether `addr` (`host:port`) is a loopback address (127.0.0.0/8, ::1,
/// localhost).
pub fn is_loopback(addr: &str) -> bool {
    if let Ok(a) = addr.parse::<std::net::SocketAddr>() {
        return a.ip().is_loopback();
    }
    addr.rsplit_once(':')
        .is_some_and(|(h, _)| h.eq_ignore_ascii_case("localhost"))
}

/// The request methods an HTTP client starts with. Not `CONNECT`: no
/// browser page can send it (fetch forbids it), and a lone `C` is how an
/// XMODEM-CRC receiver starts, which must not wait.
const HTTP_METHODS: [&[u8]; 8] = [
    b"GET ",
    b"POST ",
    b"PUT ",
    b"HEAD ",
    b"OPTIONS ",
    b"DELETE ",
    b"PATCH ",
    b"TRACE ",
];
/// How long the first bytes of a connection may stay undecided (a prefix
/// of a method name) before they are forwarded anyway.
const SNIFF_WAIT: Duration = Duration::from_millis(300);

/// What the first bytes of a TCP connection are.
#[derive(Debug, PartialEq, Eq)]
enum Sniff {
    /// An HTTP request line: a browser or other HTTP client, refused.
    Http,
    /// Not HTTP: forward. Kermit (SOH) and XMODEM (NAK, `C`, SOH) are
    /// told apart by their first byte.
    Plain,
    /// Still a prefix of a method name.
    Undecided,
}

fn sniff(first: &[u8]) -> Sniff {
    if HTTP_METHODS.iter().any(|m| first.starts_with(m)) {
        Sniff::Http
    } else if HTTP_METHODS.iter().any(|m| m.starts_with(first)) {
        Sniff::Undecided
    } else {
        Sniff::Plain
    }
}

/// Options of the bridge loop.
#[derive(Debug)]
pub struct BridgeOptions<'a> {
    /// Where the port goes.
    pub spec: SerialSpec,
    /// Stop after the first client disconnects (stdin EOF for `stdio`).
    pub exit_on_disconnect: bool,
    /// Append a trace of the wire traffic here.
    pub log: Option<&'a Path>,
    /// Report connections and pacing on stderr.
    pub verbose: bool,
}

/// The peer on the other end of the wire.
enum Peer {
    Tcp(TcpStream),
    /// Chunks from the stdin thread, the unread rest of the last chunk,
    /// and stdout.
    Stdio(Receiver<Vec<u8>>, Vec<u8>, io::Stdout),
}

/// Result of polling a peer for input.
enum Input {
    Bytes(Vec<u8>),
    Nothing,
    Closed,
}

impl Peer {
    /// Read at most `max` (> 0) bytes that are available now.
    fn poll(&mut self, max: usize) -> io::Result<Input> {
        match self {
            Peer::Tcp(s) => {
                let mut buf = [0u8; READ_CHUNK];
                let max = max.min(buf.len());
                match s.read(&mut buf[..max]) {
                    Ok(0) => Ok(Input::Closed),
                    Ok(n) => Ok(Input::Bytes(buf[..n].to_vec())),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(Input::Nothing),
                    Err(e) if e.kind() == ErrorKind::Interrupted => Ok(Input::Nothing),
                    Err(e) => Err(e),
                }
            }
            Peer::Stdio(rx, rest, _) => {
                if rest.is_empty() {
                    match rx.try_recv() {
                        Ok(b) => *rest = b,
                        Err(TryRecvError::Empty) => return Ok(Input::Nothing),
                        Err(TryRecvError::Disconnected) => return Ok(Input::Closed),
                    }
                }
                let n = max.min(rest.len());
                Ok(Input::Bytes(rest.drain(..n).collect()))
            }
        }
    }

    /// Write as much of `out` as the peer takes now; the rest stays.
    fn send(&mut self, out: &mut Vec<u8>) -> io::Result<()> {
        if out.is_empty() {
            return Ok(());
        }
        match self {
            Peer::Tcp(s) => {
                while !out.is_empty() {
                    match s.write(out) {
                        Ok(0) => return Err(io::Error::from(ErrorKind::WriteZero)),
                        Ok(n) => {
                            out.drain(..n);
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(()),
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Peer::Stdio(_, _, stdout) => {
                let mut lock = stdout.lock();
                lock.write_all(out)?;
                lock.flush()?;
                out.clear();
                Ok(())
            }
        }
    }
}

/// Wire trace in hptx's `kermit_proto::trace` style, with the emulated
/// time in milliseconds: `> ` host to calculator, `< ` calculator to host.
struct WireLog {
    file: std::fs::File,
}

impl WireLog {
    fn open(p: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .with_context(|| format!("cannot open serial log {}", p.display()))?;
        Ok(Self { file })
    }

    fn record(&mut self, ms: f64, dir: char, bytes: &[u8]) -> Result<()> {
        let mut line = format!("{ms:10.1} {dir} ");
        for &b in bytes {
            match b {
                b'\\' => line.push_str("\\\\"),
                0x20..=0x7E => line.push(char::from(b)),
                _ => {
                    let _ = write!(line, "\\x{b:02x}");
                }
            }
        }
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .context("cannot write serial log")
    }

    fn note(&mut self, ms: f64, text: &str) -> Result<()> {
        self.file
            .write_all(format!("{ms:10.1} # {text}\n").as_bytes())
            .context("cannot write serial log")
    }
}

/// Spawn a thread that forwards stdin in chunks; the channel closes on EOF.
/// The channel holds one chunk, so the thread blocks (and stops reading
/// stdin) while the bridge is not taking input.
fn stdin_reader() -> Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; READ_CHUNK];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

/// The serial bridge, served from the machine thread between its passes
/// (a [`runner::Hook`]): accept a client, move its bytes into the UART
/// (at most [`INBOUND_HIGH_WATER`] queued) and the calculator's out.
pub struct SerialPort {
    listener: Option<TcpListener>,
    peer: Option<Peer>,
    out: Vec<u8>,
    log: Option<WireLog>,
    /// Bytes the calculator sent while nobody listened.
    dropped: usize,
    /// The first bytes of a new TCP client, held until they cannot be an
    /// HTTP request line, and when the first arrived.
    first: Option<(Vec<u8>, Option<Instant>)>,
    /// Set by `exit_on_disconnect`: stop once the last bytes went out.
    leaving: bool,
    exit_on_disconnect: bool,
    verbose: bool,
    cycles_per_ms: f64,
}

impl std::fmt::Debug for SerialPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialPort")
            .field("connected", &self.peer.is_some())
            .finish_non_exhaustive()
    }
}

/// What a turn of the bridge did.
enum Turn {
    Idle,
    Input,
    Leave,
}

impl SerialPort {
    /// Open the port of `opts` for a machine of `clock_hz`: listen (TCP) or
    /// take stdin/stdout. Returns the port and its endpoint for messages
    /// (`tcp:127.0.0.1:4841`, `stdio`).
    pub fn open(opts: &BridgeOptions<'_>, clock_hz: u32) -> Result<(Self, String)> {
        let log = opts.log.map(WireLog::open).transpose()?;
        let (listener, peer, endpoint) = match &opts.spec {
            SerialSpec::Tcp(addr) => {
                let l = TcpListener::bind(addr)
                    .map_err(|e| crate::control::bind_error("serial bridge", addr, e))?;
                l.set_nonblocking(true)
                    .context("cannot make the listener non-blocking")?;
                let local = l.local_addr().context("listener has no address")?;
                (Some(l), None, format!("tcp:{local}"))
            }
            SerialSpec::Stdio => (
                None,
                Some(Peer::Stdio(stdin_reader(), Vec::new(), io::stdout())),
                "stdio".to_string(),
            ),
        };
        let port = Self {
            listener,
            peer,
            out: Vec::new(),
            log,
            dropped: 0,
            first: None,
            leaving: false,
            exit_on_disconnect: opts.exit_on_disconnect,
            verbose: opts.verbose,
            cycles_per_ms: f64::from(clock_hz) / 1000.0,
        };
        Ok((port, endpoint))
    }

    fn ms(&self, m: &Machine) -> f64 {
        m.cycles() as f64 / self.cycles_per_ms
    }

    /// Longest the machine thread may block between two turns: short while
    /// a client is connected (bytes arrive at any time), longer while only
    /// listening.
    fn interval(&self) -> Duration {
        if self.peer.is_some() {
            Duration::from_millis(1)
        } else {
            Duration::from_millis(10)
        }
    }

    /// One turn between the machine's passes.
    fn turn(&mut self, m: &mut Machine) -> Result<Turn> {
        if self.leaving && m.serial_pending() == 0 {
            return Ok(Turn::Leave);
        }
        // A new client waits until the previous one's last bytes (typically
        // its final ACK) have reached the calculator, so the sessions do
        // not mix; at most INBOUND_HIGH_WATER bytes, about 2.4 s.
        if self.peer.is_none()
            && !self.leaving
            && m.serial_pending() == 0
            && let Some(l) = &self.listener
        {
            match l.accept() {
                Ok((stream, addr)) => {
                    stream.set_nodelay(true).context("cannot set TCP_NODELAY")?;
                    stream
                        .set_nonblocking(true)
                        .context("cannot make the client socket non-blocking")?;
                    if self.verbose {
                        eprintln!("serial: client {addr} connected");
                    }
                    let ms = self.ms(m);
                    if let Some(l) = self.log.as_mut() {
                        l.note(ms, "connect")?;
                    }
                    self.peer = Some(Peer::Tcp(stream));
                    self.first = Some((Vec::new(), None));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(e).context("accept failed"),
            }
        }
        // One bounded read per turn; nothing while enough is queued
        // (backpressure).
        let mut closed = false;
        let mut refused = false;
        let mut input = false;
        let budget = read_budget(m.serial_pending());
        let mut forward = Vec::new();
        if let Some(p) = self.peer.as_mut()
            && budget > 0
        {
            match p.poll(budget) {
                Ok(Input::Bytes(b)) => match self.first.as_mut() {
                    None => forward = b,
                    Some((held, since)) => {
                        held.extend_from_slice(&b);
                        since.get_or_insert_with(Instant::now);
                        match sniff(held) {
                            Sniff::Http => refused = true,
                            Sniff::Plain => forward = std::mem::take(held),
                            Sniff::Undecided => {}
                        }
                    }
                },
                Ok(Input::Nothing) => {}
                Ok(Input::Closed) => closed = true,
                Err(e) => {
                    if self.verbose {
                        eprintln!("serial: read error {e}, dropping client");
                    }
                    closed = true;
                }
            }
        }
        // A method-name prefix that stops there (a lone "G") goes through
        // after a short wait.
        if let Some((held, Some(since))) = self.first.as_mut()
            && !refused
            && since.elapsed() >= SNIFF_WAIT
        {
            forward = std::mem::take(held);
        }
        if !forward.is_empty() {
            self.first = None;
            let ms = self.ms(m);
            if let Some(l) = self.log.as_mut() {
                l.record(ms, '>', &forward)?;
            }
            m.serial_push(&forward);
            input = true;
        }
        if refused {
            // Not one byte reached the calculator; the slot is free again.
            if self.verbose {
                eprintln!("serial: refused an HTTP request (a browser?), closing it");
            }
            let ms = self.ms(m);
            if let Some(l) = self.log.as_mut() {
                l.note(ms, "refused an HTTP request")?;
            }
            self.peer = None;
            self.first = None;
            self.out.clear();
            return Ok(if input { Turn::Input } else { Turn::Idle });
        }
        let tx = m.serial_drain();
        if !tx.is_empty() {
            let ms = self.ms(m);
            if let Some(l) = self.log.as_mut() {
                l.record(ms, '<', &tx)?;
            }
            if self.peer.is_some() && !closed {
                self.out.extend_from_slice(&tx);
                if self.out.len() > OUTBOUND_LIMIT {
                    if self.verbose {
                        eprintln!("serial: client is not reading, dropping it");
                    }
                    closed = true;
                }
            } else {
                self.dropped += tx.len();
            }
        }
        if let Some(p) = self.peer.as_mut()
            && !closed
            && let Err(e) = p.send(&mut self.out)
        {
            if self.verbose {
                eprintln!("serial: write error {e}, dropping client");
            }
            closed = true;
        }
        if closed {
            self.peer = None;
            self.first = None;
            self.out.clear();
            if self.verbose {
                eprintln!(
                    "serial: client disconnected at cycle {} ({} bytes still going out to the calculator)",
                    m.cycles(),
                    m.serial_pending()
                );
            }
            let ms = self.ms(m);
            if let Some(l) = self.log.as_mut() {
                l.note(ms, "disconnect")?;
            }
            if self.exit_on_disconnect {
                self.leaving = true;
            }
        }
        Ok(if input { Turn::Input } else { Turn::Idle })
    }

    /// The bridge stops: bytes nobody will wait for any more stay out of
    /// `--save`.
    fn finish(&mut self, m: &mut Machine) {
        let discarded = m.serial_clear_inbound();
        if self.verbose {
            eprintln!(
                "serial: stopped at cycle {}; {discarded} queued bytes discarded; {} bytes sent with no client",
                m.cycles(),
                self.dropped
            );
        }
    }
}

/// What `saturnus run` does beside the machine while it serves: the serial
/// bridge, and stopping on SIGINT/SIGTERM. An error of the bridge stops
/// the run and is kept for [`ServeHook::errors`].
#[derive(Debug)]
pub struct ServeHook {
    stop: Arc<AtomicBool>,
    serial: Option<SerialPort>,
    error: Arc<std::sync::Mutex<Option<anyhow::Error>>>,
}

impl ServeHook {
    /// Serve `serial` (if any) until `stop` is set.
    pub fn new(stop: Arc<AtomicBool>, serial: Option<SerialPort>) -> Self {
        Self {
            stop,
            serial,
            error: Arc::default(),
        }
    }

    /// Where the bridge's error lands, readable after the run.
    pub fn errors(&self) -> Arc<std::sync::Mutex<Option<anyhow::Error>>> {
        Arc::clone(&self.error)
    }
}

impl runner::Hook for ServeHook {
    fn service(&mut self, machine: Option<&mut Machine>) -> runner::Service {
        let Some(m) = machine else {
            return if self.stop.load(Ordering::Relaxed) {
                runner::Service::Stop
            } else {
                runner::Service::Idle
            };
        };
        let Some(port) = self.serial.as_mut() else {
            return if self.stop.load(Ordering::Relaxed) {
                runner::Service::Stop
            } else {
                runner::Service::Idle
            };
        };
        if self.stop.load(Ordering::Relaxed) {
            port.finish(m);
            return runner::Service::Stop;
        }
        match port.turn(m) {
            Ok(Turn::Idle) => runner::Service::Idle,
            Ok(Turn::Input) => runner::Service::Input,
            Ok(Turn::Leave) => {
                port.finish(m);
                runner::Service::Stop
            }
            Err(e) => {
                port.finish(m);
                if let Ok(mut slot) = self.error.lock() {
                    *slot = Some(e);
                }
                runner::Service::Stop
            }
        }
    }

    fn interval(&self) -> Duration {
        self.serial
            .as_ref()
            .map_or(Duration::from_millis(50), SerialPort::interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_bytes_tell_http_from_serial_protocols() {
        assert_eq!(sniff(b"GET / HTTP/1.1"), Sniff::Http);
        assert_eq!(sniff(b"POST "), Sniff::Http);
        assert_eq!(sniff(b"OPTIONS /x"), Sniff::Http);
        for undecided in [&b""[..], b"G", b"PO", b"OPTION", b"DELETE"] {
            assert_eq!(sniff(undecided), Sniff::Undecided, "{undecided:?}");
        }
        // Kermit and XMODEM: decided by the first byte.
        for plain in [&b"\x01"[..], b"\x15", b"C", b"GX", b"get /", b"POSTX"] {
            assert_eq!(sniff(plain), Sniff::Plain, "{plain:?}");
        }
        assert!(is_loopback("127.0.0.1:4841"));
        assert!(is_loopback("127.1.2.3:1"));
        assert!(is_loopback("[::1]:1"));
        assert!(is_loopback("localhost:1"));
        assert!(!is_loopback("0.0.0.0:4841"));
        assert!(!is_loopback("192.168.1.2:4841"));
    }

    /// A bridge on an ephemeral port in front of a 48SX on a ROM of zeros,
    /// with its wire log.
    fn bridge(dir: &Path) -> (SerialPort, Machine, u16, std::path::PathBuf) {
        let log = dir.join("wire.log");
        let opts = BridgeOptions {
            spec: SerialSpec::Tcp("127.0.0.1:0".into()),
            exit_on_disconnect: false,
            log: Some(&log),
            verbose: false,
        };
        let (port, endpoint) = SerialPort::open(&opts, 2_000_000).unwrap();
        let tcp = endpoint.rsplit(':').next().unwrap().parse().unwrap();
        let m = Machine::new(saturnus::Model::Hp48sx, &vec![0u8; 256 * 1024]).unwrap();
        (port, m, tcp, log)
    }

    /// Turns until `done` holds, at most 3 s.
    fn turns(
        p: &mut SerialPort,
        m: &mut Machine,
        mut done: impl FnMut(&SerialPort, &Turn) -> bool,
    ) {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(3) {
            let r = p.turn(m).unwrap();
            if done(p, &r) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("timed out");
    }

    /// A browser's request (a no-cors POST) reaches the calculator with
    /// not one byte, and the one-client slot is free again at once; a
    /// Kermit packet after it passes unchanged in the turn that reads it.
    #[test]
    fn http_requests_are_refused_and_kermit_passes() {
        let dir = std::env::temp_dir().join(format!("saturnus-sniff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut p, mut m, port, log) = bridge(&dir);
        let mut browser = TcpStream::connect(("127.0.0.1", port)).unwrap();
        browser
            .write_all(
                b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 9\r\n\r\n\x01# S~* @-",
            )
            .unwrap();
        turns(&mut p, &mut m, |_, _| {
            std::fs::read_to_string(&log).is_ok_and(|t| t.contains("refused"))
        });
        assert!(p.peer.is_none(), "the slot is free");
        assert_eq!(m.serial_pending(), 0);
        browser
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut rest = Vec::new();
        let _ = browser.read_to_end(&mut rest);
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(!text.contains(" > "), "{text}");
        assert!(text.contains("refused an HTTP request"), "{text}");

        let mut kermit = TcpStream::connect(("127.0.0.1", port)).unwrap();
        turns(&mut p, &mut m, |p, _| p.peer.is_some());
        let packet = b"\x01) SI~* @-#Y1~\r";
        kermit.write_all(packet).unwrap();
        // The turn that reads the bytes forwards them.
        turns(&mut p, &mut m, |_, r| matches!(r, Turn::Input));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains(r" > \x01) SI~* @-#Y1~\x0d"), "{text}");
        drop(kermit);
        drop(p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_specs() {
        assert_eq!(
            SerialSpec::parse("tcp:4850").unwrap(),
            SerialSpec::Tcp("127.0.0.1:4850".into())
        );
        assert_eq!(
            SerialSpec::parse("tcp:0.0.0.0:1").unwrap(),
            SerialSpec::Tcp("0.0.0.0:1".into())
        );
        assert_eq!(SerialSpec::parse("stdio").unwrap(), SerialSpec::Stdio);
        assert!(SerialSpec::parse("tcp:").is_err());
        assert!(SerialSpec::parse("tcp::5").is_err());
        assert!(SerialSpec::parse("tcp:99999").is_err());
        assert!(SerialSpec::parse("udp:1").is_err());
    }

    #[test]
    fn read_budget_stops_at_the_high_water_mark() {
        assert_eq!(read_budget(0), READ_CHUNK);
        assert_eq!(read_budget(INBOUND_HIGH_WATER - 10), 10);
        assert_eq!(read_budget(INBOUND_HIGH_WATER), 0);
        assert_eq!(read_budget(usize::MAX), 0);
    }

    #[test]
    fn stdio_peer_hands_out_at_most_max_bytes() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut p = Peer::Stdio(rx, Vec::new(), io::stdout());
        assert!(matches!(p.poll(4).unwrap(), Input::Nothing));
        tx.send(b"abcdef".to_vec()).unwrap();
        let mut got = Vec::new();
        while let Input::Bytes(b) = p.poll(4).unwrap() {
            assert!(b.len() <= 4);
            got.extend(b);
        }
        assert_eq!(got, b"abcdef");
        drop(tx);
        assert!(matches!(p.poll(4).unwrap(), Input::Closed));
    }

    #[test]
    fn tcp_peer_leaves_unread_bytes_in_the_socket() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut c = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (srv, _) = l.accept().unwrap();
        srv.set_nonblocking(true).unwrap();
        c.write_all(&[7u8; 3000]).unwrap();
        let mut p = Peer::Tcp(srv);
        let mut total = 0;
        let start = Instant::now();
        while total < 3000 && start.elapsed() < Duration::from_secs(5) {
            match p.poll(100).unwrap() {
                Input::Bytes(b) => {
                    assert!(b.len() <= 100);
                    total += b.len();
                }
                Input::Nothing => std::thread::sleep(Duration::from_millis(1)),
                Input::Closed => panic!("closed"),
            }
        }
        assert_eq!(total, 3000);
    }
}
