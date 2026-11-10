//! The [`Component`] trait implemented by UI values on the main shard, and
//! [`ComponentRef`], a weak type-erased reference to one.
//!
//! A component draws itself with ratatui in [`render`](Component::render) and
//! reacts to keys and the mouse in [`on_input`](Component::on_input). Both
//! take `&self`: keep changing state in `Cell`s and `RefCell`s, and call
//! [`invalidate`](crate::invalidate::invalidate) after changing it so the
//! loop draws a new frame.
//!
//! ```
//! use eventful_ratatui::{prelude::*, testing::Harness};
//! use std::{cell::Cell, rc::Rc};
//!
//! /// A row of stars; `+` adds one.
//! struct Stars(Cell<usize>);
//! impl Component for Stars {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         "*".repeat(self.0.get()).render(area, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         let InputEvent::Key(key) = event else { return Handled::No };
//!         if key.code != KeyCode::Char('+') {
//!             return Handled::No;
//!         }
//!         self.0.set(self.0.get() + 1);
//!         invalidate::invalidate();
//!         Handled::Yes
//!     }
//!     fn focusable(&self) -> bool {
//!         true
//!     }
//! }
//!
//! /// Two rows of stars. Rendering them through an `Rc` puts them in the
//! /// render tree, so Tab moves between them and keys reach the focused one.
//! struct Rating {
//!     top: Rc<Stars>,
//!     bottom: Rc<Stars>,
//! }
//! impl Component for Rating {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let [top, bottom] = Layout::vertical([Constraint::Length(1); 2]).areas(area);
//!         self.top.render(top, buf);
//!         self.bottom.render(bottom, buf);
//!     }
//! }
//!
//! let rating = Rating {
//!     top: Rc::new(Stars(Cell::new(1))),
//!     bottom: Rc::new(Stars(Cell::new(0))),
//! };
//! let mut ui = Harness::new(rating, 5, 2);
//! ui.key("tab").key("+").key("tab").key("+").key("+");
//! assert_eq!(ui.lines(), ["**", "**"]);
//! ```
use crate::tree;
use crossterm::event::Event;
use eventful_rs::{Eventful, HasEvents, ShardRc, ShardWeak};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{fmt, rc::Rc};

/// Whether a component consumed an input event, returned from
/// [`Component::on_input`].
///
/// Unhandled input bubbles up to the component's parents, then to the root;
/// a Tab, `BackTab` or Esc nobody handles moves focus or closes the top
/// overlay. Converts from `bool`, so a component can return
/// `changed.into()`:
///
/// ```
/// use eventful_ratatui::Handled;
///
/// assert_eq!(Handled::from(true), Handled::Yes);
/// assert_eq!(Handled::from(false), Handled::No);
/// ```
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
/// with interior mutability.
///
/// Mutable state lives in `Cell`s and `RefCell`s on the shard; after
/// changing it, call [`invalidate`](crate::invalidate::invalidate) to have
/// the loop draw a new frame. Only [`render`](Self::render) is required.
/// See the [module docs](self) for a complete example.
///
/// Render children through an `Rc`, a [`ShardRc`] or a library component
/// handle: those record the child in the frame's render tree, which gives it
/// mouse events, focus and a place in the Tab order. A child rendered by plain
/// reference draws fine but is invisible to input routing.
pub trait Component {
    /// Draw into `buf` within `area`.
    ///
    /// Called every frame the component is on screen, so keep it free of
    /// side effects other than requests like
    /// [`cursor::show_at`](crate::cursor::show_at) or
    /// [`invalidate::after`](crate::invalidate::after). Draw only inside
    /// `area`; it may be empty.
    fn render(&self, area: Rect, buf: &mut Buffer);
    /// Handle an input event synchronously. The default ignores it.
    ///
    /// Keys go to the focused component first, then bubble up through the
    /// components that rendered it. Mouse events go to the component under
    /// the pointer, then bubble the same way. Return [`Handled::Yes`] to stop
    /// bubbling. Paste events arrive here too.
    ///
    /// Input is handled synchronously, between frames; call
    /// [`invalidate`](crate::invalidate::invalidate) if the event changed
    /// what is drawn.
    fn on_input(&self, _event: &Event) -> Handled {
        Handled::No
    }
    /// Whether the component can take keyboard focus: Tab stops at it and a
    /// click focuses it. The default is `false`.
    fn focusable(&self) -> bool {
        false
    }
    /// The size the component would like within `available`, for overlays
    /// and containers that size to content. The default takes everything.
    ///
    /// Return at most `available`. Callers may still give a different size,
    /// so `render` must cope with any area.
    ///
    /// ```
    /// use eventful_ratatui::prelude::*;
    ///
    /// struct Hint(&'static str);
    /// impl Component for Hint {
    ///     fn render(&self, area: Rect, buf: &mut Buffer) {
    ///         self.0.render(area, buf);
    ///     }
    ///     fn preferred_size(&self, available: Size) -> Size {
    ///         let width = u16::try_from(self.0.len()).unwrap_or(u16::MAX);
    ///         Size::new(width.min(available.width), available.height.min(1))
    ///     }
    /// }
    ///
    /// assert_eq!(Hint("hello").preferred_size(Size::new(80, 24)), Size::new(5, 1));
    /// assert_eq!(Hint("hello").preferred_size(Size::new(3, 24)), Size::new(3, 1));
    /// ```
    fn preferred_size(&self, available: Size) -> Size {
        available
    }
    /// Whether rendering this records it in the render tree itself, so a
    /// handle around it must not record it again. Only handle types return
    /// `true`; leave the default.
    #[doc(hidden)]
    fn __records(&self) -> bool {
        false
    }
}

