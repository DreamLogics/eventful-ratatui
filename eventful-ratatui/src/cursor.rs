//! The terminal cursor. Text inputs place it during `render`, so terminals,
//! input methods and screen readers see where typing goes. Without a request,
//! the cursor is hidden.
//!
//! ```
//! use eventful_ratatui::{prelude::*, testing::Harness};
//!
//! struct Prompt;
//! impl Component for Prompt {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         "> ".render(area, buf);
//!         cursor::show_at(Position::new(area.x + 2, area.y));
//!     }
//! }
//!
//! let ui = Harness::new(Prompt, 10, 1);
//! assert_eq!(ui.cursor(), Some(Position::new(2, 0)));
//! ```
use ratatui_core::layout::Position;
use std::cell::Cell;

thread_local! {
    static CURSOR: Cell<Option<Position>> = const { Cell::new(None) };
}

/// Show the cursor at `position` in this frame. Call it from `render`; the
/// last call in a frame wins.
///
/// `position` is a screen position, like the `area` given to `render`.
/// Inside a scrolling container it is shifted with the content, and the
/// request is dropped when it is scrolled out of view. See the
/// [module docs](self).
pub fn show_at(position: Position) {
    // Inside a scrolled viewport, map to the screen; hidden if scrolled out.
    if let Some(position) = crate::tree::to_screen(position) {
        CURSOR.with(|cursor| cursor.set(Some(position)));
    }
}

/// Take this frame's request.
pub(crate) fn take() -> Option<Position> {
    CURSOR.with(Cell::take)
}
