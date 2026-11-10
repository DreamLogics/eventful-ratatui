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
    /// The area the component was given, before any viewport transform.
    pub(crate) content_area: Rect,
}

/// A scrolled viewport: content coordinates plus `(dx, dy)` are screen
/// coordinates, clipped to `clip`.
#[derive(Clone, Copy, Debug)]
struct Viewport {
    dx: i32,
    dy: i32,
    clip: Rect,
}

impl Viewport {
    fn area(self, area: Rect) -> Rect {
        let x = i32::from(area.x) + self.dx;
        let y = i32::from(area.y) + self.dy;
        let (Ok(x), Ok(y)) = (u16::try_from(x.max(0)), u16::try_from(y.max(0))) else {
            return Rect::ZERO;
        };
        let width = (i32::from(area.width) + (i32::from(area.x) + self.dx).min(0)).max(0);
        let height = (i32::from(area.height) + (i32::from(area.y) + self.dy).min(0)).max(0);
        let moved = Rect::new(x, y, width as u16, height as u16);
        let clipped = moved.intersection(self.clip);
        if clipped.is_empty() {
            Rect::new(clipped.x, clipped.y, 0, 0)
        } else {
            clipped
        }
    }
    fn position(self, position: Position) -> Option<Position> {
        let x = u16::try_from(i32::from(position.x) + self.dx).ok()?;
        let y = u16::try_from(i32::from(position.y) + self.dy).ok()?;
        let position = Position::new(x, y);
        self.clip.contains(position).then_some(position)
    }
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
    /// Viewports being rendered into, innermost last.
    viewports: Vec<Viewport>,
    /// The last completed frame.
    last: Vec<Node>,
    /// Where the mouse pointer was last seen.
    pointer: Option<Position>,
    /// The components under the pointer, innermost first.
    hovered: Vec<*const ()>,
    /// Components that read the pointer position in the frame being
    /// rendered, and in the last one: they redraw on every move over them.
    tracking: Vec<*const ()>,
    tracking_last: Vec<*const ()>,
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
        let content_area = area;
        let area = tree
            .viewports
            .iter()
            .rev()
            .fold(area, |area, v| v.area(area));
        tree.frame.push(Node {
            area,
            target: target(),
            handle,
            parent,
            focusable,
            layer: tree.layer,
            content_area,
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

/// Render what follows into a viewport: content coordinates shifted by
/// `(dx, dy)` and clipped to `clip`, until the guard drops. Nested viewports
/// compose.
pub(crate) fn push_viewport(dx: i32, dy: i32, clip: Rect) -> ViewportGuard {
    TREE.with_borrow_mut(|tree| tree.viewports.push(Viewport { dx, dy, clip }));
    ViewportGuard
}

/// Ends a [`push_viewport`].
pub(crate) struct ViewportGuard;

impl Drop for ViewportGuard {
    fn drop(&mut self) {
        TREE.with_borrow_mut(|tree| tree.viewports.pop());
    }
}

/// Map a content position through the open viewports to the screen.
pub(crate) fn to_screen(position: Position) -> Option<Position> {
    TREE.with_borrow(|tree| {
        tree.viewports
            .iter()
            .rev()
            .try_fold(position, |position, v| v.position(position))
    })
}

/// Start recording a frame.
pub(crate) fn begin_frame() {
    let _old = TREE.with_borrow_mut(|tree| {
        tree.open.clear();
        tree.viewports.clear();
        tree.layer = BASE;
        tree.tracking.clear();
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
        mem::swap(&mut tree.tracking, &mut tree.tracking_last);
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

/// Remember where the pointer is. Returns whether it moved to another
/// cell while the innermost hovered component reads the pointer position.
pub(crate) fn set_pointer(position: Position) -> bool {
    TREE.with_borrow_mut(|tree| {
        let moved = tree.pointer != Some(position);
        tree.pointer = Some(position);
        // Only the innermost one: over a child, the child is what changes.
        moved
            && tree
                .hovered
                .first()
                .is_some_and(|addr| tree.tracking_last.contains(addr))
    })
}

/// Note that the component at `addr` reads the pointer position while
/// rendering.
pub(crate) fn track_pointer(addr: *const ()) {
    TREE.with_borrow_mut(|tree| {
        if tree.active && !tree.tracking.contains(&addr) {
            tree.tracking.push(addr);
        }
    });
}

/// Where the pointer was last seen.
pub(crate) fn pointer() -> Option<Position> {
    TREE.with_borrow(|tree| tree.pointer)
}

/// Replace the hovered components. Returns whether they changed.
pub(crate) fn set_hovered(hovered: Vec<*const ()>) -> bool {
    TREE.with_borrow_mut(|tree| {
        let changed = tree.hovered != hovered;
        tree.hovered = hovered;
        changed
    })
}

/// Whether the component at `addr` is under the pointer.
pub(crate) fn is_hovered(addr: *const ()) -> bool {
    TREE.with_borrow(|tree| tree.hovered.contains(&addr))
}
