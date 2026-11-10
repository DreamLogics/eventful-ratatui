//! [`run_tui`], the [`Tui`] builder and the [`Quit`] handle.
//!
//! The loop is meant to be the `#[sharded_main]` future, so it takes turns
//! with queued calls and event handlers on the main shard. It waits for input,
//! a redraw request or a tick, and draws at most once per frame interval.
//!
//! ```no_run
//! use eventful_ratatui::{prelude::*, theme::ThemeFile};
//! use eventful_rs::*;
//! use std::time::Duration;
//!
//! declare_shard!(Ui, runtime = main);
//!
//! #[eventful(shard = Ui)]
//! struct Clock;
//! impl Component for Clock {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let now = std::time::SystemTime::now()
//!             .duration_since(std::time::UNIX_EPOCH)
//!             .unwrap_or_default();
//!         format!("{}s since the epoch, q quits", now.as_secs()).render(area, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         let InputEvent::Key(key) = event else { return Handled::No };
//!         if key.code != KeyCode::Char('q') {
//!             return Handled::No;
//!         }
//!         quit();
//!         Handled::Yes
//!     }
//! }
//!
//! #[sharded_main(Ui)]
//! async fn main() -> eventful_ratatui::Result<()> {
//!     let clock = Clock::bind_local(Clock { events: Default::default() })?;
//!     Tui::new()
//!         .frame_rate(30)
//!         .tick(Duration::from_secs(1))
//!         .mouse(true)
//!         .theme_file(ThemeFile::open("themes/ocean.crod")?)
//!         .run(&clock)
//!         .await
//! }
//! ```
use crate::{
    Component, Result, TerminalGuard,
    dispatch::{Dispatcher, Session},
    invalidate,
    theme::{self, Appearance, TerminalColors, Theme, ThemeFile},
};
use crossterm::event::{Event, EventStream};
use futures::{FutureExt, Stream, StreamExt, future::poll_fn, task::AtomicWaker};
use futures_timer::Delay;
use ratatui::{Terminal, backend::Backend};
use std::{
    io,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::{Duration, Instant},
};

/// Run `root` full-screen with default settings until [`quit`] is called or
/// input ends. The terminal is restored on return and on panic.
///
/// The same as `Tui::new().run(root)`: 60 frames per second at most, no
/// tick, no mouse capture, and the built-in theme from the terminal's
/// colors. See [`Tui`] to change those, and the [module docs](self) for a
/// complete program.
///
/// # Errors
/// Returns [`Error::Io`](crate::Error::Io) if the terminal cannot be set up
/// or reading input fails, and [`Error::Backend`](crate::Error::Backend) if
/// drawing fails.
pub async fn run_tui<C: Component + ?Sized + 'static>(root: &C) -> Result<()> {
    Tui::new().run(root).await
}

/// Loop settings. Start with [`Tui::new`], then [`run`](Self::run), or
/// [`run_with`](Self::run_with) to run without a real terminal. See the
/// [module docs](self) for an example.
#[derive(Clone, Debug)]
pub struct Tui {
    /// Minimum time between frames.
    frame_interval: Duration,
    /// Time between ticks, if any.
    tick: Option<Duration>,
    /// Route mouse events by hit-testing.
    mouse: bool,
    /// The theme to set when the loop starts.
    theme: ThemeChoice,
}

/// Where the loop's theme comes from.
#[derive(Clone, Debug, Default)]
enum ThemeChoice {
    /// The built-in theme, from the terminal's colors.
    #[default]
    Terminal,
    /// A fixed theme.
    Fixed(Box<Theme>),
    /// The variant of a theme file that matches the terminal.
    File(ThemeFile),
}

/// How long to wait for the terminal to tell its colors.
const QUERY_TIMEOUT: Duration = Duration::from_millis(500);

impl Default for Tui {
    fn default() -> Self {
        Self {
            frame_interval: Duration::from_secs(1) / 60,
            tick: None,
            mouse: false,
            theme: ThemeChoice::Terminal,
        }
    }
}

impl Tui {
    /// Default settings: up to 60 frames per second, no tick, no mouse
    /// capture, and the built-in theme from the terminal's colors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Draw at most `fps` frames per second. Redraw requests in between are
    /// coalesced into the next frame.
    ///
    /// # Panics
    /// Panics if `fps` is zero.
    pub fn frame_rate(mut self, fps: u32) -> Self {
        assert!(fps > 0, "frame rate must be positive");
        self.frame_interval = Duration::from_secs(1) / fps;
        self
    }

    /// Redraw every `interval` even without requests, for clocks and
    /// animations that render from the current time. For a one-off or
    /// irregular redraw, [`invalidate::after`] is
    /// cheaper.
    pub fn tick(mut self, interval: Duration) -> Self {
        self.tick = Some(interval);
        self
    }

    /// Capture the mouse and route its events to the component under the
    /// pointer; see [`crate::mouse`]. Off by default, because capturing the
    /// mouse disables the terminal's own text selection.
    pub fn mouse(mut self, enabled: bool) -> Self {
        self.mouse = enabled;
        self
    }

