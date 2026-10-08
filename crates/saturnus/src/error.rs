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
    /// A card image is not a power of two between the minimum card size
    /// and the port's maximum.
    CardSize {
        /// Size that was given, in bytes.
        actual: usize,
        /// Largest card the port takes, in bytes.
        max: usize,
    },
    /// The model has no card port with this number.
    NoSuchPort {
        /// The port number.
        port: u8,
    },
    /// No model has this name (see [`Model`](crate::Model)'s `FromStr`).
    UnknownModel {
        /// The name that was given.
        name: String,
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
    /// A key the model's keyboard does not have (e.g. `prg` on the 49G).
    KeyNotOnModel {
        /// The key's script name.
        key: &'static str,
        /// Model name.
        model: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::RomSize { expected, actual } => write!(
                f,
                "ROM image is {actual} bytes, this model needs {expected} bytes"
            ),
            Error::CardSize { actual, max } => write!(
                f,
                "card image is {actual} bytes, cards for this port must be a power of two from 1 KB to {} KB",
                max / 1024
            ),
            Error::NoSuchPort { port } => write!(f, "this model has no card port {port}"),
            Error::UnknownModel { name } => {
                let names: Vec<&str> = crate::Model::ALL.iter().map(|m| m.name()).collect();
                write!(
                    f,
                    "unknown model {name:?}; expected one of {}",
                    names.join(", ")
                )
            }
            Error::InvalidState { reason, offset } => {
                write!(f, "invalid saved state at byte {offset}: {reason}")
            }
            Error::StateModelMismatch => write!(f, "saved state is for another model"),
            Error::StateRomMismatch => write!(f, "saved state was made with a different ROM"),
            Error::KeyNotOnModel { key, model } => {
                write!(f, "key \"{key}\" is not on the {model} keyboard")
            }
        }
    }
}

impl std::error::Error for Error {}
