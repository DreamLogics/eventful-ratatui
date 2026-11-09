//! The terminal cursor. Text inputs place it during `render`, so terminals,
//! input methods and screen readers see where typing goes. Without a request,
//! the cursor is hidden.
use ratatui_core::layout::Position;
use std::cell::Cell;

thread_local! {
    static CURSOR: Cell<Option<Position>> = const { Cell::new(None) };
}

/// Show the cursor at `position` in this frame. Call it from `render`; the
/// last call in a frame wins.
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