/// Forward every method to the referenced component.
macro_rules! forward {
    () => {
        fn on_input(&self, event: &Event) -> Handled {
            (**self).on_input(event)
        }
        fn focusable(&self) -> bool {
            (**self).focusable()
        }
        fn preferred_size(&self, available: Size) -> Size {
            (**self).preferred_size(available)
        }
    };
}

/// Forward every method to the referenced component, which is recorded by
/// the caller.
macro_rules! forward_all {
    () => {
        forward!();
        fn __records(&self) -> bool {
            (**self).__records()
        }
    };
}

impl<C: Component + ?Sized> Component for &C {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        (**self).render(area, buf);
    }
    forward_all!();
}

impl<C: Component + ?Sized> Component for Box<C> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        (**self).render(area, buf);
    }
    forward_all!();
}

/// Records the component in the render tree.
impl<C: Component + ?Sized + 'static> Component for Rc<C> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // A handle around a handle: the inner one records.
        if (**self).__records() {
            return (**self).render(area, buf);
        }
        let focusable = (**self).focusable();
        let _node = tree::record(area, tree::Handle::of(self), focusable, || self.into());
        (**self).render(area, buf);
    }
    forward!();
    fn __records(&self) -> bool {
        true
    }
}

/// Records the component in the render tree.
impl<C> Component for ShardRc<C>
where
    C: Component + Eventful + HasEvents<C::EventSetType> + 'static,
{
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // A handle around a handle: the inner one records.
        if (**self).__records() {
            return (**self).render(area, buf);
        }
        let focusable = (**self).focusable();
        let _node = tree::record(area, tree::Handle::of(self), focusable, || self.into());
        (**self).render(area, buf);
    }
    forward!();
    fn __records(&self) -> bool {
        true
    }
}

/// A weak reference to a component, with its type erased.
trait Target {
    /// Pass input to the component, if it is still alive.
    fn on_input(&self, event: &Event) -> Option<Handled>;
    /// Whether the component is still alive.
    fn is_alive(&self) -> bool;
}

impl<C> Target for ShardWeak<C>
where
    C: Component + Eventful + HasEvents<C::EventSetType>,
{
    fn on_input(&self, event: &Event) -> Option<Handled> {
        self.upgrade().map(|component| (*component).on_input(event))
    }
    fn is_alive(&self) -> bool {
        self.upgrade().is_some()
    }
}

impl<C: Component + ?Sized> Target for std::rc::Weak<C> {
    fn on_input(&self, event: &Event) -> Option<Handled> {
        self.upgrade().map(|component| (*component).on_input(event))
    }
    fn is_alive(&self) -> bool {
        self.strong_count() > 0
    }
}

/// A weak, type-erased reference to a component on this thread. It does not
/// keep the component alive; input to a collected component is skipped.
///
/// [`focus::focused`](crate::focus::focused) returns one. Compare it with a
/// component through [`refers_to`](Self::refers_to):
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// struct Slot(u8);
/// impl Component for Slot {
///     fn render(&self, _: Rect, _: &mut Buffer) {}
///     fn focusable(&self) -> bool {
///         true
///     }
/// }
/// struct Row(Rc<Slot>, Rc<Slot>);
/// impl Component for Row {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let [a, b] = Layout::horizontal([Constraint::Fill(1); 2]).areas(area);
///         self.0.render(a, buf);
///         self.1.render(b, buf);
///     }
/// }
///
/// let mut ui = Harness::new(Row(Rc::new(Slot(0)), Rc::new(Slot(1))), 4, 1);
/// ui.key("tab").key("tab");
/// let focused = focus::focused().expect("Tab focused a slot");
/// assert!(focused.refers_to(&*ui.root().1));
/// assert!(!focused.refers_to(&*ui.root().0));
/// ```
#[derive(Clone)]
pub struct ComponentRef {
    /// The component.
    target: Rc<dyn Target>,
    /// The component's address, for identity checks.
    addr: *const (),
}

impl ComponentRef {
    /// Whether this refers to `component`, which must still be alive. Pass
    /// the component itself, not the `Rc` or `ShardRc` holding it: inside
    /// `render` or `on_input`, `self`.
    ///
    /// Identity is by address, so two zero-sized components can compare
    /// equal; give a component a field if that matters.
    pub fn refers_to<C: ?Sized>(&self, component: &C) -> bool {
        self.is(addr_of(component)) && self.target.is_alive()
    }

    /// Whether the component is still alive.
    pub fn is_alive(&self) -> bool {
        self.target.is_alive()
    }

    /// The component's address.
    pub(crate) fn addr(&self) -> *const () {
        self.addr
    }

    /// Whether this refers to the component at `addr`, alive or not.
    pub(crate) fn is(&self, addr: *const ()) -> bool {
        std::ptr::eq(self.addr, addr)
    }

    /// Pass `event` to the component, if it is still alive.
    pub(crate) fn offer(&self, event: &Event) -> Option<Handled> {
        self.target.on_input(event)
    }
}

/// The address identifying `component`.
pub(crate) fn addr_of<C: ?Sized>(component: &C) -> *const () {
    std::ptr::from_ref(component).cast()
}

impl<C> From<&ShardRc<C>> for ComponentRef
where
    C: Component + Eventful + HasEvents<C::EventSetType>,
{
    fn from(component: &ShardRc<C>) -> Self {
        Self {
            target: Rc::new(ShardRc::downgrade(component)),
            addr: addr_of(&**component),
        }
    }
}

impl<C: Component + ?Sized + 'static> From<&Rc<C>> for ComponentRef {
    fn from(component: &Rc<C>) -> Self {
        Self {
            target: Rc::new(Rc::downgrade(component)),
            addr: Rc::as_ptr(component).cast(),
        }
    }
}

impl fmt::Debug for ComponentRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ComponentRef").field(&self.addr).finish()
    }
}
