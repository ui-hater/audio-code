//! Error type used throughout the engine.

use std::fmt;

/// Everything that can go wrong inside the engine.
#[derive(Debug)]
pub enum Error {
    /// A file could not be read.
    Io(std::io::Error),
    /// A sound file could not be decoded.
    Decode(String),
    /// The audio device could not be opened.
    Audio(String),
    /// The window or event loop could not be created.
    Window(String),
}

/// Shorthand for `std::result::Result<T, audio_code::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Decode(e) => write!(f, "could not decode sound: {e}"),
            Error::Audio(e) => write!(f, "audio device error: {e}"),
            Error::Window(e) => write!(f, "window error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
