//! The one error type of this crate's operations. Every front end shows it
//! to the user as it is ([`std::fmt::Display`]); a host that must react
//! to a halted CPU matches [`Error::Halted`] instead of reading the text,
//! and one that waits for the ROM to set up its memory matches
//! [`Error::Objects`] ([`saturnus_objects::Error::is_not_set_up`]).

use saturnus_core::Halt;

/// What went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The CPU met an undefined opcode and stopped; it runs again after a
    /// reset or a state load.
    Halted(Halt),
    /// The core refused a request: a ROM of the wrong size, an unusable
    /// saved state, a key the model lacks, an unknown model name.
    Machine(saturnus_core::Error),
    /// Reading user memory, the command line or an object failed: a model
    /// without a memory view, memory the ROM has not set up, an object
    /// that does not decode or encode. Shown with its whole chain
    /// (`stack level 1: object truncated at nibble 5`).
    Objects(saturnus_objects::Error),
    /// Anything else (an unknown key or verb, user memory the ROM has not
    /// set up, text the model cannot type), as a message for the user.
    Message(String),
}

/// This crate's result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Halted(h) => write!(f, "CPU halted: {h}"),
            Error::Machine(e) => e.fmt(f),
            Error::Objects(e) => write!(f, "{e:#}"),
            Error::Message(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Machine(e) => Some(e),
            // The text already holds the objects error's chain.
            Error::Halted(_) | Error::Objects(_) | Error::Message(_) => None,
        }
    }
}

impl From<Halt> for Error {
    fn from(h: Halt) -> Self {
        Self::Halted(h)
    }
}

impl From<saturnus_core::Error> for Error {
    fn from(e: saturnus_core::Error) -> Self {
        Self::Machine(e)
    }
}

impl From<saturnus_objects::Error> for Error {
    fn from(e: saturnus_objects::Error) -> Self {
        Self::Objects(e)
    }
}

impl From<saturnus_objects::DecodeError> for Error {
    fn from(e: saturnus_objects::DecodeError) -> Self {
        Self::Objects(e.into())
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}
