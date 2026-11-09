//! The [`Component`] trait implemented by UI values on the main shard.
use crate::mouse;
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

impl<C: Component + ?Sized> Component for &C {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        (**self).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        (**self).on_input(event)
    }
}

impl<C: Component + ?Sized> Component for Box<C> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        (**self).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        (**self).on_input(event)
    }
}

/// Records the component for mouse hit-testing, see [`crate::mouse`].
impl<C: Component + ?Sized + 'static> Component for std::rc::Rc<C> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let _hit = mouse::record(area, mouse::Handle::of(self), || self.into());
        (**self).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        (**self).on_input(event)
    }
}

/// Records the component for mouse hit-testing, see [`crate::mouse`].
impl<C> Component for eventful_rs::ShardRc<C>
where
    C: Component + eventful_rs::Eventful + eventful_rs::HasEvents<C::EventSetType> + 'static,
{
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let _hit = mouse::record(area, mouse::Handle::of(self), || self.into());
        (**self).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        (**self).on_input(event)
    }
}
