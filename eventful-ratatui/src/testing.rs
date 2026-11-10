//! [`Harness`]: drive a component without a terminal, for tests.
//!
//! The harness renders into ratatui's `TestBackend` and routes input exactly
//! like the loop does: focus, bubbling, mouse routing, overlays and Tab
//! traversal. Everything is synchronous: each input is handled and the frame
//! redrawn before the call returns. Events emitted through eventful-rs are
//! queued as usual and only delivered when a shard runs; test those inside a
//! shard, with [`Tui::run_with`](crate::Tui::run_with).
//!
//! ```
//! use eventful_ratatui::{prelude::*, testing::Harness};
//! use std::cell::Cell;
//!
//! struct Counter(Cell<u32>);
//! impl Component for Counter {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         format!("count: {}", self.0.get()).render(area, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         let InputEvent::Key(key) = event else { return Handled::No };
//!         if key.code != KeyCode::Char('j') { return Handled::No }
//!         self.0.set(self.0.get() + 1);
//!         invalidate::invalidate();
//!         Handled::Yes
//!     }
//! }
//!
//! let mut ui = Harness::new(Counter(Cell::new(0)), 10, 1);
//! ui.key("j").key("j");
//! assert_eq!(ui.lines(), ["count: 2"]);
//! ```
//!
//! Only one harness or loop may run per thread at a time; the test runner
//! gives every test its own thread.
use crate::{
    Component, KeyChord,
    dispatch::{Dispatcher, Session},
    invalidate,
    theme::{self, Theme},
};
use crossterm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use ratatui_core::{buffer::Buffer, layout::Position};
use std::fmt;

/// Renders a component into a test buffer and feeds it input. See the
/// [module docs](self).
///
/// Input methods take chords in [`KeyChord`] syntax and screen cells as
/// `column, row`, and return `&mut Self`, so steps chain. Read the result
/// with [`lines`](Self::lines), [`text`](Self::text),
/// [`buffer`](Self::buffer) for styles, or [`cursor`](Self::cursor).
///
/// ```
/// use eventful_ratatui::{components::TextInput, prelude::*, testing::Harness};
///
/// let mut ui = Harness::new(TextInput::new().placeholder("Name"), 12, 1);
/// ui.click(0, 0).type_text("Ada").key("backspace").paste("da");
/// assert_eq!(ui.root().text(), "Adda");
/// // After the four characters and the input's one-cell padding.
/// assert_eq!(ui.cursor(), Some(Position::new(5, 0)));
/// ```
///
/// The harness theme is [`Theme::default`]; switch with
/// [`theme`](Self::theme).
pub struct Harness<C: Component + 'static> {
    root: C,
    terminal: Terminal<TestBackend>,
    dispatcher: Dispatcher,
    cursor: Option<Position>,
    renders: usize,
    // Dropped last: resets this thread's focus, overlays and render tree.
    _session: Session,
}

