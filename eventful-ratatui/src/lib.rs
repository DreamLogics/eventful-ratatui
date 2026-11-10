//! Ratatui applications on [eventful-rs](https://github.com/DreamLogics/eventful-rs).
//!
//! Workers on background shards do I/O and computation, while UI components
//! live on the main shard as ordinary `#[eventful]` values. Events flow from
//! workers to components, components request redraws, and one loop handles
//! input and rendering.
//!
//! - [`components`]: ready-made buttons, inputs, lists, tables, trees, tabs,
//!   dialogs, menus, toasts and more, reporting through eventful-rs events.
//! - [`Component`]: the trait to write your own, rendered with ratatui.
//! - [`focus`], [`mouse`] and [`overlay`]: keyboard focus with Tab order,
//!   mouse routing, and layered popups, driven by the frame's render tree.
//! - [`theme`]: a palette from which focus, hover and disabled colors are
//!   derived. The built-in theme is generated from the terminal's own 16
//!   colors; other themes load from CROL files with `<dark>` and `<light>`
//!   variants, and six of those are bundled, like
//!   [`ThemeFile::one`](theme::ThemeFile::one).
//! - [`run_tui`] and [`Tui`]: the loop; [`testing::Harness`] drives
//!   components without a terminal.
//!
//! Applications depend on both `eventful-rs` and `eventful-ratatui`: the
//! eventful-rs macros resolve the runtime through the application's own
//! `eventful-rs` dependency. Both must resolve to the same semver-compatible
//! version, or types like `ShardRc` from the two copies won't match.
//!
//! # Quick start
//!
//! A counter: a label, a button that reports clicks as an event, and a key
//! binding to quit.
//!
//! ```
//! use eventful_ratatui::prelude::*;
//! use eventful_rs::*;
//! use std::cell::Cell;
//!
//! declare_shard!(Ui, runtime = main);
//!
//! #[eventful(shard = Ui)]
//! struct Counter {
//!     count: Cell<u32>,
//!     label: Label,
//!     add: Button,
//!     keys: Keymap<Counter>,
//! }
//!
//! impl Counter {
//!     fn add(&self) {
//!         self.count.set(self.count.get() + 1);
//!         self.label.set_text(format!("Clicked {} times", self.count.get()));
//!     }
//! }
//!
//! impl Component for Counter {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let [label, button] =
//!             Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).areas(area);
//!         self.label.render(label, buf);
//!         self.add.render(button, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         self.keys.dispatch(self, event)
//!     }
//! }
//!
//! #[sharded_main(Ui)]
//! async fn main() -> eventful_ratatui::Result<()> {
//!     let counter = Counter::bind_local(Counter {
//!         count: Cell::new(0),
//!         label: Label::new("Not clicked yet; q quits"),
//!         add: Button::new("Add one").primary(),
//!         keys: Keymap::new().bind("q", |_| quit()),
//!         events: Default::default(),
//!     })?;
//!     // Components report through events; connect them to your methods.
//!     let _add = counter.add.clicked().with_receiver(&counter).connect(Counter::add).scoped();
//!     counter.add.focus();
//!     # if false {
//!     Tui::new().mouse(true).run(&counter).await?;
//!     # }
//!     Ok(())
//! }
//! ```
//!
//! The repository's `examples/` directory has larger apps: a gallery of every
//! component, a chat client with a network worker, and a dashboard.
#![forbid(unsafe_code)]
#![deny(missing_docs, missing_debug_implementations)]
#![warn(
    // Documentation is part of the API: see CLAUDE.md. CI denies warnings.
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::too_long_first_doc_paragraph,
    rustdoc::missing_crate_level_docs,
    rustdoc::unescaped_backticks
)]

// `#[key_events]` refers to this crate as `::eventful_ratatui`, also inside it.
extern crate self as eventful_ratatui;

pub mod component;
#[cfg(feature = "components")]
pub mod components;
pub mod cursor;
mod dispatch;
mod error;
pub mod focus;
pub mod invalidate;
pub mod keymap;
pub mod mouse;
pub mod overlay;
pub mod run;
pub mod terminal;
pub mod testing;
pub mod theme;
mod tree;

