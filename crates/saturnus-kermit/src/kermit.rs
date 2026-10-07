//! The Kermit client side of the ROM's server: `kermit-proto`'s state
//! machine driven over a [`Link`], and what a host does with it (host
//! commands, `G D` listings, GET and SEND, `G F`). wiki:
//! protocols/server-commands, protocols/kermit.
//!
//! A host command returns the whole stack as display text and leaves its
//! results on the user's stack; a failed command replies `Error: X` (no E
//! packet) and leaves its arguments there.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use kermit_proto::{Client, Command, Config, Event, OutgoingFile};
use saturnus_objects::charset;
use saturnus_objects::transfer::is_plain_name;

use crate::link::Link;
use crate::reply::{Listing, StackReply, parse_list, parse_listing, parse_stack};

/// Reply timeout per packet, on the link's clock (idle emulated time):
/// a calculator that left server mode fails a command in seconds.
const TIMEOUT: Duration = Duration::from_secs(6);
/// Retransmissions per packet.
const RETRIES: u32 = 3;
/// Clock advance while a transaction runs but the client has no deadline.
const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Shortest read wait.
const MIN_WAIT: Duration = Duration::from_millis(10);

/// Kermit transfer format, flag -35 on the calculator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferMode {
    /// Flag -35 set: GET returns `HPHP48-x` / `HPHP49-x` and the object;
    /// SEND of other data stores a string.
    Binary,
    /// Flag -35 clear: objects travel as `%%HP:` text.
    Ascii,
}

/// What one transaction produced.
#[derive(Debug, Default)]
struct Transcript {
    /// Received files (GET), discarded ones left out.
    files: Vec<Vec<u8>>,
    /// The names the calculator stored sent files under (SEND).
    stored_names: Vec<String>,
    /// Server text (host command and directory replies), HP bytes.
    text: Vec<u8>,
}

/// The client's configuration: `kermit-proto`'s defaults with a shorter
/// timeout and fewer retries, and no linger after a receive (the next
/// command comes after the link's turnaround anyway).
///
/// A host command may be resent ([`Config::first_packet_retries`] stays
/// `None`): the link's clock only advances while the calculator idles, so
/// a `C` goes out again only after the calculator sat idle without
/// answering, which means it did not take the command. The ROMs do NAK a
/// `C` now and then (its idle NAK crossing the packet); with `Some(0)` the
/// resend after `kermit-proto`'s stale-NAK grace fails the transaction.
fn config() -> Config {
    let mut c = Config::default();
    c.timeout = TIMEOUT;
    c.retries = RETRIES;
    c.linger = Duration::ZERO;
    c
}

/// Run one transaction to completion over `link`. Input the idle server
/// sent before it (its periodic NAKs) is dropped first.
fn transact(link: &mut Link, command: Command) -> Result<Transcript> {
    link.discard_input();
    let sending = matches!(command, Command::Send(_));
    let mut client = Client::new(config());
    client
        .start(link.now(), command)
        .map_err(|e| anyhow!("{e}"))?;
    let mut transcript = Transcript::default();
    let mut current: Option<Vec<u8>> = None;
    let mut outcome: Option<Result<()>> = None;
    loop {
        let now = link.now();
        while let Some(packet) = client.poll_output(now) {
            link.write_packet(&packet)?;
        }
        while let Some(event) = client.poll_event() {
            match event {
                Event::FileStart { name } if sending => {
                    transcript.stored_names.push(charset::decode(&name));
                }
                Event::FileStart { .. } => current = Some(Vec::new()),
                Event::Data(data) => {
                    if let Some(file) = current.as_mut() {
                        file.extend_from_slice(&data);
                    }
                }
                Event::FileEnd { discarded } => {
                    if let Some(file) = current.take()
                        && !discarded
                    {
                        transcript.files.push(file);
                    }
                }
                Event::ServerText(text) => transcript.text.extend_from_slice(&text),
                Event::Done => outcome = Some(Ok(())),
                Event::Error(kermit_proto::Error::Remote(text)) => {
                    outcome = Some(Err(anyhow!("calculator error: {}", charset::decode(&text))));
                }
                Event::Error(e) => outcome = Some(Err(anyhow!("Kermit: {e}"))),
                _ => {}
            }
        }
        let deadline = match (client.next_timeout(), &outcome) {
            (None, Some(_)) => break,
            (Some(t), _) => t.max(now + MIN_WAIT),
            (None, None) => now + IDLE_WAIT,
        };
        let input = link.read(deadline)?;
        let now = link.now();
        if !input.is_empty() {
            client.handle_input(now, &input);
        }
        client.handle_timeout(now);
    }
    match outcome {
        Some(Err(e)) => Err(e),
        _ => Ok(transcript),
    }
}

