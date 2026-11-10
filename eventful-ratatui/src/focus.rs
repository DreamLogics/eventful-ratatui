//! Keyboard focus: one focused component per loop.
//!
//! Components opt in with [`Component::focusable`](crate::Component::focusable).
//! Tab and `BackTab` move between focusable components in render order, a click
//! focuses the component under the pointer, and key input goes to the focused
//! component first, then bubbles through the components that rendered it, then
//! to the root. With nothing focused, the root gets every key.
//!
//! A focused component that stops being focusable, like a button that was
//! just disabled, loses focus after the next frame: it moves to the next
//! focusable component in the same layer, wrapping around, or is cleared if
//! there is none. Components that were never focusable keep focus given to
//! them with [`set`].
//!
//! Overlays scope the traversal: while a modal dialog or a menu is open, Tab
//! stays inside it. See [`crate::overlay`].
//!
//! Pass the component itself to these functions, not the `Rc` or `ShardRc`
//! holding it: inside `render` or `on_input`, that is `self`.
//!
//! ```
//! use eventful_ratatui::prelude::*;
//!
//! /// A box that shows whether it has focus.
//! struct Field;
//! impl Component for Field {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let style = if focus::is_focused(self) {
//!             Style::new().reversed()
//!         } else {
//!             Style::new()
//!         };
//!         Block::bordered().border_style(style).render(area, buf);
//!     }
//!     fn focusable(&self) -> bool {
//!         true
//!     }
//! }
//! ```
use crate::{
    component::{ComponentRef, addr_of},
    invalidate,
    tree::{self, Handle, Node},
};
use std::cell::RefCell;

