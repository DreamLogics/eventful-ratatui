//! [`run_tui`], the [`Tui`] builder and the [`Quit`] handle.
//!
//! The loop is meant to be the `#[sharded_main]` future, so it takes turns
//! with queued calls and event handlers on the main shard. It waits for input,
//! a redraw request or a tick, and draws at most once per frame interval.
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
/// # Errors
/// Fails if the terminal cannot be set up, input fails, or drawing fails.
pub async fn run_tui<C: Component + ?Sized + 'static>(root: &C) -> Result<()> {
    Tui::new().run(root).await
}

/// Loop settings. Start with [`Tui::new`], then [`run`](Self::run).
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
    /// Default settings: up to 60 frames per second, no tick.
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
    /// animations that render from the current time.
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
    /// see [`crate::theme`].
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = ThemeChoice::Fixed(Box::new(theme));
        self
    }

    /// Draw with the variant of `file` that matches the terminal's
    /// background: `<dark>` or `<light>`. [`run`](Self::run) fails with
    /// [`Error::Theme`](crate::Error::Theme) if it does not load.
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
    /// Fails if the terminal cannot be set up, a theme file does not load,
    /// input fails, or drawing fails.
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
    /// # Errors
    /// Fails if `input` yields an error or drawing fails.
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
#[derive(Clone, Debug)]
pub struct Quit(Arc<QuitState>);

/// The handle of the running loop, if any.
static CURRENT: Mutex<Option<Quit>> = Mutex::new(None);

impl Quit {
    /// The handle of the loop that is running, if any.
    pub fn current() -> Option<Self> {
        CURRENT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Ask the loop to return. Idempotent.
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
