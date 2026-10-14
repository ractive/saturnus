//! The crate's errors: [`DecodeError`] where the nibbles of an object do
//! not make one (with the offset where decoding failed and a [`Reason`]),
//! and [`Error`] for everything else (no memory view on this model, memory
//! the ROM has not set up, text that is no number, an object that cannot
//! be encoded), wrapping a [`DecodeError`] where one is the cause.
//!
//! [`Error`] displays like the `anyhow` errors it replaced: `{}` shows the
//! outermost message, `{:#}` the whole chain (`stack level 1: object
//! truncated at nibble 5`), and [`std::error::Error::source`] walks it.

use std::fmt;

use saturnus_core::Model;

use crate::prolog::{MAX_DEPTH, ObjectType};

/// Why the nibbles at [`DecodeError::offset`] are not an object.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The object runs past the end of the nibbles: a field starts or
    /// ends beyond them.
    Truncated,
    /// The 5-nibble prolog is in the prolog range but is no known type.
    UnknownProlog(u32),
    /// Objects nest deeper than [`MAX_DEPTH`] levels.
    TooDeep,
    /// A size or position does not fit a `usize`.
    SizeOverflow,
    /// A length field holds less than its own 5 nibbles.
    LengthTooShort(usize),
    /// The object's fields say it is `size` nibbles long, but only `left`
    /// nibbles follow its start.
    DoesNotFit {
        /// Its type.
        kind: ObjectType,
        /// Its size from its fields.
        size: usize,
        /// Nibbles from its start to the end of the input.
        left: usize,
    },
    /// A directory's last-variable offset matches none of its records.
    DirectoryOffset,
    /// A real's body is shorter than 16 nibbles.
    RealTooShort,
    /// A real's or an integer's body holds a nibble above 9; the body as
    /// hex.
    NotBcd {
        /// The type ([`ObjectType::Real`] or [`ObjectType::Integer`]).
        kind: ObjectType,
        /// The body, hex.
        hex: String,
    },
    /// A real's sign nibble is neither 0 nor 9.
    BadSign(u8),
    /// An integer without a single nibble.
    EmptyInteger,
    /// A string's, binary integer's or integer's length field is below 5.
    LengthTooSmall(ObjectType),
    /// A string of an odd number of nibbles.
    OddString,
    /// A unit object whose first element is no real.
    UnitWithoutReal,
    /// An array of more dimensions than the calculator makes (2).
    ArrayDimensions(usize),
    /// An array whose dimensions do not match its length.
    ArrayLength,
    /// More objects than [`crate::MAX_DECODED_OBJECTS`] in one decode.
    TooManyObjects,
    /// More nibbles read than [`crate::MAX_DECODED_NIBBLES`] in one decode.
    TooManyNibbles,
    /// More nibbles cloned than [`crate::MAX_CLONED_NIBBLES`] in one
    /// decode.
    TooManyCloned,
}

/// Nibbles that are not an object, and where they stop being one.
///
/// `offset` is in the coordinates of the function that failed: the nibble
/// index into the slice given to [`crate::decode`], [`crate::prolog`]'s
/// walks, [`crate::Real::from_body`] and [`crate::Integer::from_nibbles`];
/// for an object read from memory ([`crate::decode_at`], a ROM pointer's
/// target) the nibble index from the start of the object at `address`.
/// For the budget reasons ([`Reason::TooManyObjects`] and the like) the
/// offset is 0: the budget covers the whole decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// Nibble offset where decoding failed (see above).
    pub offset: usize,
    /// Why.
    pub reason: Reason,
    /// The memory address of the object `offset` counts in, when it was
    /// read from memory; `None` when `offset` indexes the caller's slice.
    pub address: Option<u32>,
}

impl DecodeError {
    /// The error `reason` at nibble `offset` of the caller's slice.
    pub fn new(offset: usize, reason: Reason) -> Self {
        Self {
            offset,
            reason,
            address: None,
        }
    }