/// Focus of this thread's loop.
#[derive(Default)]
struct State {
    /// The focused component.
    focused: Option<ComponentRef>,
    /// A component asked for focus before it was first rendered.
    pending: Option<*const ()>,
    /// The focused component as of the last frame, and whether it was
    /// focusable then: focus moves on when it stops being focusable.
    seen: Option<(*const (), bool)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// Give focus to `component`. If it has not been rendered yet, it gets focus
/// once it is, so applications can set the initial focus before running.
///
/// Requests a redraw. Pass the component itself, not its `Rc` or `ShardRc`;
/// it must be rendered through a handle to receive keys.
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// struct Input;
/// impl Component for Input {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let text = if focus::is_focused(self) { "[focused]" } else { "[ ]" };
///         text.render(area, buf);
///     }
///     fn focusable(&self) -> bool {
///         true
///     }
/// }
/// struct Form(Rc<Input>);
/// impl Component for Form {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         self.0.render(area, buf);
///     }
/// }
///
/// let input = Rc::new(Input);
/// focus::set(&*input);
/// let mut ui = Harness::new(Form(input), 10, 1);
/// assert_eq!(ui.lines(), ["[focused]"]);
///
/// focus::clear();
/// ui.render();
/// assert_eq!(ui.lines(), ["[ ]"]);
/// ```
pub fn set<C: ?Sized>(component: &C) {
    let addr = addr_of(component);
    let target = tree::with_last(|nodes| tree::find(nodes, addr).map(|i| nodes[i].target.clone()));
    let _old = STATE.with_borrow_mut(|state| match target {
        Some(target) => {
            state.pending = None;
            state.focused.replace(target)
        }
        None => {
            state.pending = Some(addr);
            state.focused.take()
        }
    });
    invalidate::mark();
}

/// Remove focus from whatever has it, and cancel a pending [`set`]. Keys
/// then go to the root. Requests a redraw.
pub fn clear() {
    let _old = STATE.with_borrow_mut(|state| {
        state.pending = None;
        state.focused.take()
    });
    invalidate::mark();
}

/// Whether `component` has focus, or has asked for it with [`set`] and not
/// been rendered yet. Pass the component itself; inside `render`, `self`.
/// See the [module docs](self) for an example.
pub fn is_focused<C: ?Sized>(component: &C) -> bool {
    let addr = addr_of(component);
    STATE.with_borrow(|state| {
        state.pending == Some(addr)
            || state
                .focused
                .as_ref()
                .is_some_and(|focused| focused.refers_to(component))
    })
}

/// Whether `component` or something it rendered has focus, as of the last
/// frame. Containers use it to highlight while a child is focused.
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// struct Input;
/// impl Component for Input {
///     fn render(&self, _: Rect, _: &mut Buffer) {}
///     fn focusable(&self) -> bool {
///         true
///     }
/// }
/// struct Group(Rc<Input>);
/// impl Component for Group {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let title = if focus::within(self) { "active" } else { "idle" };
///         title.render(area, buf);
///         self.0.render(area, buf);
///     }
/// }
///
/// let mut ui = Harness::new(Rc::new(Group(Rc::new(Input))), 6, 1);
/// assert_eq!(ui.lines(), ["idle"]);
/// ui.key("tab");
/// assert_eq!(ui.lines(), ["active"]);
/// ```
pub fn within<C: ?Sized>(component: &C) -> bool {
    if is_focused(component) {
        return true;
    }
    let addr = addr_of(component);
    let Some(focused) = focused() else {
        return false;
    };
    tree::with_last(|nodes| {
        let mut next = nodes
            .iter()
            .rposition(|node| node.target.is(focused.addr()));
        while let Some(index) = next {
            if nodes[index].target.is(addr) {
                return true;
            }
            next = nodes[index].parent;
        }
        false
    })
}

/// The focused component, if any and still alive. See [`ComponentRef`] for
/// an example.
pub fn focused() -> Option<ComponentRef> {
    STATE.with_borrow(|state| state.focused.clone().filter(ComponentRef::is_alive))
}

/// Move focus to the next focusable component in render order, wrapping
/// around. Tab does this when no component handles it.
///
/// With nothing focused, focuses the first one. Traversal uses the last
/// frame's render tree and stays inside the open modal overlay, or the
/// popup holding the focus. Does nothing if no component is focusable.
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use std::rc::Rc;
///
/// struct Digit(u8);
/// impl Component for Digit {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let mark = if focus::is_focused(self) { '>' } else { ' ' };
///         format!("{mark}{}", self.0).render(area, buf);
///     }
///     fn focusable(&self) -> bool {
///         true
///     }
/// }
/// struct Row(Vec<Rc<Digit>>);
/// impl Component for Row {
///     fn render(&self, area: Rect, buf: &mut Buffer) {
///         let areas = Layout::horizontal([Constraint::Length(2); 3]).split(area);
///         for (digit, area) in self.0.iter().zip(areas.iter()) {
///             digit.render(*area, buf);
///         }
///     }
/// }
///
/// let mut ui = Harness::new(Row((1..=3).map(|n| Rc::new(Digit(n))).collect()), 6, 1);
/// focus::next();
/// ui.render();
/// assert_eq!(ui.lines(), [">1 2 3"]);
/// focus::previous();
/// ui.render();
/// assert_eq!(ui.lines(), [" 1 2>3"]);
/// ```
pub fn next() {
    traverse(true);
}

/// Move focus to the previous focusable component, wrapping around.
/// `BackTab` does this when no component handles it. With nothing focused,
/// focuses the last one. See [`next`].
pub fn previous() {
    traverse(false);
}

/// Move focus within the current scope.
fn traverse(forward: bool) {
    let scope = crate::overlay::focus_scope();
    let current = focused();
    let target = tree::with_last(|nodes| {
        // Stay in the focused component's layer if it is in scope, so Tab
        // in a non-modal popup stays in the popup.
        let layer = current
            .as_ref()
            .and_then(|focused| tree::find(nodes, focused.addr()))
            .map(|i| nodes[i].layer)
            .filter(|layer| crate::overlay::in_scope(*layer, scope))
            .unwrap_or(scope);
        let mut stops: Vec<&Node> = Vec::new();
        for node in nodes.iter().filter(|n| n.focusable && n.layer == layer) {
            if !stops.iter().any(|stop| stop.target.is(node.target.addr())) {
                stops.push(node);
            }
        }
        let position = current
            .as_ref()
            .and_then(|focused| stops.iter().position(|stop| stop.target.is(focused.addr())));
        let index = match (position, forward) {
            _ if stops.is_empty() => return None,
            (Some(i), true) => (i + 1) % stops.len(),
            (Some(i), false) => (i + stops.len() - 1) % stops.len(),
            (None, true) => 0,
            (None, false) => stops.len() - 1,
        };
        Some(stops[index].target.clone())
    });
    if let Some(target) = target {
        focus_ref(target);
    }
}

/// Focus a component found in the render tree.
pub(crate) fn focus_ref(target: ComponentRef) {
    let _old = STATE.with_borrow_mut(|state| {
        state.pending = None;
        state.focused.replace(target)
    });
    invalidate::mark();
}

/// Restore a focus saved with [`focused`], or clear it.
pub(crate) fn restore(saved: Option<ComponentRef>) {
    match saved.filter(ComponentRef::is_alive) {
        Some(target) => focus_ref(target),
        None => clear(),
    }
}

/// After a frame: resolve a pending focus request, and move focus on if the
/// focused component stopped being focusable, like a button just disabled.
pub(crate) fn end_frame() {
    if let Some(addr) = STATE.with_borrow(|state| state.pending) {
        let target =
            tree::with_last(|nodes| tree::find(nodes, addr).map(|i| nodes[i].target.clone()));
        if let Some(target) = target {
            focus_ref(target);
        }
    }
    let Some(focused) = focused() else {
        return;
    };
    let addr = focused.addr();
    // Only a change counts: overlays and applications may focus components
    // that are never focusable, and those keep focus.
    let Some((focusable, successor)) = tree::with_last(|nodes| {
        let index = tree::find(nodes, addr)?;
        let node = &nodes[index];
        if node.focusable {
            return Some((true, None));
        }
        // The next focusable component in the same layer, wrapping around.
        let successor = nodes[index + 1..]
            .iter()
            .chain(&nodes[..index])
            .find(|n| n.focusable && n.layer == node.layer)
            .map(|n| n.target.clone());
        Some((false, successor))
    }) else {
        // Not rendered this frame: keep focus, it may be rendered again.
        return;
    };
    let lost = STATE.with_borrow_mut(|state| {
        let lost = state.seen == Some((addr, true)) && !focusable;
        state.seen = Some((addr, focusable));
        lost
    });
    if lost {
        match successor {
            Some(target) => focus_ref(target),
            None => clear(),
        }
    }
}

/// The focused node and its ancestors, innermost first, if the focused
/// component was rendered in the last frame in a layer accepted by `scope`.
pub(crate) fn key_path(scope: impl Fn(u64) -> bool) -> Option<Vec<(ComponentRef, Handle)>> {
    let focused = focused()?;
    tree::with_last(|nodes| {
        let index = tree::find(nodes, focused.addr())?;
        scope(nodes[index].layer).then(|| tree::path(nodes, index))
    })
}

/// Clears focus when a loop run ends. Focus requested before the run starts
/// is kept, so apps can set the initial focus before running.
pub(crate) struct Active;

impl Active {
    pub(crate) fn start() -> Self {
        Self
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        let _old = STATE.with_borrow_mut(std::mem::take);
    }
}
