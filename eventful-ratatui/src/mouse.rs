//! Mouse routing by hit-testing the last frame.
//!
//! Enable it with [`Tui::mouse`](crate::Tui::mouse). It routes by the frame's
//! render tree (see [`Component`](crate::Component)), so it needs no code in
//! components. Later entries are on top, overlays above the root, and an
//! entry rendered inside another one's `render` is its child.
//!
//! A mouse event goes to the topmost component under the pointer, then
//! bubbles to its parents until one returns [`Handled::Yes`]. A component that
//! handles a button press captures the mouse: drags and the release go to it
//! and its parents, even when the pointer leaves its area. If nothing handles
//! the event and the root was not hit-tested itself (it is not an `Rc` or
//! `ShardRc`), the root gets it.
//!
//! A press also focuses the innermost focusable component under the pointer,
//! before the event is delivered, and closes popups it lands outside of (see
//! [`crate::overlay`]). Use [`area_of`] or [`is_hit`] to find out where an
//! event landed relative to a component:
//!
//! ```
//! use eventful_ratatui::prelude::*;
//! # use std::cell::Cell;
//!
//! struct Toggle {
//!     on: Cell<bool>,
//!     ui: Invalidate,
//! }
//! impl Component for Toggle {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         Line::from(if self.on.get() { "[x]" } else { "[ ]" }).render(area, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         let InputEvent::Mouse(mouse) = event else {
//!             return Handled::No;
//!         };
//!         if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
//!             return Handled::No;
//!         }
//!         // Only the first column is the box.
//!         if mouse::area_of(self).is_some_and(|area| mouse.column == area.x) {
//!             self.on.set(!self.on.get());
//!             self.ui.invalidate();
//!         }
//!         Handled::Yes
//!     }
//! }
//! ```
use crate::{component::addr_of, tree};
use crossterm::event::MouseEvent;
use ratatui_core::layout::{Position, Rect};

/// The area `component` was last drawn into, if it was rendered through a
/// handle in the last frame. If it was drawn more than once, the topmost
/// area. Pass the component itself, not its `Rc`; inside `on_input`, `self`.
pub fn area_of<C: ?Sized>(component: &C) -> Option<Rect> {
    let addr = addr_of(component);
    tree::with_last(|nodes| tree::find(nodes, addr).map(|index| nodes[index].area))
}

/// Whether `event` happened inside the area `component` was last drawn into.
/// See [`area_of`].
pub fn is_hit<C: ?Sized>(component: &C, event: &MouseEvent) -> bool {
    area_of(component).is_some_and(|area| area.contains(position(event)))
}

/// Where `event` happened.
pub fn position(event: &MouseEvent) -> Position {
    Position::new(event.column, event.row)
}
