//! The crate's [`Error`] and [`Result`] types.
use eventful_rs::{InvokeError, ShardError};
use std::{fmt, io};

/// A `Result` whose error is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Failure while running a TUI, or in the eventful-rs calls around it.
///
/// Converts from eventful-rs errors, so a `#[sharded_main]` function returning
/// [`Result`] can use `?` on `spawn`, `bind_local` and tracked emissions.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Terminal setup, restore or input failed.
    Io(io::Error),
    /// The ratatui backend failed to draw.
    Backend(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// A shard call or tracked delivery failed.
    Invoke(InvokeError),
    /// A shard could not be joined or posted to.
    Shard(ShardError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "terminal i/o failed: {err}"),
            Self::Backend(err) => write!(f, "drawing failed: {err}"),
            Self::Invoke(err) => write!(f, "shard call failed: {err}"),
            Self::Shard(err) => write!(f, "shard failed: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Backend(err) => Some(err.as_ref()),
            Self::Invoke(err) => Some(err),
            Self::Shard(err) => Some(err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
impl From<InvokeError> for Error {
    fn from(err: InvokeError) -> Self {
        Self::Invoke(err)
    }
}
impl From<ShardError> for Error {
    fn from(err: ShardError) -> Self {
        Self::Shard(err)
    }
}
