//! The crate's [`Error`] and [`Result`] types.
use eventful_rs::{InvokeError, ShardError};
use std::{fmt, io};

/// A `Result` whose error is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Failure while running a TUI, or in the eventful-rs calls around it.
///
/// Converts from eventful-rs errors, `std::io::Error` and
/// [`ThemeError`](crate::theme::ThemeError), so a `#[sharded_main]` function
/// returning [`Result`] can use `?` on `spawn`, `bind_local`, tracked
/// emissions and theme loading. [`source`](std::error::Error::source)
/// returns the wrapped error.
///
/// ```
/// use eventful_ratatui::Error;
/// use std::io;
///
/// fn setup() -> eventful_ratatui::Result<()> {
///     Err(io::Error::other("no tty"))?
/// }
///
/// let err = setup().unwrap_err();
/// assert!(matches!(err, Error::Io(_)));
/// assert_eq!(err.to_string(), "terminal i/o failed: no tty");
/// ```
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
    /// A theme file could not be loaded.
    Theme(crate::theme::ThemeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "terminal i/o failed: {err}"),
            Self::Backend(err) => write!(f, "drawing failed: {err}"),
            Self::Invoke(err) => write!(f, "shard call failed: {err}"),
            Self::Shard(err) => write!(f, "shard failed: {err}"),
            Self::Theme(err) => err.fmt(f),
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
            Self::Theme(err) => Some(err),
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
impl From<crate::theme::ThemeError> for Error {
    fn from(err: crate::theme::ThemeError) -> Self {
        Self::Theme(err)
    }
}
