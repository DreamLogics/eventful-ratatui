//! [`TerminalGuard`] and the panic hook.
//!
//! `ratatui::init()` installs a hook that restores the terminal on any panic,
//! including callback panics that eventful-rs isolates while the app keeps
//! running. This module installs its own hook instead. While a guard is alive:
//!
//! - **unwind builds:** the hook only records the message. A panic that
//!   unwinds the main future drops the guard, which restores the terminal.
//!   Isolated callback panics leave the UI intact.
//! - **`panic = "abort"` builds:** the hook restores the terminal, because no
//!   unwinding will run the guard.
//!
//! The guard also routes eventful-rs callback-panic reports away from stderr.
//! When it drops, it prints the recorded messages after restoring the
//! terminal, so they are not lost. Outside a guard, panics are reported as
//! usual.
use crossterm::{
    cursor::Show,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Terminal, backend::CrosstermBackend};
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    io::{self, stdout},
    mem,
    panic::{self, PanicHookInfo},
    sync::{Mutex, Once, PoisonError},
    thread,
};

/// Panic messages recorded while a guard is alive.
#[derive(Default)]
struct Log {
    /// A guard owns the terminal.
    active: bool,
    /// Messages to print once the terminal is restored.
    entries: Vec<String>,
}

static LOG: Mutex<Log> = Mutex::new(Log {
    active: false,
    entries: Vec::new(),
});

/// Lock the log, ignoring poison: a panic while it is held must not hide
/// later panics.
fn log() -> std::sync::MutexGuard<'static, Log> {
    LOG.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Owns the terminal in raw mode on the alternate screen, and restores it on
/// drop, including while a panic unwinds.
///
/// [`crate::run_tui`] creates one; use it directly to drive your own loop.
/// Only one guard should exist at a time. See the [module docs](self) for how
/// it handles panics.
///
/// ```no_run
/// use eventful_ratatui::{TerminalGuard, prelude::*};
///
/// # fn main() -> std::io::Result<()> {
/// let mut guard = TerminalGuard::enter()?;
/// guard.terminal().draw(|frame| {
///     frame.render_widget("Hello! Press any key.", frame.area());
/// })?;
/// let _ = crossterm::event::read()?;
/// // Dropping the guard restores the terminal.
/// # Ok(())
/// # }
/// ```
pub struct TerminalGuard {
    /// The terminal drawn to. Dropped before the terminal is restored.
    terminal: DefaultTerminal,
    /// Restores the terminal and flushes the log on drop.
    _active: Active,
}

impl TerminalGuard {
    /// Install the panic hook, enable raw mode and enter the alternate screen.
    ///
    /// The panic hook is installed once per process and stays; outside a
    /// guard it defers to the previous hook.
    ///
    /// # Errors
    /// Returns the I/O error if raw mode cannot be enabled, the alternate
    /// screen cannot be entered, or the terminal size cannot be read, as when
    /// output is not a terminal. Any partial setup is undone.
    pub fn enter() -> io::Result<Self> {
        install_hooks();
        let active = Active::start();
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(Self {
            terminal,
            _active: active,
        })
    }

    /// Report mouse events. Capture ends when the guard restores the
    /// terminal. While captured, the terminal's own text selection needs a
    /// modifier, usually Shift.
    ///
    /// # Errors
    /// Returns the I/O error if the terminal cannot be written to.
    pub fn capture_mouse(&mut self) -> io::Result<()> {
        execute!(stdout(), EnableMouseCapture)
    }

    /// The terminal to draw to, a ratatui [`DefaultTerminal`] on stdout.
    pub fn terminal(&mut self) -> &mut DefaultTerminal {
        &mut self.terminal
    }
}

impl std::fmt::Debug for TerminalGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalGuard").finish_non_exhaustive()
    }
}

/// Marks the log active; dropping it restores the terminal and flushes the log.
struct Active;

impl Active {
    fn start() -> Self {
        log().active = true;
        Self
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        let _ = restore();
        let entries = {
            let mut log = log();
            log.active = false;
            mem::take(&mut log.entries)
        };
        for entry in entries {
            eprintln!("{entry}");
        }
    }
}

/// Leave raw mode and the alternate screen, release the mouse and show the
/// cursor.
fn restore() -> io::Result<()> {
    // Raw mode first: it has more side effects than the alternate screen.
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)
}

/// Install the panic hook and eventful-rs panic reporter once per process.
/// They are never uninstalled, because `panic::set_hook` panics on a
/// panicking thread, where guards may drop. Outside a guard they defer to the
/// previous behavior.
fn install_hooks() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !log().active {
                return previous(info);
            }
            if cfg!(panic = "abort") {
                let _ = restore();
                return previous(info);
            }
            let entry = describe(info);
            log().entries.push(entry);
        }));
        eventful_rs::set_panic_reporter(|panic| {
            // The panic hook already recorded the message.
            if !log().active {
                eprintln!("eventful-rs: {panic}");
            }
        });
    });
}

/// Format a panic the way the default hook does.
fn describe(info: &PanicHookInfo<'_>) -> String {
    let thread = thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let message = info.payload_as_str().unwrap_or("Box<dyn Any>").to_owned();
    let location = info
        .location()
        .map(|location| format!(" at {location}"))
        .unwrap_or_default();
    let mut entry = format!("thread '{name}' panicked{location}:\n{message}");
    let backtrace = Backtrace::capture();
    if backtrace.status() == BacktraceStatus::Captured {
        entry.push_str(&format!("\nstack backtrace:\n{backtrace}"));
    }
    entry
}
