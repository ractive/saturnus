//! The error of the [`Emulator`](crate::Emulator)'s operations: a message
//! meant for the user, which every front end shows as it is.

/// What went wrong, as a message for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

/// The [`Emulator`](crate::Emulator)'s result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// The message.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self(message.to_string())
    }
}

/// For hosts whose own errors are messages (the native runner).
impl From<Error> for String {
    fn from(e: Error) -> Self {
        e.0
    }
}