    /// The same error with `offset` counted from `by` nibbles earlier: an
    /// error in a sub-slice that starts at `by`.
    pub(crate) fn shifted(mut self, by: usize) -> Self {
        if self.address.is_none() {
            self.offset = self.offset.saturating_add(by);
        }
        self
    }

    /// The same error in the object read from memory at `addr`, unless it
    /// already names one or is a budget's.
    pub(crate) fn in_memory(mut self, addr: u32) -> Self {
        if self.address.is_none() && !self.is_budget() {
            self.address = Some(addr);
        }
        self
    }

    /// Whether a decode budget ran out (the object is too big, not
    /// malformed).
    pub fn is_budget(&self) -> bool {
        matches!(
            self.reason,
            Reason::TooManyObjects | Reason::TooManyNibbles | Reason::TooManyCloned
        )
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(a) = self.address {
            write!(f, "object at #{a:05X}: ")?;
        }
        let at = self.offset;
        match &self.reason {
            Reason::Truncated => write!(f, "object truncated at nibble {at}"),
            Reason::UnknownProlog(p) => write!(f, "unknown prolog #{p:05X} at nibble {at}"),
            Reason::TooDeep => write!(f, "objects nested deeper than {MAX_DEPTH} levels"),
            Reason::SizeOverflow => f.write_str("object size overflows"),
            Reason::LengthTooShort(len) => write!(
                f,
                "length field #{len:05X} at nibble {at} is shorter than itself"
            ),
            Reason::DoesNotFit { kind, size, left } => write!(
                f,
                "{} at nibble {at} needs {size} nibbles, only {left} left",
                kind.name()
            ),
            Reason::DirectoryOffset => write!(
                f,
                "directory at nibble {at}: the last-variable offset matches no record"
            ),
            Reason::RealTooShort => f.write_str("real body shorter than 16 nibbles"),
            Reason::NotBcd { kind, hex } => {
                let what = if *kind == ObjectType::Integer {
                    "integer"
                } else {
                    "real"
                };
                write!(f, "{what} body is not BCD: {hex}")
            }
            Reason::BadSign(s) => write!(f, "real sign nibble {s} is neither 0 nor 9"),
            Reason::EmptyInteger => f.write_str("empty integer"),
            Reason::LengthTooSmall(kind) => {
                let what = match kind {
                    ObjectType::String => "string",
                    ObjectType::BinaryInteger => "binary",
                    ObjectType::Integer => "integer",
                    other => other.name(),
                };
                write!(f, "{what} length too small")
            }
            Reason::OddString => f.write_str("string of an odd number of nibbles"),
            Reason::UnitWithoutReal => f.write_str("unit does not start with a real"),
            Reason::ArrayDimensions(n) => write!(f, "array of {n} dimensions (at most 2)"),
            Reason::ArrayLength => f.write_str("array dimensions do not match its length"),
            Reason::TooManyObjects => write!(
                f,
                "more than {} objects (MAX_DECODED_OBJECTS) to decode: \
                 pointers that repeat, memory corrupt or not set up",
                crate::MAX_DECODED_OBJECTS
            ),
            Reason::TooManyNibbles => write!(
                f,
                "more than {} nibbles (MAX_DECODED_NIBBLES) to decode: \
                 objects that repeat, memory corrupt or not set up",
                crate::MAX_DECODED_NIBBLES
            ),
            Reason::TooManyCloned => write!(
                f,
                "more than {} nibbles (MAX_CLONED_NIBBLES) of repeated \
                 objects to decode: pointers that repeat, memory corrupt or not set up",
                crate::MAX_CLONED_NIBBLES
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// What went wrong outside the decoder, or a [`DecodeError`] on the way.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Nibbles that are not an object.
    Decode(DecodeError),
    /// The model keeps no RPL user memory this crate reads (the aplet
    /// models, the Pioneers, the 28S, the 19BII).
    NoUserMemory(Model),
    /// The model has no RPL command line this crate reads.
    NoCommandLine(Model),
    /// HOME (at this address) is no directory: the ROM has not set up its
    /// memory yet (not booted, or reset and not idle since).
    NotSetUp {
        /// Where the HOME pointer points.
        home: u32,
    },
    /// The memory at this address cannot be read.
    Unreadable(u32),
    /// Memory that is not as the ROM leaves it (implausible pointers, no
    /// stack end marker, a command line too long), as a message.
    Memory(String),
    /// Text that is no number of the kind asked for, or a number out of
    /// the calculator's range, as a message.
    Parse(String),
    /// An object or value the operation cannot take (one with no text
    /// form, a name too long to encode, a picture of the wrong size), as a
    /// message.
    Invalid(String),
    /// `source` in the step described by `context` (`stack level 2`).
    Context {
        /// What was being done.
        context: String,
        /// What failed.
        source: Box<Error>,
    },
}

impl Error {
    /// `self` in the step `context`.
    pub fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }

    /// The innermost error, under every [`Error::Context`]: match it for
    /// the case (`Error::NotSetUp`, `Error::Decode`, ...).
    pub fn root(&self) -> &Error {
        let mut e = self;
        while let Error::Context { source, .. } = e {
            e = source;
        }
        e
    }

    /// Whether the ROM has not set up its user memory yet.
    pub fn is_not_set_up(&self) -> bool {
        matches!(self.root(), Error::NotSetUp { .. })
    }
}

impl fmt::Display for Error {
    /// The message; with `{:#}` followed by those of every source, joined
    /// with `: ` (as `anyhow` shows a chain).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Decode(e) => e.fmt(f),
            Error::NoUserMemory(model) => {
                let why = if *model == Model::Hp28s {
                    "keeps its RPL memory in a layout this does not read"
                } else if model.is_pioneer() {
                    "has no RPL user memory (no HOME directory, no RPL stack)"
                } else {
                    "keeps aplets, not a HOME directory"
                };
                write!(
                    f,
                    "the {} {why}: no memory view",
                    model.name().to_uppercase()
                )
            }
            Error::NoCommandLine(model) => {
                let what = if *model == Model::Hp28s {
                    "has a command line this cannot read or type into yet"
                } else {
                    "has no RPL command line to read or type into"
                };
                write!(
                    f,
                    "the {} {what} (48SX, 48GX and 49G only)",
                    model.name().to_uppercase()
                )
            }
            Error::NotSetUp { home } => write!(
                f,
                "no directory at HOME (#{home:05X}): the ROM has not set up memory yet"
            ),
            Error::Unreadable(addr) => write!(f, "#{addr:05X} is not readable"),
            Error::Memory(m) | Error::Parse(m) | Error::Invalid(m) => f.write_str(m),
            Error::Context { context, source } => {
                f.write_str(context)?;
                if f.alternate() {
                    write!(f, ": {source:#}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Context { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

/// [`Error::context`] on a result.
pub(crate) trait ResultExt<T> {
    /// The error in the step `context`.
    fn context(self, context: &str) -> Result<T, Error>;
    /// The error in the step `f()` describes.
    fn with_context<C: Into<String>>(self, f: impl FnOnce() -> C) -> Result<T, Error>;
}

impl<T, E: Into<Error>> ResultExt<T> for Result<T, E> {
    fn context(self, context: &str) -> Result<T, Error> {
        self.map_err(|e| e.into().context(context))
    }

    fn with_context<C: Into<String>>(self, f: impl FnOnce() -> C) -> Result<T, Error> {
        self.map_err(|e| e.into().context(f()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{NoMemory, decode};

    fn nib(s: &str) -> Vec<u8> {
        s.chars()
            .map(|c| c.to_digit(16).unwrap_or(0) as u8)
            .collect()
    }

    #[test]
    fn a_truncated_object_says_where() {
        // A string whose length field is cut after 3 of its 5 nibbles.
        let e = crate::prolog::object_size(&nib("C2A20900"), 0).unwrap_err();
        assert_eq!(e, DecodeError::new(5, Reason::Truncated));
        assert_eq!(e.to_string(), "object truncated at nibble 5");
        // A real of 10 nibbles: it needs 21.
        let e = crate::prolog::object_size(&nib("3392000000"), 0).unwrap_err();
        assert_eq!(e.offset, 0);
        assert_eq!(
            e.reason,
            Reason::DoesNotFit {
                kind: ObjectType::Real,
                size: 21,
                left: 10
            }
        );
    }

    #[test]
    fn an_unknown_prolog_says_which_and_where() {
        // A list holding #02700, in the prolog range but no type.
        let e = crate::prolog::object_size(&nib("47A2000720B2130"), 0).unwrap_err();
        assert_eq!(e, DecodeError::new(5, Reason::UnknownProlog(0x02700)));
        assert_eq!(e.to_string(), "unknown prolog #02700 at nibble 5");
    }

    /// Memory holding `hex` from #00100.
    struct Ram(Vec<u8>);

    impl crate::Memory for Ram {
        fn nibble(&self, addr: u32) -> Option<u8> {
            self.0
                .get(usize::try_from(addr.checked_sub(0x100)?).ok()?)
                .copied()
        }
    }

    #[test]
    fn decode_reports_offsets_in_its_input() {
        let e = crate::Real::from_body(&nib("0000000000000005")).unwrap_err();
        assert_eq!(e, DecodeError::new(15, Reason::BadSign(5)));
        let e = crate::Integer::from_nibbles(&[]).unwrap_err();
        assert_eq!(e.reason, Reason::EmptyInteger);
        // A list at #00100 holding a real whose sign nibble is 5: the
        // offset counts from the list's start (prolog 5, real prolog 5,
        // sign 15), not from the real's body.
        let ram = Ram(nib("47A20339200000000000000005B2130"));
        let e = crate::decode_at(0x100, &ram).unwrap_err();
        assert_eq!(
            e,
            DecodeError {
                offset: 25,
                reason: Reason::BadSign(5),
                address: Some(0x100)
            }
        );
        assert_eq!(
            e.to_string(),
            "object at #00100: real sign nibble 5 is neither 0 nor 9"
        );
        // From a slice the decoder keeps what it cannot read as Unknown;
        // only a missing prolog is an error.
        assert!(matches!(
            decode(&nib("47A20339200000000000000005B2130"), &NoMemory),
            Ok(crate::Object::Unknown { .. })
        ));
        assert_eq!(
            decode(&nib("47A"), &NoMemory).unwrap_err(),
            DecodeError::new(0, Reason::Truncated)
        );
    }

    #[test]
    fn error_wraps_decode_errors_and_keeps_its_chain() {
        let d = DecodeError::new(5, Reason::Truncated);
        let e = Error::from(d.clone()).context("stack level 2");
        assert_eq!(e.root(), &Error::Decode(d));
        assert_eq!(e.to_string(), "stack level 2");
        assert_eq!(
            format!("{e:#}"),
            "stack level 2: object truncated at nibble 5"
        );
        let source = std::error::Error::source(&e).map(ToString::to_string);
        assert_eq!(source.as_deref(), Some("object truncated at nibble 5"));
        assert!(!e.is_not_set_up());
    }

    #[test]
    fn not_set_up_is_its_own_case() {
        let e = Error::NotSetUp { home: 0x7FE9B }.context("reading HOME");
        assert!(e.is_not_set_up());
        assert!(matches!(e.root(), Error::NotSetUp { home: 0x7FE9B }));
        assert_eq!(
            format!("{e:#}"),
            "reading HOME: no directory at HOME (#7FE9B): the ROM has not set up memory yet"
        );
    }
}