    /// Draw with `theme`, whatever the terminal's colors. By default the loop
    /// draws with the built-in theme, generated from the terminal's colors;
    /// see [`crate::theme`]. Replaces an earlier [`theme_file`](Self::theme_file).
    /// With a fixed theme, the loop does not query the terminal's colors.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = ThemeChoice::Fixed(Box::new(theme));
        self
    }

    /// Draw with the variant of `file` that matches the terminal's
    /// background: `<dark>` or `<light>`. [`run`](Self::run) fails with
    /// [`Error::Theme`](crate::Error::Theme) if it does not load; the file is
    /// read when the loop starts, not here. Under
    /// [`run_with`](Self::run_with), the dark variant loads. Replaces an
    /// earlier [`theme`](Self::theme).
    pub fn theme_file(mut self, file: ThemeFile) -> Self {
        self.theme = ThemeChoice::File(file);
        self
    }

    /// The theme to start with, given the terminal's colors if known.
    fn resolve_theme(&self, colors: Option<TerminalColors>) -> Result<Option<Theme>> {
        Ok(match &self.theme {
            ThemeChoice::Fixed(theme) => Some((**theme).clone()),
            ThemeChoice::Terminal => colors.map(|colors| Theme::from_terminal(&colors)),
            ThemeChoice::File(file) => {
                let background = colors.unwrap_or_default().background;
                Some(file.load(Appearance::of(background))?)
            }
        })
    }

    /// Take over the terminal and run `root` until [`quit`] is called or
    /// input ends. The terminal is restored on return and on panic.
    ///
    /// First asks the terminal for its colors, for the built-in theme or to
    /// pick a theme file's variant.
    ///
    /// # Errors
    /// Returns [`Error::Theme`](crate::Error::Theme) if a
    /// [theme file](Self::theme_file) does not load,
    /// [`Error::Io`](crate::Error::Io) if the terminal cannot be set up or
    /// reading input fails, and [`Error::Backend`](crate::Error::Backend) if
    /// drawing fails.
    pub async fn run<C: Component + ?Sized + 'static>(mut self, root: &C) -> Result<()> {
        if !matches!(self.theme, ThemeChoice::Fixed(_)) {
            // Before raw mode and the input stream: the answer arrives as input.
            let colors = TerminalColors::query(QUERY_TIMEOUT).unwrap_or_default();
            if let Some(theme) = self.resolve_theme(Some(colors))? {
                self.theme = ThemeChoice::Fixed(Box::new(theme));
            }
        }
        let mut guard = TerminalGuard::enter()?;
        if self.mouse {
            guard.capture_mouse()?;
        }
        self.run_with(guard.terminal(), EventStream::new(), root)
            .await
    }

    /// Run `root` on a given terminal and input stream, without touching the
    /// real terminal. Use a `TestBackend` and a scripted stream to test
    /// components headlessly.
    ///
    /// Returns when [`quit`] is called, or when `input` ends, after drawing
    /// any pending frame. Keys go to the focused component and bubble up to
    /// `root` (see [`crate::focus`]); mouse events go to `root` unless
    /// [`mouse`](Self::mouse) routing is on; resizes trigger a redraw.
    ///
    /// Unlike [`testing::Harness`](crate::testing::Harness), this is the real
    /// loop: it runs as a future, so eventful-rs events are delivered while it
    /// waits. Only one loop can run per process at a time, because [`quit`]
    /// targets the current one.
    ///
    /// ```
    /// use eventful_ratatui::prelude::*;
    /// use futures::{executor::block_on, stream};
    /// use ratatui::backend::TestBackend;
    /// use std::cell::Cell;
    ///
    /// struct Counter(Cell<u32>);
    /// impl Component for Counter {
    ///     fn render(&self, area: Rect, buf: &mut Buffer) {
    ///         format!("{}", self.0.get()).render(area, buf);
    ///     }
    ///     fn on_input(&self, _: &InputEvent) -> Handled {
    ///         self.0.set(self.0.get() + 1);
    ///         invalidate::invalidate();
    ///         Handled::Yes
    ///     }
    /// }
    ///
    /// let mut terminal = Terminal::new(TestBackend::new(4, 1)).expect("cannot fail");
    /// let keys = ["a", "b", "c"].map(|_| Ok(InputEvent::Key(KeyCode::Char('x').into())));
    /// let counter = Counter(Cell::new(0));
    /// // Returns once the scripted input ends.
    /// block_on(Tui::new().run_with(&mut terminal, stream::iter(keys), &counter))?;
    /// assert_eq!(counter.0.get(), 3);
    /// assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "3");
    /// # Ok::<(), eventful_ratatui::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns the error `input` yields, as [`Error::Io`](crate::Error::Io),
    /// [`Error::Backend`](crate::Error::Backend) if drawing fails, and
    /// [`Error::Theme`](crate::Error::Theme) if a theme file does not load.
    pub async fn run_with<B, S, C>(
        self,
        terminal: &mut Terminal<B>,
        mut input: S,
        root: &C,
    ) -> Result<()>
    where
        B: Backend,
        B::Error: Send + Sync + 'static,
        S: Stream<Item = io::Result<Event>> + Unpin,
        C: Component + ?Sized + 'static,
    {
        let quit = QuitScope::enter();
        let _session = Session::start();
        // Without a real terminal there is nothing to ask: theme files load
        // their dark variant, and the built-in theme stays as it is.
        if let Some(theme) = self.resolve_theme(None)? {
            theme::set(theme);
        }
        let mut dispatcher = Dispatcher::new(self.mouse);
        let mut tick = self.tick.map(|interval| (interval, Delay::new(interval)));
        let mut timer = None;
        let mut last_frame: Option<Instant> = None;
        let mut next_frame: Option<Delay> = None;
        invalidate::mark(); // The first frame.
        loop {
            let wake = poll_fn(|cx| {
                if quit.0.poll(cx).is_ready() {
                    return Poll::Ready(Wake::Quit);
                }
                if invalidate::poll_deadline(cx, &mut timer).is_ready() {
                    invalidate::mark();
                }
                // Drawing goes before input, so a flood of input cannot
                // starve it.
                if invalidate::poll_dirty(cx).is_ready() {
                    let due = last_frame.map(|last| last + self.frame_interval);
                    match due {
                        Some(due) if Instant::now() < due => {
                            let delay =
                                next_frame.get_or_insert_with(|| Delay::new(due - Instant::now()));
                            if delay.poll_unpin(cx).is_ready() {
                                return Poll::Ready(Wake::Draw);
                            }
                        }
                        _ => return Poll::Ready(Wake::Draw),
                    }
                }
                if let Poll::Ready(event) = input.poll_next_unpin(cx) {
                    return Poll::Ready(Wake::Input(event));
                }
                if let Some((interval, delay)) = &mut tick
                    && delay.poll_unpin(cx).is_ready()
                {
                    delay.reset(*interval);
                    return Poll::Ready(Wake::Tick);
                }
                Poll::Pending
            })
            .await;
            match wake {
                Wake::Quit => return Ok(()),
                Wake::Draw => {
                    dispatcher.draw(terminal, root)?;
                    last_frame = Some(Instant::now());
                    next_frame = None;
                }
                Wake::Tick => invalidate::mark(),
                Wake::Input(Some(Ok(event))) => dispatcher.handle(root, &event),
                Wake::Input(Some(Err(err))) => return Err(err.into()),
                Wake::Input(None) => {
                    if invalidate::is_dirty() {
                        dispatcher.draw(terminal, root)?;
                    }
                    return Ok(());
                }
            }
        }
    }
}

