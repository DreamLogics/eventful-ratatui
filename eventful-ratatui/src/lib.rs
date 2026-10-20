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

pub mod component;
pub mod focus;
pub mod invalidate;
pub mod keymap;
pub mod run;
pub mod terminal;

pub use component::{Component, Handled};
/// The eventful-rs version this crate is built against.
pub use eventful_rs;

/// Re-exports for `use eventful_ratatui::prelude::*`.
pub mod prelude {
    pub use crate::component::{Component, Handled};
    pub use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    pub use ratatui::prelude::*;
    pub use ratatui::widgets::*;
}
