//! The error type of `satx-core`.

/// Everything that can go wrong talking to a calculator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The link failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The serial port could not be opened or configured. The variant
    /// exists in every build so that `native` stays additive (a match on
    /// [`Error`] compiles with and without it); only `native` makes one.
    #[error("serial port: {0}")]
    Serial(#[source] SerialError),
    /// The link address is not `tcp://host:port`, `saturnus://[MODEL@]ROM`
    /// or a device path.
    #[error(
        "bad address {0:?}: expected a serial device path, tcp://host:port or saturnus://[MODEL@]ROM-PATH"
    )]
    Address(String),
    /// The in-process emulator could not be started (missing ROM, wrong
    /// size, no `saturnus` feature).
    #[error("emulator: {0}")]
    Emulator(String),
    /// A host command got no reply in time. A `C` packet is sent only once
    /// (resent, it would run again), so the command may still be running
    /// on the calculator or may have run; the next connection resyncs
    /// ([`Calculator::sync`](crate::Calculator::sync)).
    #[error(
        "no reply to host command {command:?}: it may still be running or may have run; the next connection resyncs"
    )]
    NoReply {
        /// The command as given.
        command: String,
    },
    /// The calculator sent a Kermit E packet (text translated from the HP
    /// character set), e.g. `Undefined Name` for a GET of a missing variable.
    #[error("calculator: {0}")]
    Remote(String),
    /// A Kermit failure other than an E packet: retries exhausted, protocol
    /// violation, cancelled.
    #[error("kermit: {0}")]
    Kermit(kermit_proto::Error),
    /// A Kermit transaction made no progress (no new packet accepted) for
    /// longer than its retries take, while the peer kept the link busy
    /// (a packet replayed over and over, noise): it was cancelled. The
    /// duration is the limit that ran out.
    #[error(
        "kermit: no progress for {:.1} s (the line kept sending, but no new packet came); the transfer was cancelled",
        .0.as_secs_f64()
    )]
    Stalled(crate::time::Duration),
    /// An XModem transfer failed: no start within the start window, retries
    /// exhausted, or cancelled by the calculator (two CANs).
    #[error("xmodem: {0}")]
    Xmodem(xmodem_proto::Error),
    /// The calculator cannot do what was asked, e.g. XModem on a 48S/SX.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// A host command ran but the calculator reported an error. `stack` is
    /// what the calculator returned with it, level 1 first.
    #[error("calculator error: {message}")]
    Calculator {
        /// The text after `Error: `, e.g. `Undefined Name`.
        message: String,
        /// Stack levels after the error, level 1 first.
        stack: Vec<String>,
    },
    /// A host command does not fit in one Kermit C packet.
    #[error(
        "host command too long: {len} encoded bytes, at most {max} fit in one packet: {command}"
    )]
    CommandTooLong {
        /// The command as given.
        command: String,
        /// Its encoded length.
        len: usize,
        /// The largest encoded length that fits.
        max: usize,
    },
    /// A character has no code in the HP character set.
    #[error("{0:?} has no code in the HP character set")]
    Charset(char),
    /// Not a valid calculator variable name.
    #[error("invalid variable name {0:?}")]
    Name(String),
    /// A file or folder name whose `%XX` codes do not read back as UTF-8,
    /// so it names no variable or directory ([`names`](crate::names)).
    #[error("file name {0:?}: its %XX codes are not UTF-8")]
    FileName(String),
    /// The calculator's reply could not be understood.
    #[error("unexpected reply: {0}")]
    Reply(String),
    /// A file is not a usable HP object.
    #[error("bad object: {0}")]
    Object(String),
}

/// `Result` with [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// The serial port's error inside [`Error::Serial`]: a `serialport::Error`
/// with the `native` feature; without it no value exists.
#[derive(Debug)]
pub struct SerialError {
    #[cfg(feature = "native")]
    inner: serialport::Error,
    #[cfg(not(feature = "native"))]
    never: std::convert::Infallible,
}

#[cfg(feature = "native")]
impl SerialError {
    /// The `serialport` error.
    pub fn inner(&self) -> &serialport::Error {
        &self.inner
    }
}

impl std::fmt::Display for SerialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "native")]
        return self.inner.fmt(f);
        #[cfg(not(feature = "native"))]
        {
            let _ = f;
            match self.never {}
        }
    }
}

impl std::error::Error for SerialError {}

#[cfg(feature = "native")]
impl From<serialport::Error> for Error {
    fn from(inner: serialport::Error) -> Self {
        Error::Serial(SerialError { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Error::Serial` exists with and without `native` (CI's clippy builds
    /// the tests without default features), so a match stays exhaustive
    /// whichever crate in the graph turns `native` on.
    #[test]
    fn the_serial_variant_exists_in_every_build() {
        let e = Error::Address("x".into());
        assert!(!matches!(e, Error::Serial(_)));
    }

    /// The text of a serial error is as before: the port's message after
    /// `serial port: `.
    #[cfg(feature = "native")]
    #[test]
    fn a_serialport_error_converts() {
        let e = Error::from(serialport::Error::new(
            serialport::ErrorKind::NoDevice,
            "no such port",
        ));
        assert_eq!(e.to_string(), "serial port: no such port");
        assert!(
            matches!(&e, Error::Serial(s) if s.inner().kind() == serialport::ErrorKind::NoDevice)
        );
    }
}
