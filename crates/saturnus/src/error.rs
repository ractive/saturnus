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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::RomSize { expected, actual } => write!(
                f,
                "ROM image is {actual} bytes, this model needs {expected} bytes"
            ),
        }
    }
}

impl std::error::Error for Error {}
