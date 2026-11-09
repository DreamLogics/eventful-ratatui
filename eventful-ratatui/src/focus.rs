//! [`FocusChain`]: the path from a container to the focused component. Input
//! goes to the focused leaf and bubbles up while it is not handled. Calls are
//! direct, not queued events, so bubbling gets an immediate answer.
use crate::{Component, Handled, invalidate};
use crossterm::event::Event;
use eventful_rs::{Eventful, HasEvents, ShardRc, ShardWeak};
use std::{cell::RefCell, fmt, rc::Rc};

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

/// A weak, type-erased reference to a component on this shard, for a
/// [`FocusChain`]. It does not keep the component alive; input to a
/// collected component is skipped.
#[derive(Clone)]
pub struct ComponentRef {
    /// The component.
    target: Rc<dyn Target>,
    /// The component's address, for identity checks.
    addr: *const (),
}

impl ComponentRef {
    /// Whether this refers to `component`, which must still be alive.
    pub fn refers_to<C: ?Sized>(&self, component: &C) -> bool {
        std::ptr::eq(self.addr, std::ptr::from_ref(component).cast()) && self.target.is_alive()
    }

    /// Pass `event` to the component, if it is still alive.
    pub(crate) fn offer(&self, event: &Event) -> Option<Handled> {
        self.target.on_input(event)
    }
}