/// What woke the loop.
enum Wake {
    Quit,
    Draw,
    Input(Option<io::Result<Event>>),
    Tick,
}

/// Shared state of a [`Quit`] handle.
#[derive(Debug, Default)]
struct QuitState {
    /// Quit was requested.
    requested: AtomicBool,
    /// The loop's waker.
    waker: AtomicWaker,
}

/// Ends the running loop. Cloneable and usable from any thread, so it can be
/// passed to worker shards.
///
/// A handle belongs to one loop run: get it with [`Quit::current`] while the
/// loop runs, and it ends that run only. [`quit`] is the shorthand for
/// quitting whatever loop runs now.
///
/// ```no_run
/// # use eventful_ratatui::Quit;
/// // On the UI shard, while the loop runs:
/// let quit = Quit::current().expect("a loop is running");
/// std::thread::spawn(move || {
///     // ... finish background work, then:
///     quit.quit();
/// });
/// ```
#[derive(Clone, Debug)]
pub struct Quit(Arc<QuitState>);

/// The handle of the running loop, if any.
static CURRENT: Mutex<Option<Quit>> = Mutex::new(None);

impl Quit {
    /// The handle of the loop that is running, if any. `None` before the
    /// loop starts and after it returns.
    pub fn current() -> Option<Self> {
        CURRENT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Ask the loop to return. Idempotent, and does nothing once that loop
    /// run has ended. The loop returns `Ok(())` without drawing again.
    pub fn quit(&self) {
        self.0.requested.store(true, Ordering::Release);
        self.0.waker.wake();
    }

    /// Ready once quit was requested.
    fn poll(&self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        self.0.waker.register(cx.waker());
        if self.0.requested.load(Ordering::Acquire) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// End the running loop, from any thread. Does nothing if no loop is running.
/// The loop returns `Ok(())`, which restores the terminal. See [`Quit`] to
/// end one specific run.
pub fn quit() {
    if let Some(quit) = Quit::current() {
        quit.quit();
    }
}

/// Publishes a fresh [`Quit`] as current while a loop runs.
struct QuitScope(Quit, Option<Quit>);

impl QuitScope {
    fn enter() -> Self {
        let quit = Quit(Arc::default());
        let previous = CURRENT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(quit.clone());
        Self(quit, previous)
    }
}

impl Drop for QuitScope {
    fn drop(&mut self) {
        *CURRENT.lock().unwrap_or_else(PoisonError::into_inner) = self.1.take();
    }
}
