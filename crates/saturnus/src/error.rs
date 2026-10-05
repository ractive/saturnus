//! Errors returned by the core library.

use std::fmt;

/// Something the caller handed the library was unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The ROM image has the wrong size for the model.
    RomSize {
        /// Size the model needs, in bytes.
        expected: usize,
        /// Size that was given, in bytes.
        actual: usize,
    },
    /// A card image is not a power of two between the minimum and maximum
    /// card size.
    CardSize {
        /// Size that was given, in bytes.
        actual: usize,
    },
    /// A saved state could not be parsed.
    InvalidState {
        /// What was wrong.
        reason: &'static str,
        /// Byte offset in the state where the problem was found.
        offset: usize,
    },
    /// A saved state belongs to another model.
    StateModelMismatch,
    /// A saved state was made with a different system ROM.
    StateRomMismatch,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::RomSize { expected, actual } => write!(
                f,
                "ROM image is {actual} bytes, this model needs {expected} bytes"
            ),
            Error::CardSize { actual } => write!(
                f,
                "card image is {actual} bytes, cards must be a power of two from 1 KB to 128 KB"
            ),
            Error::InvalidState { reason, offset } => {
                write!(f, "invalid saved state at byte {offset}: {reason}")
            }
            Error::StateModelMismatch => write!(f, "saved state is for another model"),
            Error::StateRomMismatch => write!(f, "saved state was made with a different ROM"),
        }
    }
}

impl std::error::Error for Error {}
