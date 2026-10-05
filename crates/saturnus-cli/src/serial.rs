//! `run --serial`: bridge the calculator's serial port to a TCP client or
//! to stdin/stdout, with the machine paced to wall-clock time.
//!
//! Kermit timeouts on both ends are wall-clock, so the emulated calculator
//! must run at its real speed: [`Pacer`] keeps the emulated cycle count at
//! `clock_hz` cycles per real second, sleeping when ahead and catching up
//! (boundedly) when behind.

use std::fmt::Write as _;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use saturnus::io::Key;

use crate::script::{Action, DEFAULT_HOLD_MS, Line};
use crate::session::Session;

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

/// Emulated time is re-anchored when it falls further behind wall-clock
/// time than this (a slow host, a debugger stop): catching up a long lag
/// would run the calculator in a burst that no real one ever does.
const MAX_LAG: Duration = Duration::from_millis(200);
/// Longest stretch of emulated time run between two socket polls.
const SLICE: Duration = Duration::from_millis(1);
/// Sleep when emulated time is ahead of wall-clock time.
const IDLE_SLEEP: Duration = Duration::from_micros(500);

/// Maps wall-clock time to a target cycle count.
#[derive(Clone, Copy, Debug)]
pub struct Pacer {
    clock_hz: u64,
    start: Instant,
    start_cycles: u64,
    /// Times the anchor was moved because the machine fell behind.
    pub rebases: u64,
}

impl Pacer {
    /// Start pacing now, at `cycles`.
    pub fn new(clock_hz: u32, now: Instant, cycles: u64) -> Self {
        Self {
            clock_hz: u64::from(clock_hz),
            start: now,
            start_cycles: cycles,
            rebases: 0,
        }
    }

    /// Cycles corresponding to `d`.
    pub fn cycles_in(&self, d: Duration) -> u64 {
        let c = d.as_nanos() * u128::from(self.clock_hz) / 1_000_000_000;
        u64::try_from(c).unwrap_or(u64::MAX)
    }

    /// How many cycles to run at `now` with the machine at `cycles`, at
    /// most one [`SLICE`]. Zero means the machine is ahead: sleep.
    pub fn budget(&mut self, now: Instant, cycles: u64) -> u64 {
        let target = self
            .start_cycles
            .saturating_add(self.cycles_in(now.saturating_duration_since(self.start)));
        let behind = target.saturating_sub(cycles);
        if behind > self.cycles_in(MAX_LAG) {
            self.start = now;
            self.start_cycles = cycles;
            self.rebases += 1;
            return self.cycles_in(SLICE);
        }
        behind.min(self.cycles_in(SLICE))
    }
}

/// The ROM's `SERVER` command typed after boot: with no `--load`, first
/// answer "Try To Recover Memory?" with NO (softkey F), then ALPHA ALPHA
/// S E R V E R ENTER, as the saturnng container's entrypoint does.
pub fn autostart_script(fresh_boot: bool) -> Vec<Line> {
    let press = |key| Action::Press {
        key,
        hold_ms: DEFAULT_HOLD_MS,
    };
    let mut actions = Vec::new();
    if fresh_boot {
        actions.push(Action::WaitIdle { cap_ms: 60_000 });
        actions.push(press(Key::F));
    }
    actions.push(press(Key::Alpha));
    actions.push(press(Key::Alpha));
    for c in "SERVER".chars() {
        // Alpha letters on the 48SX: S = SIN, E = softkey E, R = right
        // arrow, V = square root.
        let key = match c {
            'S' => Key::Sin,
            'E' => Key::E,
            'R' => Key::Right,
            _ => Key::Sqrt,
        };
        actions.push(press(key));
    }
    // ENTER starts the server; it then never idles in SHUTDN for long, so
    // only hold the key and give the ROM time to draw its banner.
    actions.push(Action::Down(Key::Enter));
    actions.push(Action::Wait {
        ms: DEFAULT_HOLD_MS,
    });
    actions.push(Action::Up(Key::Enter));
    actions.push(Action::Wait { ms: 1_000 });
    actions
        .into_iter()
        .enumerate()
        .map(|(i, action)| Line {
            number: i + 1,
            action,
        })
        .collect()
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
    Stdio(Receiver<Vec<u8>>, io::Stdout),
}

/// Result of polling a peer for input.
enum Input {
    Bytes(Vec<u8>),
    Nothing,
    Closed,
}