impl<C: Component + 'static> Harness<C> {
    /// Render `root` on a `width` × `height` screen, with mouse routing on.
    #[expect(clippy::missing_panics_doc, reason = "the test backend cannot fail")]
    pub fn new(root: C, width: u16, height: u16) -> Self {
        let session = Session::start();
        let mut harness = Self {
            root,
            terminal: Terminal::new(TestBackend::new(width, height))
                .expect("the test backend cannot fail"),
            dispatcher: Dispatcher::new(true),
            cursor: None,
            renders: 0,
            _session: session,
        };
        harness.render();
        harness
    }

    /// Switch to `theme` and redraw.
    pub fn theme(&mut self, theme: Theme) -> &mut Self {
        theme::set(theme);
        self.settle()
    }

    /// The component under test.
    pub fn root(&self) -> &C {
        &self.root
    }

    /// Route `event` as the loop would, then redraw if anything asked for
    /// it. The other input methods build on this one.
    pub fn send(&mut self, event: Event) -> &mut Self {
        self.dispatcher.handle(&self.root, &event);
        self.settle()
    }

    /// Press the key chord `chord`, like `"j"`, `"ctrl+s"` or `"shift+tab"`.
    ///
    /// # Panics
    /// Panics if `chord` is not a valid [`KeyChord`].
    pub fn key(&mut self, chord: &str) -> &mut Self {
        let chord: KeyChord = chord.parse().unwrap_or_else(|err| panic!("{err}"));
        self.send(Event::Key(KeyEvent::new(chord.code(), chord.modifiers())))
    }

    /// Press each chord in turn.
    pub fn keys(&mut self, chords: &[&str]) -> &mut Self {
        for chord in chords {
            self.key(chord);
        }
        self
    }

    /// Type `text`: one `KeyCode::Char` press per `char`, without modifiers.
    /// Use [`key`](Self::key) for Enter and other named keys.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        for c in text.chars() {
            let code = crossterm::event::KeyCode::Char(c);
            self.send(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        }
        self
    }

    /// Paste `text` in one event, as bracketed paste does.
    pub fn paste(&mut self, text: &str) -> &mut Self {
        self.send(Event::Paste(text.to_owned()))
    }

    /// Send a mouse event of `kind` at `column`, `row`.
    pub fn mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) -> &mut Self {
        self.send(Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
    }

    /// Move the pointer to `column`, `row` without a button, for hover.
    pub fn hover(&mut self, column: u16, row: u16) -> &mut Self {
        self.mouse(MouseEventKind::Moved, column, row)
    }

    /// Click the left button at `column`, `row`.
    pub fn click(&mut self, column: u16, row: u16) -> &mut Self {
        self.mouse(MouseEventKind::Down(MouseButton::Left), column, row)
            .mouse(MouseEventKind::Up(MouseButton::Left), column, row)
    }

    /// Drag with the left button from one cell to another.
    pub fn drag(&mut self, from: (u16, u16), to: (u16, u16)) -> &mut Self {
        self.mouse(MouseEventKind::Down(MouseButton::Left), from.0, from.1)
            .mouse(MouseEventKind::Drag(MouseButton::Left), to.0, to.1)
            .mouse(MouseEventKind::Up(MouseButton::Left), to.0, to.1)
    }

    /// Scroll the wheel down (`lines` > 0) or up at `column`, `row`.
    pub fn scroll(&mut self, column: u16, row: u16, lines: i32) -> &mut Self {
        let kind = if lines > 0 {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        };
        for _ in 0..lines.unsigned_abs() {
            self.mouse(kind, column, row);
        }
        self
    }

    /// Resize the screen and redraw.
    pub fn resize(&mut self, width: u16, height: u16) -> &mut Self {
        self.terminal.backend_mut().resize(width, height);
        self.send(Event::Resize(width, height))
    }

    /// Fire pending [`invalidate::after`] timers now and redraw.
    pub fn fire_timers(&mut self) -> &mut Self {
        invalidate::fire_deadline();
        self.settle()
    }

    /// Draw a frame, whether or not one was requested.
    #[expect(clippy::missing_panics_doc, reason = "the test backend cannot fail")]
    pub fn render(&mut self) -> &mut Self {
        // Focus moves after a frame can ask for another; a few rounds settle
        // it, while a component that invalidates on every render cannot hang
        // the test.
        for _ in 0..4 {
            self.cursor = self
                .dispatcher
                .draw(&mut self.terminal, &self.root)
                .expect("the test backend cannot fail");
            self.renders += 1;
            if !invalidate::is_dirty() {
                break;
            }
        }
        self
    }

    /// Redraw if requested.
    fn settle(&mut self) -> &mut Self {
        if invalidate::is_dirty() {
            self.render();
        }
        self
    }

    /// The last frame.
    pub fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    /// The last frame as text, one string per row, trailing spaces trimmed.
    pub fn lines(&self) -> Vec<String> {
        let buffer = self.buffer();
        let area = buffer.area;
        (area.top()..area.bottom())
            .map(|y| {
                let mut line = String::new();
                for x in area.left()..area.right() {
                    line.push_str(buffer[(x, y)].symbol());
                }
                line.trim_end().to_owned()
            })
            .collect()
    }

    /// The last frame as one string, rows joined by newlines.
    pub fn text(&self) -> String {
        self.lines().join("\n")
    }

    /// Where the last frame showed the cursor, if anywhere.
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }

    /// How many frames were drawn.
    pub fn renders(&self) -> usize {
        self.renders
    }
}

impl<C: Component + 'static> fmt::Debug for Harness<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Harness")
            .field("renders", &self.renders)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}