impl<C> From<&ShardRc<C>> for ComponentRef
where
    C: Component + Eventful + HasEvents<C::EventSetType>,
{
    fn from(component: &ShardRc<C>) -> Self {
        Self {
            target: Rc::new(ShardRc::downgrade(component)),
            addr: (&**component as *const C).cast(),
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

/// The focused path below a container, ordered from the outermost component
/// to the focused leaf.
///
/// A container keeps one, renders its children with
/// [`is_focused`](Self::is_focused) or [`contains`](Self::contains), and
/// forwards input to [`dispatch`](Self::dispatch) before handling it itself:
///
/// ```
/// use eventful_ratatui::prelude::*;
///
/// struct Panel {
///     focus: FocusChain,
///     keys: Keymap<Panel>,
/// }
/// impl Component for Panel {
///     fn render(&self, area: Rect, buf: &mut Buffer) {}
///     fn on_input(&self, event: &InputEvent) -> Handled {
///         match self.focus.dispatch(event) {
///             Handled::No => self.keys.dispatch(self, event),
///             handled => handled,
///         }
///     }
/// }
/// ```
///
/// Changing the focus requests a redraw.
#[derive(Debug, Default)]
pub struct FocusChain {
    /// Outermost first, focused leaf last.
    path: RefCell<Vec<ComponentRef>>,
}

impl FocusChain {
    /// An empty chain: nothing below the container has focus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the focused path, outermost first.
    pub fn focus<I>(&self, path: I)
    where
        I: IntoIterator,
        I::Item: Into<ComponentRef>,
    {
        *self.path.borrow_mut() = path.into_iter().map(Into::into).collect();
        invalidate::mark();
    }

    /// Move focus into `component`, a child of the current leaf.
    pub fn push(&self, component: impl Into<ComponentRef>) {
        self.path.borrow_mut().push(component.into());
        invalidate::mark();
    }

    /// Move focus out of the leaf, to its parent. Returns the old leaf.
    pub fn pop(&self) -> Option<ComponentRef> {
        let leaf = self.path.borrow_mut().pop();
        invalidate::mark();
        leaf
    }

    /// Give up focus entirely.
    pub fn clear(&self) {
        self.path.borrow_mut().clear();
        invalidate::mark();
    }

    /// The focused leaf, if any.
    pub fn leaf(&self) -> Option<ComponentRef> {
        self.path.borrow().last().cloned()
    }

    /// Whether `component` is the focused leaf.
    pub fn is_focused<C: ?Sized>(&self, component: &C) -> bool {
        self.path
            .borrow()
            .last()
            .is_some_and(|leaf| leaf.refers_to(component))
    }

    /// Whether `component` is on the focused path.
    pub fn contains<C: ?Sized>(&self, component: &C) -> bool {
        self.path
            .borrow()
            .iter()
            .any(|entry| entry.refers_to(component))
    }

    /// Offer `event` to the focused leaf, then to each component above it,
    /// until one handles it. Collected components are skipped. Handlers may
    /// change the focus; this event still follows the path it started on.
    ///
    /// Mouse events are not dispatched: the pointer picks their target, see
    /// [`crate::mouse`].
    pub fn dispatch(&self, event: &Event) -> Handled {
        if matches!(event, Event::Mouse(_)) {
            return Handled::No;
        }
        let path = self.path.borrow().clone();
        for component in path.iter().rev() {
            if component.target.on_input(event) == Some(Handled::Yes) {
                return Handled::Yes;
            }
        }
        Handled::No
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};
    use ratatui_core::{buffer::Buffer, layout::Rect};
    use std::cell::Cell;

    /// Handles one key and records what reached it.
    struct Probe {
        key: char,
        seen: Cell<u32>,
        chain: Option<Rc<FocusChain>>,
    }
    impl Probe {
        fn new(key: char) -> Rc<Self> {
            Rc::new(Self {
                key,
                seen: Cell::new(0),
                chain: None,
            })
        }
    }
    impl Component for Probe {
        fn render(&self, _: Rect, _: &mut Buffer) {}
        fn on_input(&self, event: &Event) -> Handled {
            self.seen.set(self.seen.get() + 1);
            if let Some(chain) = &self.chain {
                chain.clear(); // Changing focus mid-dispatch must not panic.
            }
            let Event::Key(key) = event else {
                return Handled::No;
            };
            (key.code == KeyCode::Char(self.key)).into()
        }
    }

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::from(KeyCode::Char(c)))
    }

    #[test]
    fn input_bubbles_from_the_leaf_until_handled() {
        let (outer, inner) = (Probe::new('o'), Probe::new('i'));
        let chain = FocusChain::new();
        chain.focus([&outer, &inner]);
        assert_eq!(chain.dispatch(&key('i')), Handled::Yes);
        assert_eq!((outer.seen.get(), inner.seen.get()), (0, 1));
        assert_eq!(chain.dispatch(&key('o')), Handled::Yes);
        assert_eq!((outer.seen.get(), inner.seen.get()), (1, 2));
        assert_eq!(chain.dispatch(&key('x')), Handled::No);
        assert_eq!((outer.seen.get(), inner.seen.get()), (2, 3));
    }

    #[test]
    fn tracks_focus_by_identity() {
        let (outer, inner, other) = (Probe::new('o'), Probe::new('i'), Probe::new('x'));
        let chain = FocusChain::new();
        chain.push(&outer);
        chain.push(&inner);
        assert!(chain.is_focused(&*inner));
        assert!(!chain.is_focused(&*outer));
        assert!(chain.contains(&*outer));
        assert!(!chain.contains(&*other));
        assert!(chain.pop().unwrap().refers_to(&*inner));
        assert!(chain.is_focused(&*outer));
    }

    #[test]
    fn collected_components_are_skipped() {
        let outer = Probe::new('o');
        let chain = FocusChain::new();
        {
            let inner = Probe::new('i');
            chain.focus([&outer, &inner]);
        }
        assert_eq!(chain.dispatch(&key('o')), Handled::Yes);
        assert_eq!(chain.dispatch(&key('i')), Handled::No);
        assert_eq!(outer.seen.get(), 2);
    }

    #[test]
    fn handlers_may_change_focus() {
        let chain = Rc::new(FocusChain::new());
        let probe = Rc::new(Probe {
            key: 'p',
            seen: Cell::new(0),
            chain: Some(chain.clone()),
        });
        chain.focus([&probe]);
        assert_eq!(chain.dispatch(&key('p')), Handled::Yes);
        assert!(chain.leaf().is_none());
    }
}
