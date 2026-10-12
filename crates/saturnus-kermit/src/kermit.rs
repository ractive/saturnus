//! The Kermit client side of the ROM's server: satx-core's sans-I/O
//! [`Machine`] (the calculator operations satx runs on real calculators)
//! driven over a [`Link`] on the link's emulated clock. wiki:
//! protocols/server-commands, protocols/kermit.
//!
//! A host command returns the whole stack as display text and leaves its
//! results on the user's stack; a failed command replies `Error: X` (no E
//! packet) and leaves its arguments there.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use saturnus_objects::transfer::is_plain_name;
use satx_core::Options;
use satx_core::machine::{Machine, Op, Reply};
use satx_core::reply::{Listing, StackReply};

use crate::link::Link;

pub use satx_core::TransferMode;

/// Reply timeout per packet, on the link's clock (idle emulated time):
/// a calculator that left server mode fails a command in seconds.
const TIMEOUT: Duration = Duration::from_secs(6);
/// Retransmissions per packet.
const RETRIES: u32 = 3;
/// Clock advance while an operation runs but the client has no deadline.
const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Shortest read wait: one step of the link.
const MIN_WAIT: Duration = Duration::from_millis(1);

/// The client's options: satx-core's with a shorter timeout and fewer
/// retries, no linger after a receive and no turnaround of its own (the
/// link runs its [`TURNAROUND`](crate::link::TURNAROUND) before every
/// packet that opens a transaction), no drain (nothing connects).
fn options() -> Options {
    let mut o = Options::default();
    o.kermit.timeout = TIMEOUT;
    o.kermit.retries = RETRIES;
    o.kermit.linger = Duration::ZERO;
    o.turnaround = Duration::ZERO;
    o.drain = Duration::ZERO;
    o
}

/// A fresh client on `link`'s clock, no transfer mode cached. The seed
/// only picks sync markers, which these operations never send.
///
/// The link is emulated ([`Machine::set_emulated_link`]): a host command
/// may be resent, since the link's clock only advances while the
/// calculator idles, so a `C` goes out again only after the calculator sat
/// idle without answering, which means it did not take it (the ROMs do NAK
/// a `C` now and then, their idle NAK crossing the packet); and the client
/// reads no stale input before a host command, since [`Server`] drops it
/// without running the calculator (that read, a millisecond of emulated
/// time before each command, left the 49G silent after the `→DATE` of
/// `saturnus-refgen`'s `DATE` example).
pub fn client(link: &Link) -> Machine {
    let mut m = Machine::new(link.now(), options(), 0);
    m.set_emulated_link(true);
    m
}

/// The ROM's Kermit server as a host sees it, over a [`Link`].
#[derive(Debug)]
pub struct Server<'a> {
    link: &'a mut Link,
    client: &'a mut Machine,
}

impl<'a> Server<'a> {
    /// The server on `link`, through `client` (from [`client`]; it caches
    /// the transfer mode last set or read, kept by the caller across
    /// calls).
    pub fn new(link: &'a mut Link, client: &'a mut Machine) -> Self {
        Server { link, client }
    }

    /// Run `op` to completion. Input the idle server sent before it (its
    /// periodic NAKs) is dropped first; a failed link (the read cap)
    /// abandons the operation.
    fn perform(&mut self, op: Op) -> Result<Reply> {
        self.link.discard_input();
        self.client
            .start(self.link.now(), op)
            .map_err(|e| anyhow!("{e}"))?;
        loop {
            while let Some(bytes) = self.client.poll_transmit() {
                if let Err(e) = self.link.write_packet(&bytes) {
                    self.client.abort(self.link.now());
                    return Err(e);
                }
            }
            if let Some(result) = self.client.poll_result() {
                return Ok(result?);
            }
            let now = self.link.now();
            let deadline = match self.client.next_timeout() {
                Some(t) => t.max(now + MIN_WAIT),
                None => now + IDLE_WAIT,
            };
            let input = match self.link.read(deadline) {
                Ok(input) => input,
                Err(e) => {
                    self.client.abort(self.link.now());
                    return Err(e);
                }
            };
            let now = self.link.now();
            if input.is_empty() {
                self.client.handle_timeout(now);
            } else {
                self.client.handle_input(now, &input);
            }
        }
    }

    /// Run a host command (Unicode, or ASCII with trigraphs such as `\->`)
    /// and return the stack. An `Error:` reply is `Ok` with `error` set.
    /// Forgets the cached transfer mode, since the command may change
    /// flag -35.
    pub fn run(&mut self, command: &str) -> Result<StackReply> {
        match self.perform(Op::Run(command.to_string()))? {
            Reply::Stack(reply) => Ok(reply),
            other => bail!("host command: unexpected reply {other:?}"),
        }
    }

    /// The current directory listing (`G D`).
    pub fn list(&mut self) -> Result<Listing> {
        match self.perform(Op::List)? {
            Reply::Listing(listing) => Ok(listing),
            other => bail!("directory: unexpected reply {other:?}"),
        }
    }

    /// The current directory, e.g. `["HOME", "D1"]`: from the listing's
    /// header (48GX, 49G) or `PATH` (48SX, whose listing has none).
    pub fn path(&mut self) -> Result<Vec<String>> {
        match self.perform(Op::Path)? {
            Reply::Path(path) => Ok(path),
            other => bail!("PATH: unexpected reply {other:?}"),
        }
    }

    /// Change to the absolute directory `path` (`["HOME", "D1"]`, the
    /// leading `HOME` optional), each component checked against the
    /// listing before it is evaluated.
    pub fn cd(&mut self, path: &[&str]) -> Result<()> {
        for name in path {
            check_name(name)?;
        }
        let path = path.iter().map(|p| (*p).to_string()).collect();
        self.perform(Op::Cd(path)).map(|_| ())
    }

    /// GET variable `name` of the current directory in `mode`. Binary data
    /// comes without the calculator's padding of the last packet.
    pub fn get(&mut self, name: &str, mode: TransferMode) -> Result<Vec<u8>> {
        check_name(name)?;
        let op = Op::Get {
            name: name.to_string(),
            mode,
        };
        match self.perform(op).with_context(|| format!("GET {name}"))? {
            Reply::Data(data) => Ok(data),
            other => bail!("GET {name}: unexpected reply {other:?}"),
        }
    }

    /// SEND `data` as variable `name` in `mode`; returns the name the
    /// calculator stored it under (a `.1` suffix when the name exists).
    pub fn put(&mut self, name: &str, data: &[u8], mode: TransferMode) -> Result<String> {
        check_name(name)?;
        let op = Op::Put {
            name: name.to_string(),
            data: data.to_vec(),
            mode,
        };
        match self.perform(op).with_context(|| format!("SEND {name}"))? {
            Reply::Stored(stored) => Ok(stored),
            other => bail!("SEND {name}: unexpected reply {other:?}"),
        }
    }

    /// End server mode (`G F`).
    pub fn finish(&mut self) -> Result<()> {
        self.perform(Op::Finish).map(|_| ())
    }
}

/// Refuse a name that is not a plain global variable name.
pub fn check_name(name: &str) -> Result<()> {
    if !is_plain_name(name) {
        bail!(
            "{name:?} is not a plain variable name (1 to 127 characters, no digit or point \
             first, no spaces, delimiters or operators)"
        );
    }
    Ok(())
}
