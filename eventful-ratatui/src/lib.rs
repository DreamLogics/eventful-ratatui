//! Ratatui applications on [eventful-rs](https://github.com/DreamLogics/eventful-rs).
//!
//! Workers on background shards do I/O and computation, while UI components
//! live on the main shard as ordinary `#[eventful]` values. Events flow from
//! workers to components, components request redraws, and one loop handles
//! input and rendering.
//!
//! Applications depend on both `eventful-rs` and `eventful-ratatui`: the
//! eventful-rs macros resolve the runtime through the application's own
//! `eventful-rs` dependency. Both must resolve to the same semver-compatible
//! version, or types like `ShardRc` from the two copies won't match.
#![forbid(unsafe_code)]
#![deny(missing_docs, missing_debug_implementations)]

pub mod component;
mod error;
pub mod focus;
pub mod invalidate;
pub mod keymap;
pub mod run;
pub mod terminal;

pub use component::{Component, Handled};
/// The crossterm version this crate reads input with.
pub use crossterm;
pub use error::{Error, Result};
/// The eventful-rs version this crate is built against.
pub use eventful_rs;
pub use focus::{ComponentRef, FocusChain};
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
pub mod prelude {
    pub use crate::{
        Component, ComponentRef, FocusChain, Handled, Invalidate, KeyChord, Keymap, Quit, Tui,
        quit, run_tui,
    };
    pub use crossterm::event::{
        Event as InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    };
    pub use ratatui::prelude::*;
    pub use ratatui::widgets::*;
}
