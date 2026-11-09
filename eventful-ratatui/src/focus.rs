//! Keyboard focus: one focused component per loop.
//!
//! Components opt in with [`Component::focusable`](crate::Component::focusable).
//! Tab and BackTab move between focusable components in render order, a click
//! focuses the component under the pointer, and key input goes to the focused
//! component first, then bubbles through the components that rendered it, then
//! to the root. With nothing focused, the root gets every key.
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
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// Give focus to `component`. If it has not been rendered yet, it gets focus
/// once it is.
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

/// Remove focus from whatever has it.
pub fn clear() {
    let _old = STATE.with_borrow_mut(|state| {
        state.pending = None;
        state.focused.take()
    });
    invalidate::mark();
}

/// Whether `component` has focus.
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

/// The focused component, if any.
pub fn focused() -> Option<ComponentRef> {
    STATE.with_borrow(|state| state.focused.clone().filter(ComponentRef::is_alive))
}

/// Move focus to the next focusable component, wrapping around. Tab does this
/// when no component handles it.
pub fn next() {
    traverse(true);
}

/// Move focus to the previous focusable component, wrapping around. BackTab
/// does this when no component handles it.
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

/// Resolve a pending focus request against the frame just rendered.
pub(crate) fn end_frame() {
    let Some(addr) = STATE.with_borrow(|state| state.pending) else {
        return;
    };
    let target = tree::with_last(|nodes| tree::find(nodes, addr).map(|i| nodes[i].target.clone()));
    if let Some(target) = target {
        focus_ref(target);
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