/// The ROM's Kermit server as a host sees it, over a [`Link`].
#[derive(Debug)]
pub struct Server<'a> {
    link: &'a mut Link,
    mode: &'a mut Option<TransferMode>,
}

impl<'a> Server<'a> {
    /// The server on `link`; `mode` caches the transfer mode last set or
    /// read (`None`: unknown), kept by the caller across calls.
    pub fn new(link: &'a mut Link, mode: &'a mut Option<TransferMode>) -> Self {
        Server { link, mode }
    }

    /// Run a host command (Unicode, or ASCII with trigraphs such as `\->`)
    /// and return the stack. An `Error:` reply is `Ok` with `error` set.
    /// Forgets the cached transfer mode, since the command may change
    /// flag -35.
    pub fn run(&mut self, command: &str) -> Result<StackReply> {
        *self.mode = None;
        self.host(command)
    }

    fn host(&mut self, command: &str) -> Result<StackReply> {
        let bytes = charset::encode_command(command)
            .map_err(|c| anyhow!("{c:?} is not in the HP character set"))?;
        let t = transact(self.link, Command::Host(bytes))?;
        Ok(parse_stack(&charset::decode(&t.text)))
    }

    /// Run `command` and fail on a calculator error.
    fn exec(&mut self, command: &str) -> Result<StackReply> {
        let reply = self.host(command)?;
        match reply.error {
            Some(e) => bail!("{command}: calculator error: {e}"),
            None => Ok(reply),
        }
    }

    /// The current directory listing (`G D`).
    pub fn list(&mut self) -> Result<Listing> {
        let t = transact(self.link, Command::Directory)?;
        parse_listing(&charset::decode(&t.text))
    }

    /// The current directory, e.g. `["HOME", "D1"]`: from the listing's
    /// header (48GX, 49G) or `PATH` (48SX, whose listing has none).
    pub fn path(&mut self) -> Result<Vec<String>> {
        if let Some(path) = self.list()?.path {
            return Ok(path);
        }
        let reply = self.exec("PATH")?;
        let value = reply.level(1).context("PATH: no value")?.to_string();
        self.exec("DROP")?;
        parse_list(&value).with_context(|| format!("PATH: not a list: {value:?}"))
    }

    /// Change to the absolute directory `path` (`["HOME", "D1"]`, the
    /// leading `HOME` optional), each component checked against the
    /// listing before it is evaluated.
    pub fn cd(&mut self, path: &[&str]) -> Result<()> {
        let components = match path.split_first() {
            Some((&"HOME", rest)) => rest,
            _ => path,
        };
        for name in components {
            check_name(name)?;
        }
        self.exec("HOME")?;
        for name in components {
            let is_dir = self
                .list()?
                .entries
                .iter()
                .any(|e| e.name == *name && e.kind == "Directory");
            if !is_dir {
                bail!("{name}: no such directory");
            }
            self.exec(name)?;
        }
        Ok(())
    }

    /// Set the transfer mode (flag -35) unless the cache says it is set.
    fn set_mode(&mut self, mode: TransferMode) -> Result<()> {
        if *self.mode == Some(mode) {
            return Ok(());
        }
        self.exec(match mode {
            TransferMode::Binary => "-35 SF",
            TransferMode::Ascii => "-35 CF",
        })?;
        *self.mode = Some(mode);
        Ok(())
    }

    /// GET variable `name` of the current directory in `mode`. Binary data
    /// may end in the calculator's padding of the last packet.
    pub fn get(&mut self, name: &str, mode: TransferMode) -> Result<Vec<u8>> {
        check_name(name)?;
        self.set_mode(mode)?;
        let t = transact(self.link, Command::Get(encode(name)?))
            .with_context(|| format!("GET {name}"))?;
        let [file]: [Vec<u8>; 1] = t
            .files
            .try_into()
            .map_err(|f: Vec<_>| anyhow!("GET {name}: {} files came back, not one", f.len()))?;
        Ok(file)
    }

    /// SEND `data` as variable `name` in `mode`; returns the name the
    /// calculator stored it under (a `.1` suffix when the name exists).
    pub fn put(&mut self, name: &str, data: &[u8], mode: TransferMode) -> Result<String> {
        check_name(name)?;
        self.set_mode(mode)?;
        let file = OutgoingFile {
            name: encode(name)?,
            data: data.to_vec(),
        };
        let t = transact(self.link, Command::Send(vec![file]))
            .with_context(|| format!("SEND {name}"))?;
        t.stored_names
            .into_iter()
            .next()
            .with_context(|| format!("SEND {name}: no file stored"))
    }

    /// End server mode (`G F`).
    pub fn finish(&mut self) -> Result<()> {
        transact(self.link, Command::Finish).map(|_| ())
    }
}

fn encode(name: &str) -> Result<Vec<u8>> {
    charset::encode(name).map_err(|c| anyhow!("{c:?} is not in the HP character set"))
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