impl Peer {
    fn poll(&mut self) -> io::Result<Input> {
        match self {
            Peer::Tcp(s) => {
                let mut buf = [0u8; 4096];
                match s.read(&mut buf) {
                    Ok(0) => Ok(Input::Closed),
                    Ok(n) => Ok(Input::Bytes(buf[..n].to_vec())),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(Input::Nothing),
                    Err(e) if e.kind() == ErrorKind::Interrupted => Ok(Input::Nothing),
                    Err(e) => Err(e),
                }
            }
            Peer::Stdio(rx, _) => match rx.try_recv() {
                Ok(b) => Ok(Input::Bytes(b)),
                Err(TryRecvError::Empty) => Ok(Input::Nothing),
                Err(TryRecvError::Disconnected) => Ok(Input::Closed),
            },
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
            Peer::Stdio(_, stdout) => {
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
fn stdin_reader() -> Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 4096];
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

/// Run the machine paced to wall-clock time and bridge its serial port
/// until `stop` is set (SIGINT/SIGTERM) or, with `exit_on_disconnect`,
/// until the first client leaves.
pub fn bridge(s: &mut Session, opts: &BridgeOptions<'_>, stop: &Arc<AtomicBool>) -> Result<()> {
    let clock_hz = s.machine.model().clock_hz();
    let cycles_per_ms = f64::from(clock_hz) / 1000.0;
    let mut log = opts.log.map(WireLog::open).transpose()?;
    let listener = match &opts.spec {
        SerialSpec::Tcp(addr) => {
            let l = TcpListener::bind(addr).with_context(|| format!("cannot listen on {addr}"))?;
            l.set_nonblocking(true)
                .context("cannot make the listener non-blocking")?;
            let local = l.local_addr().context("listener has no address")?;
            println!("serial bridged on tcp:{local}");
            io::stdout().flush().context("cannot flush stdout")?;
            Some(l)
        }
        SerialSpec::Stdio => {
            eprintln!("serial bridged on stdio");
            None
        }
    };
    let mut peer = match &opts.spec {
        SerialSpec::Stdio => Some(Peer::Stdio(stdin_reader(), io::stdout())),
        SerialSpec::Tcp(_) => None,
    };
    let mut out: Vec<u8> = Vec::new();
    let mut pacer = Pacer::new(clock_hz, Instant::now(), s.machine.cycles());
    // Bytes the calculator sends while nobody listens are dropped; the
    // saturnng container's pty keeps them instead, which is the "stale NAK"
    // hptx drains on connect.
    let mut dropped = 0usize;
    while !stop.load(Ordering::Relaxed) {
        if peer.is_none() {
            if let Some(l) = &listener {
                match l.accept() {
                    Ok((stream, addr)) => {
                        stream.set_nodelay(true).context("cannot set TCP_NODELAY")?;
                        stream
                            .set_nonblocking(true)
                            .context("cannot make the client socket non-blocking")?;
                        if opts.verbose {
                            eprintln!("serial: client {addr} connected");
                        }
                        if let Some(l) = log.as_mut() {
                            l.note(s.machine.cycles() as f64 / cycles_per_ms, "connect")?;
                        }
                        peer = Some(Peer::Tcp(stream));
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e).context("accept failed"),
                }
            }
        }
        let mut closed = false;
        if let Some(p) = peer.as_mut() {
            loop {
                match p.poll() {
                    Ok(Input::Bytes(b)) => {
                        if let Some(l) = log.as_mut() {
                            l.record(s.machine.cycles() as f64 / cycles_per_ms, '>', &b)?;
                        }
                        s.machine.serial_push(&b);
                    }
                    Ok(Input::Nothing) => break,
                    Ok(Input::Closed) => {
                        closed = true;
                        break;
                    }
                    Err(e) => {
                        if opts.verbose {
                            eprintln!("serial: read error {e}, dropping client");
                        }
                        closed = true;
                        break;
                    }
                }
            }
        }

        let n = pacer.budget(Instant::now(), s.machine.cycles());
        if n > 0 {
            s.run(n)?;
        }

        let tx = s.machine.serial_drain();
        if !tx.is_empty() {
            if let Some(l) = log.as_mut() {
                l.record(s.machine.cycles() as f64 / cycles_per_ms, '<', &tx)?;
            }
            if peer.is_some() && !closed {
                out.extend_from_slice(&tx);
            } else {
                dropped += tx.len();
            }
        }
        if let Some(p) = peer.as_mut() {
            if !closed && let Err(e) = p.send(&mut out) {
                if opts.verbose {
                    eprintln!("serial: write error {e}, dropping client");
                }
                closed = true;
            }
        }
        if closed {
            peer = None;
            out.clear();
            if opts.verbose {
                eprintln!(
                    "serial: client disconnected at cycle {} ({} undelivered bytes)",
                    s.machine.cycles(),
                    s.machine.serial_pending()
                );
            }
            if let Some(l) = log.as_mut() {
                l.note(s.machine.cycles() as f64 / cycles_per_ms, "disconnect")?;
            }
            if opts.exit_on_disconnect {
                break;
            }
        }
        if n == 0 {
            std::thread::sleep(IDLE_SLEEP);
        }
    }
    if opts.verbose {
        eprintln!(
            "serial: stopped at cycle {}; {} bytes sent with no client; re-anchored {} times",
            s.machine.cycles(),
            dropped,
            pacer.rebases
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn pacer_follows_wall_clock_and_rebases_long_lags() {
        let t0 = Instant::now();
        let mut p = Pacer::new(2_000_000, t0, 1000);
        // Nothing elapsed: ahead, sleep.
        assert_eq!(p.budget(t0, 1000), 0);
        // 0.5 ms behind: run exactly that.
        assert_eq!(p.budget(t0 + Duration::from_micros(500), 1000), 1000);
        // 50 ms behind: at most one slice per call.
        assert_eq!(p.budget(t0 + Duration::from_millis(50), 1000), 2000);
        // Ahead of the clock: sleep.
        assert_eq!(p.budget(t0 + Duration::from_millis(1), 10_000), 0);
        assert_eq!(p.rebases, 0);
        // A second behind: re-anchor, then follow the clock from there.
        let t1 = t0 + Duration::from_secs(1);
        assert_eq!(p.budget(t1, 1000), 2000);
        assert_eq!(p.rebases, 1);
        assert_eq!(p.budget(t1 + Duration::from_micros(250), 1000), 500);
    }

    #[test]
    fn autostart_types_server() {
        let keys: Vec<Key> = autostart_script(true)
            .iter()
            .filter_map(|l| match l.action {
                Action::Press { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(
            keys,
            [
                Key::F,
                Key::Alpha,
                Key::Alpha,
                Key::Sin,
                Key::E,
                Key::Right,
                Key::Sqrt,
                Key::E,
                Key::Right
            ]
        );
        assert!(!autostart_script(false).iter().any(|l| l.action
            == Action::Press {
                key: Key::F,
                hold_ms: DEFAULT_HOLD_MS
            }));
    }
}
