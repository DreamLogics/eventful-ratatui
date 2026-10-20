//! The [`Component`] trait implemented by UI values on the main shard.
use crossterm::event::Event;
use ratatui_core::{buffer::Buffer, layout::Rect};

/// Whether a component consumed an input event. Unhandled input bubbles up
/// the focus chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handled {
    /// The event was consumed; stop bubbling.
    Yes,
    /// The event was ignored; offer it to the parent.
    No,
}
impl From<bool> for Handled {
    fn from(handled: bool) -> Self {
        if handled { Self::Yes } else { Self::No }
    }
}

/// A UI element rendered through `&self`, as ratatui recommends for widgets
/// with interior mutability. Mutable state lives in `RefCell`s on the shard.
pub trait Component {
    /// Draw into `buf` within `area`.
    fn render(&self, area: Rect, buf: &mut Buffer);
    /// Handle an input event synchronously. The default ignores it.
    fn on_input(&self, _event: &Event) -> Handled {
        Handled::No
    }
}
