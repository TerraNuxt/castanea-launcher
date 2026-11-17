//! One error type for the core: what went wrong, with the file or URL it concerns.

use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    Io {
        what: String,
        source: std::io::Error,
    },
    Json {
        what: String,
        message: String,
    },
    Http {
        url: String,
        message: String,
    },
    /// A download or file whose SHA-256 is not the expected one.
    Hash {
        what: String,
        expected: String,
        actual: String,
    },
    /// Refused by the security rules (plain HTTP, an unsafe path, a size over the limit, no checksum).
    Refused(String),
    /// A download whose release signature is missing, malformed, from an unknown key or invalid.
    Signature(String),
    Invalid(String),
}

/// The start of the message for a download without a valid signature; the UI recognises it.
pub const NOT_SIGNED: &str = "this download is not signed by castanea";

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io { what: path.display().to_string(), source }
    }

    pub fn json(what: impl Into<String>, error: impl fmt::Display) -> Self {
        Self::Json { what: what.into(), message: error.to_string() }
    }

    pub fn http(url: &str, error: impl fmt::Display) -> Self {
        Self::Http { url: url.to_owned(), message: error.to_string() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub fn refused(message: impl Into<String>) -> Self {
        Self::Refused(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { what, source } => write!(f, "{what}: {source}"),
            Self::Json { what, message } => write!(f, "{what}: {message}"),
            Self::Http { url, message } => write!(f, "{url}: {message}"),
            Self::Hash { what, expected, actual } => {
                write!(f, "{what}: SHA-256 is {actual}, expected {expected}")
            }
            Self::Refused(message) => write!(f, "refused: {message}"),
            Self::Signature(reason) => write!(f, "{NOT_SIGNED}: {reason}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
