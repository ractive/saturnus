//! The Kermit client side of the ROM's server: the products' Kermit client
//! ([`saturnus_host::transfer::Exchange`], what the engine's hidden writes
//! run) over a [`Link`], with the calculator operations a host needs on
//! top: host commands, listings, the current directory, GET, SEND and
//! `G F`. The replies are parsed with satx-core's parsers. wiki:
//! protocols/server-commands, protocols/kermit.
//!
//! A host command returns the whole stack as display text and leaves its
//! results on the user's stack; a failed command replies `Error: X` (no E
//! packet) and leaves its arguments there.

use anyhow::{Context, Result, anyhow, bail};
use kermit_proto::{Command, OutgoingFile};
use saturnus_host::transfer::Transcript;
use saturnus_objects::transfer::is_plain_name;
use satx_core::charset::{decode, encode, encode_command};
use satx_core::object::{KERMIT_PADDING_ALLOWANCE, strip_padding};
use satx_core::reply::{Listing, StackReply, parse_list, parse_listing, parse_stack};

use crate::link::Link;

pub use satx_core::TransferMode;

/// The ROM's Kermit server as a host sees it, over a [`Link`].
#[derive(Debug)]
pub struct Server<'a> {
    link: &'a mut Link,
    /// The transfer mode (flag -35) last set, kept by the caller across
    /// calls while the server runs; `None` when not known.
    mode: &'a mut Option<TransferMode>,
}

impl<'a> Server<'a> {
    /// The server on `link`; `mode` caches the transfer mode last set
    /// (`None` at first), kept by the caller across calls.
    pub fn new(link: &'a mut Link, mode: &'a mut Option<TransferMode>) -> Self {
        Server { link, mode }
    }

    /// Run one transaction; a failed one (an E packet, a timeout) is an
    /// error.
    fn transact(&mut self, command: Command) -> Result<Transcript> {
        self.link
            .transact(command)?
            .map_err(|f| anyhow!("{}", f.message()))
    }

    /// A host command's stack reply.
    fn host(&mut self, command: &str) -> Result<StackReply> {
        let transcript = self.transact(Command::Host(encode_command(command)?))?;
        Ok(parse_stack(&decode(&transcript.text)))
    }

    /// [`Server::host`], with an `Error:` reply as an error.
    fn exec(&mut self, command: &str) -> Result<StackReply> {
        let reply = self.host(command)?;
        if let Some(e) = &reply.error {
            bail!("calculator error: {e}");
        }
        Ok(reply)
    }

    /// Set flag -35 for `mode` unless it is known to be set so.
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

    /// Run a host command (Unicode, or ASCII with trigraphs such as `\->`)
    /// and return the stack. An `Error:` reply is `Ok` with `error` set.
    /// Forgets the cached transfer mode, since the command may change
    /// flag -35.
    pub fn run(&mut self, command: &str) -> Result<StackReply> {
        *self.mode = None;
        self.host(command)
    }

    /// The current directory listing (`G D`).
    pub fn list(&mut self) -> Result<Listing> {
        let transcript = self.transact(Command::Directory)?;
        Ok(parse_listing(&decode(&transcript.text))?)
    }

    /// The current directory, e.g. `["HOME", "D1"]`: from the listing's
    /// header (48GX, 49G) or `PATH` (48SX, whose listing has none).
    pub fn path(&mut self) -> Result<Vec<String>> {
        if let Some(path) = self.list()?.path {
            return Ok(path);
        }
        let value = self
            .exec("PATH")?
            .levels
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("PATH: no value"))?;
        self.exec("DROP")?;
        parse_list(&value).ok_or_else(|| anyhow!("PATH: not a list: {value:?}"))
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
                .any(|e| e.name == *name && e.is_directory());
            if !is_dir {
                bail!("{name}: no such directory");
            }
            self.exec(name)?;
        }
        Ok(())
    }

    /// GET variable `name` of the current directory in `mode`. Binary data
    /// comes without the calculator's padding of the last packet.
    pub fn get(&mut self, name: &str, mode: TransferMode) -> Result<Vec<u8>> {
        check_name(name)?;
        let get = |s: &mut Self| -> Result<Vec<u8>> {
            s.set_mode(mode)?;
            let transcript = s.transact(Command::Get(encode(name)?))?;
            let [data]: [Vec<u8>; 1] = transcript
                .files
                .try_into()
                .map_err(|files: Vec<_>| anyhow!("expected one file, got {}", files.len()))?;
            Ok(match mode {
                TransferMode::Binary => strip_padding(&data, KERMIT_PADDING_ALLOWANCE).to_vec(),
                TransferMode::Ascii => data,
            })
        };
        get(self).with_context(|| format!("GET {name}"))
    }

    /// SEND `data` as variable `name` in `mode`; returns the name the
    /// calculator stored it under (a `.1` suffix when the name exists).
    pub fn put(&mut self, name: &str, data: &[u8], mode: TransferMode) -> Result<String> {
        check_name(name)?;
        let put = |s: &mut Self| -> Result<String> {
            s.set_mode(mode)?;
            let file = OutgoingFile {
                name: encode(name)?,
                data: data.to_vec(),
            };
            s.transact(Command::Send(vec![file]))?
                .stored
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("no file stored"))
        };
        put(self).with_context(|| format!("SEND {name}"))
    }

    /// End server mode (`G F`).
    pub fn finish(&mut self) -> Result<()> {
        self.transact(Command::Finish).map(|_| ())
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
