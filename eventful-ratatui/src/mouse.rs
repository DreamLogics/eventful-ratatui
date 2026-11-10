//! Mouse routing by hit-testing the last frame.
//!
//! Enable it with [`Tui::mouse`](crate::Tui::mouse). It routes by the frame's
//! render tree (see [`Component`](crate::Component)), so it needs no code in
//! components. Later entries are on top, overlays above the root, and an
//! entry rendered inside another one's `render` is its child.
//!
//! A mouse event goes to the topmost component under the pointer, then
//! bubbles to its parents until one returns
//! [`Handled::Yes`](crate::Handled::Yes). A component that handles a button
//! press captures the mouse: drags and the release go to it and its parents,
//! even when the pointer leaves its area. If nothing handles
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
//!
//! // The harness routes the mouse like the loop does.
//! let toggle = Toggle { on: Cell::new(false), ui: Invalidate::new() };
//! let mut ui = eventful_ratatui::testing::Harness::new(std::rc::Rc::new(toggle), 5, 1);
//! ui.click(1, 0);
//! assert_eq!(ui.lines(), ["[ ]"]);
//! ui.click(0, 0);
//! assert_eq!(ui.lines(), ["[x]"]);
//! ```
use crate::{component::addr_of, tree};
use crossterm::event::MouseEvent;
use ratatui_core::layout::{Position, Rect};

/// The area `component` was last drawn into, if it was rendered through a
/// handle in the last frame.
///
/// If it was drawn more than once, the topmost area. Pass the component
/// itself, not its `Rc`; inside `on_input`, `self`. `None` if it was not
/// rendered through an `Rc`, a `ShardRc` or a component handle. See the
/// [module docs](self) for an example.
pub fn area_of<C: ?Sized>(component: &C) -> Option<Rect> {
    let addr = addr_of(component);
    tree::with_last(|nodes| tree::find(nodes, addr).map(|index| nodes[index].area))
}

/// Whether `event` happened inside the area `component` was last drawn into.
/// See [`area_of`]; `false` if it was not drawn.
///
/// The usual check for a press that should only count over the component,
/// while drags and releases still arrive outside it after capture:
///
/// ```
/// use eventful_ratatui::prelude::*;
///
/// struct Knob;
/// impl Component for Knob {
///     fn render(&self, _: Rect, _: &mut Buffer) {}
///     fn on_input(&self, event: &InputEvent) -> Handled {
///         match event {
///             InputEvent::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left) => {
///                 if mouse::is_hit(self, m) { /* released over the knob: commit */ }
///                 Handled::Yes
///             }
///             _ => Handled::No,
///         }
///     }
/// }
/// ```
pub fn is_hit<C: ?Sized>(component: &C, event: &MouseEvent) -> bool {
    area_of(component).is_some_and(|area| area.contains(position(event)))
}

/// Whether the pointer is over `component`, or over a component it
/// rendered, in the last frame.
///
/// Overlays above it and modal overlays hide it
/// from the pointer. Pass the component itself; inside `render`, `self`.
///
/// Hover changes redraw by themselves, so components can draw a hover state
/// without handling mouse moves. Requires mouse routing; without it, nothing
/// is hovered.
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// struct Link;
/// impl Component for Link {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let text = if mouse::is_hovered(self) { "_link_" } else { "link" };
///         text.render(area, buf);
///     }
/// }
///
/// let mut ui = Harness::new(Rc::new(Link), 6, 1);
/// assert_eq!(ui.lines(), ["link"]);
/// ui.hover(2, 0);
/// assert_eq!(ui.lines(), ["_link_"]);
/// ```
pub fn is_hovered<C: ?Sized>(component: &C) -> bool {
    tree::is_hovered(addr_of(component))
}

/// Where the pointer is, if `component` is hovered. See [`is_hovered`].
///
/// Calling it from `render` marks the component as following the pointer:
/// it then redraws on every move over it, for hover effects on parts of it,
/// like rows or tabs. The position is on the screen; subtract the
/// component's area origin for a position inside it.
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// /// Highlights the cell under the pointer.
/// struct Cells;
/// impl Component for Cells {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let hovered = mouse::hover_position(self).map(|p| p.x - area.x);
///         let row: String = (0..area.width)
///             .map(|x| if Some(x) == hovered { '#' } else { '.' })
///             .collect();
///         row.render(area, buf);
///     }
/// }
///
/// let mut ui = Harness::new(Rc::new(Cells), 4, 1);
/// ui.hover(1, 0);
/// assert_eq!(ui.lines(), [".#.."]);
/// ui.hover(3, 0);
/// assert_eq!(ui.lines(), ["...#"]);
/// ```
pub fn hover_position<C: ?Sized>(component: &C) -> Option<Position> {
    let addr = addr_of(component);
    tree::track_pointer(addr);
    tree::is_hovered(addr).then(tree::pointer).flatten()
}

/// Where `event` happened, as a screen position.
pub fn position(event: &MouseEvent) -> Position {
    Position::new(event.column, event.row)
}
