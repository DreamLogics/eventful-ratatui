//! The render tree: every component rendered through a handle records its
//! area, its parent and whether it is focusable. The last completed frame
//! drives hit-testing, focus traversal and key bubbling.
use crate::component::ComponentRef;
use ratatui_core::layout::{Position, Rect};
use std::{any::TypeId, cell::RefCell, mem};

/// Identifies the handle a component was rendered through, so the loop can
/// tell whether its root was recorded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Handle(*const (), TypeId);

impl Handle {
    pub(crate) fn of<T: ?Sized + 'static>(value: &T) -> Self {
        Self(std::ptr::from_ref(value).cast(), TypeId::of::<T>())
    }
}

/// The base layer; overlays have their own ids.
pub(crate) const BASE: u64 = 0;

/// A rendered component.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) area: Rect,
    pub(crate) target: ComponentRef,
    pub(crate) handle: Handle,
    /// The node whose `render` rendered this one.
    pub(crate) parent: Option<usize>,
    pub(crate) focusable: bool,
    /// The overlay this node was rendered in, or [`BASE`].
    pub(crate) layer: u64,
}

/// Render trees of this thread's loop.
#[derive(Default)]
struct Tree {
    /// A loop or harness is running.
    active: bool,
    /// The layer being rendered.
    layer: u64,
    /// The frame being rendered.
    frame: Vec<Node>,
    /// Nodes whose `render` is running, innermost last.
    open: Vec<usize>,
    /// The last completed frame.
    last: Vec<Node>,
}

thread_local! {
    static TREE: RefCell<Tree> = RefCell::default();
}

/// Record that a component is rendering into `area`. Keep the result alive
/// for the rest of its `render`, so components it renders become children.
pub(crate) fn record(
    area: Rect,
    handle: Handle,
    focusable: bool,
    target: impl FnOnce() -> ComponentRef,
) -> Recording {
    TREE.with_borrow_mut(|tree| {
        if !tree.active {
            return Recording(false);
        }
        let parent = tree.open.last().copied();
        tree.open.push(tree.frame.len());
        tree.frame.push(Node {
            area,
            target: target(),
            handle,
            parent,
            focusable,
            layer: tree.layer,
        });
        Recording(true)
    })
}

/// Closes a [`record`] entry.
pub(crate) struct Recording(bool);

impl Drop for Recording {
    fn drop(&mut self) {
        if self.0 {
            TREE.with_borrow_mut(|tree| tree.open.pop());
        }
    }
}

/// Turn recording on for a loop run; it is off again, and the trees are
/// cleared, on drop.
pub(crate) struct Active;

impl Active {
    pub(crate) fn start() -> Self {
        let fresh = Tree {
            active: true,
            ..Tree::default()
        };
        let _old = TREE.with_borrow_mut(|tree| mem::replace(tree, fresh));
        Self
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        // Swap out, so the old nodes drop outside the borrow.
        let _old = TREE.with_borrow_mut(mem::take);
    }
}

/// Start recording a frame.
pub(crate) fn begin_frame() {
    let _old = TREE.with_borrow_mut(|tree| {
        tree.open.clear();
        tree.layer = BASE;
        mem::take(&mut tree.frame)
    });
}

/// Record what follows in `layer`.
pub(crate) fn set_layer(layer: u64) {
    TREE.with_borrow_mut(|tree| tree.layer = layer);
}

/// Make the frame just rendered the one input is routed against.
pub(crate) fn end_frame() {
    let _old = TREE.with_borrow_mut(|tree| {
        let frame = mem::take(&mut tree.frame);
        mem::replace(&mut tree.last, frame)
    });
}

/// Read the last completed frame. Don't call components from `f`; copy out
/// what you need first.
pub(crate) fn with_last<R>(f: impl FnOnce(&[Node]) -> R) -> R {
    TREE.with_borrow(|tree| f(&tree.last))
}

/// The topmost node for the component at `addr`.
pub(crate) fn find(nodes: &[Node], addr: *const ()) -> Option<usize> {
    nodes.iter().rposition(|node| node.target.is(addr))
}

/// `index` and its ancestors, innermost first.
pub(crate) fn path(nodes: &[Node], index: usize) -> Vec<(ComponentRef, Handle)> {
    let mut path = Vec::new();
    let mut next = Some(index);
    while let Some(index) = next {
        let node = &nodes[index];
        path.push((node.target.clone(), node.handle));
        next = node.parent;
    }
    path
}

/// The topmost node containing `position`, among nodes accepted by `scope`.
pub(crate) fn hit(
    nodes: &[Node],
    position: Position,
    scope: impl Fn(&Node) -> bool,
) -> Option<usize> {
    nodes
        .iter()
        .rposition(|node| scope(node) && node.area.contains(position))
}