pub use component::{Component, ComponentRef, Handled};
/// The crossterm version this crate reads input with.
pub use crossterm;
pub use error::{Error, Result};
/// # Example
///
/// ```
/// use eventful_ratatui::prelude::*;
/// use eventful_rs::*;
///
/// declare_shard!(Ui, runtime = main);
///
/// #[key_events]
/// trait ListKeys {
///     #[key("j", "down")]
///     fn next(&self);
///     #[key("q", "esc")]
///     fn quit(&self) {
///         quit();
///     }
/// }
///
/// #[eventful(shard = Ui)]
/// struct List {
///     keys: ListKeysBridge,
/// }
/// impl ListKeys for List {
///     fn next(&self) { /* move the selection */ }
/// }
/// impl Component for List {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let help: Vec<String> = ListKeysBridge::BINDINGS
///             .iter()
///             .map(|(chord, event)| format!("{chord}: {event}"))
///             .collect();
///         Line::from(help.join("  ")).render(area, buf);
///     }
///     fn on_input(&self, event: &InputEvent) -> Handled {
///         self.keys.dispatch(event)
///     }
/// }
///
/// #[sharded_main(Ui)]
/// async fn main() -> eventful_ratatui::Result<()> {
///     let list = List::bind_local(List {
///         keys: ListKeysBridge::new(),
///         events: Default::default(),
///     })?;
///     let _keys = list.keys.connect_to(&list).scoped();
///     # if false {
///     run_tui(&list).await?;
///     # }
///     Ok(())
/// }
/// ```
///
/// Mistakes are compile errors, like binding a chord twice:
///
/// ```compile_fail
/// use eventful_ratatui::prelude::*;
///
/// #[key_events]
/// trait Keys {
///     #[key("j")]
///     fn down(&self);
///     #[key("j")] // error: `j` is already bound to `down`
///     fn up(&self);
/// }
/// # fn main() {}
/// ```
pub use eventful_ratatui_macros::key_events;
/// The eventful-rs version this crate is built against.
pub use eventful_rs;
pub use invalidate::Invalidate;
pub use keymap::{KeyChord, Keymap};
/// The ratatui version this crate draws with.
pub use ratatui;
pub use run::{Quit, Tui, quit, run_tui};
pub use terminal::TerminalGuard;

/// Re-exports for `use eventful_ratatui::prelude::*`.
///
/// crossterm's `Event` is exported as `InputEvent`, so the prelude can be
/// glob-imported next to `eventful_rs::*`, which has an `Event` of its own.
///
/// From ratatui it brings its prelude plus the basic widgets that don't share
/// a name with a component (`Block`, `Paragraph`, …). Import ratatui's `List`
/// or `Tabs` widgets explicitly if you want them next to the components.
pub mod prelude {
    #[cfg(feature = "components")]
    pub use crate::components::*;
    pub use crate::{
        Component, ComponentRef, Handled, Invalidate, KeyChord, Keymap, Quit, Tui, cursor, focus,
        invalidate, key_events, mouse, overlay, overlay::Placement, quit, run_tui, theme,
    };
    pub use crossterm::event::{
        Event as InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
        MouseEvent, MouseEventKind,
    };
    pub use ratatui::prelude::*;
    pub use ratatui::widgets::{
        Block, BorderType, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    };
}

/// The README's examples, compiled as doctests.
#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
pub struct ReadmeDoctests;

/// Support for code generated by [`key_events`]. Not public API.
#[doc(hidden)]
pub mod __private {
    pub use crossterm::event::{Event, KeyCode, KeyModifiers};

    /// The chord of a key press or repeat.
    pub fn pressed(event: &Event) -> Option<crate::KeyChord> {
        crate::keymap::pressed(event)
    }

    /// A chord the macro has already normalized.
    pub const fn chord(code: KeyCode, modifiers: KeyModifiers) -> crate::KeyChord {
        crate::KeyChord::from_normalized(code, modifiers)
    }
}
