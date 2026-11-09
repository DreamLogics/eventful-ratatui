//! Mouse routing by hit-testing the last frame.
//!
//! Enable it with [`Tui::mouse`](crate::Tui::mouse). While a frame renders,
//! every component rendered through an `Rc` or [`ShardRc`](eventful_rs::ShardRc)
//! records its area, so the hit map needs no code in components. Later
//! entries are on top, and an entry rendered inside another one's `render` is
//! its child.
//!
//! A mouse event goes to the topmost component under the pointer, then
//! bubbles to its parents until one returns [`Handled::Yes`]. A component that
//! handles a button press captures the mouse: drags and the release go to it
//! and its parents, even when the pointer leaves its area. If nothing handles
//! the event and the root was not hit-tested itself (it is not an `Rc` or
//! `ShardRc`), the root gets it.
//!
//! Mouse events never travel along a [`FocusChain`](crate::FocusChain); the
//! pointer, not the focus, picks the target. Use [`area_of`] or [`is_hit`] to
//! find out where an event landed relative to a component:
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
use crate::{Component, ComponentRef, Handled};
use crossterm::event::{Event, MouseEvent, MouseEventKind};
use ratatui_core::layout::{Position, Rect};
use std::{any::TypeId, cell::RefCell, mem};

/// Identifies the handle a component was rendered through, so the loop can
/// tell whether its root was hit-tested.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Handle(*const (), TypeId);

impl Handle {
    pub(crate) fn of<T: ?Sized + 'static>(value: &T) -> Self {
        Self(std::ptr::from_ref(value).cast(), TypeId::of::<T>())
    }
}

/// A rendered component.
#[derive(Clone)]
struct Hit {
    area: Rect,
    target: ComponentRef,
    handle: Handle,
    /// The entry whose `render` rendered this one.
    parent: Option<usize>,
}

/// Hit maps of this thread's loop.
#[derive(Default)]
struct HitMap {
    /// A loop with mouse routing is running.
    enabled: bool,
    /// The frame being rendered.
    frame: Vec<Hit>,
    /// Entries whose `render` is running, innermost last.
    open: Vec<usize>,
    /// The last completed frame, which input is routed against.
    last: Vec<Hit>,
}

thread_local! {
    static HITS: RefCell<HitMap> = RefCell::default();
}

/// Record that a component is rendering into `area`. Keep the result alive
/// for the rest of its `render`, so components it renders become children.
pub(crate) fn record(
    area: Rect,
    handle: Handle,
    target: impl FnOnce() -> ComponentRef,
) -> Recording {
    HITS.with_borrow_mut(|map| {
        if !map.enabled {
            return Recording(false);
        }
        let parent = map.open.last().copied();
        map.open.push(map.frame.len());
        map.frame.push(Hit {
            area,
            target: target(),
            handle,
            parent,
        });
        Recording(true)
    })
}

/// Closes a [`record`] entry.
pub(crate) struct Recording(bool);

impl Drop for Recording {
    fn drop(&mut self) {
        if self.0 {
            HITS.with_borrow_mut(|map| map.open.pop());
        }
    }
}

/// Mouse routing for one loop run. Turns recording off again on drop.
pub(crate) struct Session;

impl Session {
    pub(crate) fn start() -> Self {
        let enabled = HitMap {
            enabled: true,
            ..HitMap::default()
        };
        let _old = HITS.with_borrow_mut(|map| mem::replace(map, enabled));
        Self
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Replace, not mutate in place, so the old entries drop outside the
        // borrow.
        let _old = HITS.with_borrow_mut(mem::take);
    }
}

/// Start recording a frame.
pub(crate) fn begin_frame() {
    let _old = HITS.with_borrow_mut(|map| {
        map.open.clear();
        mem::take(&mut map.frame)
    });
}

/// Route input against the frame just rendered.
pub(crate) fn end_frame() {
    let _old = HITS.with_borrow_mut(|map| {
        let frame = mem::take(&mut map.frame);
        mem::replace(&mut map.last, frame)
    });
}

/// The area `component` was last drawn into, if it was rendered through an
/// `Rc` or `ShardRc` in the last frame and mouse routing is enabled. If it
/// was drawn more than once, the topmost area.
pub fn area_of<C: ?Sized>(component: &C) -> Option<Rect> {
    HITS.with_borrow(|map| {
        map.last
            .iter()
            .rev()
            .find(|hit| hit.target.refers_to(component))
            .map(|hit| hit.area)
    })
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

/// The components under `position`, topmost first, then its parents.
fn path_at(position: Position) -> Vec<(ComponentRef, Handle)> {
    HITS.with_borrow(|map| {
        let mut path = Vec::new();
        let mut next = map.last.iter().rposition(|hit| hit.area.contains(position));
        while let Some(index) = next {
            let hit = &map.last[index];
            path.push((hit.target.clone(), hit.handle));
            next = hit.parent;
        }
        path
    })
}

/// Routes mouse events and tracks mouse capture.
#[derive(Default)]
pub(crate) struct Router {
    /// The path that handled the last button press, until it is released.
    capture: Option<Vec<(ComponentRef, Handle)>>,
}

impl Router {
    /// Offer `event` along the path for `mouse`, then to `root` if it is not
    /// on the path.
    pub(crate) fn dispatch<C: Component + ?Sized + 'static>(
        &mut self,
        root: &C,
        event: &Event,
        mouse: &MouseEvent,
    ) -> Handled {
        let path = match mouse.kind {
            MouseEventKind::Drag(_) | MouseEventKind::Up(_) => match &self.capture {
                Some(path) => path.clone(),
                None => path_at(position(mouse)),
            },
            _ => path_at(position(mouse)),
        };
        let press = matches!(mouse.kind, MouseEventKind::Down(_));
        if press || matches!(mouse.kind, MouseEventKind::Up(_)) {
            self.capture = None;
        }
        // Offer outside the hit map borrow: handlers may call `area_of`.
        for (index, (target, _)) in path.iter().enumerate() {
            if target.offer(event) == Some(Handled::Yes) {
                if press {
                    self.capture = Some(path[index..].to_vec());
                }
                return Handled::Yes;
            }
        }
        let root_handle = Handle::of(root);
        if path.iter().any(|(_, handle)| *handle == root_handle) {
            return Handled::No;
        }
        let handled = root.on_input(event);
        if press && handled == Handled::Yes {
            // An empty path: only the root, below, gets the drags.
            self.capture = Some(Vec::new());
        }
        handled
    }
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("captured", &self.capture.is_some())
            .finish()
    }
}
