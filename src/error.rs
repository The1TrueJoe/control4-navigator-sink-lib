//! Error type for the crate.

use std::fmt;

/// Errors from reading/parsing the relay stream.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O failure on the socket/reader.
    Io(std::io::Error),
    /// A line was not valid NDJSON for a [`crate::Frame`].
    Parse {
        line: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::Parse { line, source } => {
                write!(f, "failed to parse frame ({source}): {line:?}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Parse { source, .. } => Some(source),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
