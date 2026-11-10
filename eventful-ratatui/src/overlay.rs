//! Overlays: components drawn above the root, for dialogs, menus, dropdowns,
//! toasts and palettes.
//!
//! Open one with [`overlay`] and keep the returned [`Overlay`]: dropping it
//! closes the overlay, like a scoped subscription. Overlays stack in opening
//! order, and each one is one of three kinds, or a mix of their options:
//!
//! - **popup** (the default): takes focus, and closes on Esc or a click
//!   outside it. Menus and dropdowns.
//! - **[`modal`](OverlayBuilder::modal)**: also dims what is below it and
//!   blocks input to it; a click outside does not close it. Dialogs.
//! - **[`passive`](OverlayBuilder::passive)**: takes no focus and closes only
//!   when told. Toasts.
//!
//! An overlay that takes focus focuses its first focusable component when it
//! opens, keeps Tab inside itself, and gives the focus back when it closes.
//! Keys go to the overlay's focused component, then bubble up to the overlay
//! root; they do not reach the app root while a focus-taking overlay is open.
//!
//! ```
//! use eventful_ratatui::{overlay::{Placement, overlay}, prelude::*};
//!
//! struct Note;
//! impl Component for Note {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         Paragraph::new("Saved").block(Block::bordered()).render(area, buf);
//!     }
//!     fn preferred_size(&self, _: Size) -> Size {
//!         Size::new(12, 3)
//!     }
//! }
//!
//! let toast = overlay(Note).placement(Placement::BottomRight).passive().open();
//! assert!(toast.is_open());
//! drop(toast);
//! ```
use crate::{
    Component,
    component::ComponentRef,
    focus, invalidate,
    theme::{self},
    tree::{self, BASE},
};
use ratatui::widgets::Clear;
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::Widget,
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// Where an overlay goes on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Placement {
    /// Centered, at the component's preferred size.
    Center,
    /// Just below `anchor`, at least as wide as it, or above it when there is
    /// more room there. Dropdowns and context menus.
    Below(Rect),
    /// Centered horizontally near the top, at the preferred size, like a
    /// command palette.
    Top,
    /// At a corner with a one-cell margin, at the preferred size.
    TopRight,
    /// At a corner with a one-cell margin, at the preferred size.
    BottomRight,
    /// At a corner with a one-cell margin, at the preferred size.
    BottomLeft,
    /// At a corner with a one-cell margin, at the preferred size.
    TopLeft,
    /// Exactly this area, clipped to the screen.
    At(Rect),
}

impl Placement {
    /// The area for `component` on a `screen`.
    fn resolve(self, component: &dyn Component, screen: Rect) -> Rect {
        let margin = screen.inner(ratatui_core::layout::Margin::new(1, 1));
        let fit = |size: Size, max: Size| {
            Size::new(size.width.min(max.width), size.height.min(max.height))
        };
        let preferred = |max: Size| fit(component.preferred_size(max), max);
        let area = match self {
            Self::Center => {
                let size = preferred(margin.as_size());
                Rect::new(
                    screen.x + (screen.width - size.width) / 2,
                    screen.y + (screen.height - size.height) / 2,
                    size.width,
                    size.height,
                )
            }
            Self::Below(anchor) => {
                let below = screen.bottom().saturating_sub(anchor.bottom());
                let above = anchor.y.saturating_sub(screen.y);
                let room = below.max(above);
                let size = preferred(Size::new(screen.width, room));
                let width = size.width.max(anchor.width).min(screen.width);
                let x = anchor.x.min(screen.right().saturating_sub(width));
                let y = if size.height <= below || below >= above {
                    anchor.bottom()
                } else {
                    anchor.y - size.height
                };
                Rect::new(x, y, width, size.height)
            }
            Self::Top => {
                let size = preferred(Size::new(margin.width, margin.height.saturating_sub(2)));
                Rect::new(
                    screen.x + (screen.width - size.width) / 2,
                    margin.y + 2.min(margin.height.saturating_sub(size.height)),
                    size.width,
                    size.height,
                )
            }
            Self::TopRight | Self::BottomRight | Self::BottomLeft | Self::TopLeft => {
                let size = preferred(margin.as_size());
                let x = match self {
                    Self::TopLeft | Self::BottomLeft => margin.x,
                    _ => margin.right() - size.width,
                };
                let y = match self {
                    Self::TopLeft | Self::TopRight => margin.y,
                    _ => margin.bottom() - size.height,
                };
                Rect::new(x, y, size.width, size.height)
            }
            Self::At(area) => area,
        };
        area.intersection(screen)
    }
}

/// Settings for an overlay. Start with [`overlay`], finish with
/// [`open`](Self::open).
pub struct OverlayBuilder {
    component: Rc<dyn Component>,
    placement: Placement,
    flags: Flags,
}

#[derive(Clone, Copy, Debug)]
struct Flags {
    /// Patch the theme's backdrop over everything below.
    dim: bool,
    /// Ignore input to everything below.
    block: bool,
    /// Focus the overlay and keep Tab inside it.
    take_focus: bool,
    close_on_esc: bool,
    close_on_click_outside: bool,
}

/// Prepare an overlay showing `component`: a popup at the center, until
/// changed.
pub fn overlay(component: impl Component + 'static) -> OverlayBuilder {
    OverlayBuilder {
        component: Rc::new(component),
        placement: Placement::Center,
        flags: Flags {
            dim: false,
            block: false,
            take_focus: true,
            close_on_esc: true,
            close_on_click_outside: true,
        },
    }
}

impl OverlayBuilder {
    /// Where to show it.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }
    /// Dim and block everything below; Esc still closes it, a click outside
    /// does not.
    pub fn modal(mut self) -> Self {
        self.flags.dim = true;
        self.flags.block = true;
        self.flags.take_focus = true;
        self.flags.close_on_click_outside = false;
        self
    }
    /// Take no focus and close only when told, like a toast.
    pub fn passive(mut self) -> Self {
        self.flags.take_focus = false;
        self.flags.close_on_esc = false;
        self.flags.close_on_click_outside = false;
        self
    }
    /// Whether Esc closes it, if nothing inside handles Esc first.
    pub fn close_on_esc(mut self, close: bool) -> Self {
        self.flags.close_on_esc = close;
        self
    }
    /// Whether a click outside closes it. The click is consumed.
    pub fn close_on_click_outside(mut self, close: bool) -> Self {
        self.flags.close_on_click_outside = close;
        self
    }
    /// Show it. It stays open until closed, dismissed, or the handle drops.
    pub fn open(self) -> Overlay {
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        });
        let shared = Rc::new(Shared::default());
        let saved_focus = if self.flags.take_focus {
            focus::focused()
        } else {
            None
        };
        LAYERS.with_borrow_mut(|layers| {
            layers.push(Layer {
                id,
                component: self.component,
                placement: self.placement,
                flags: self.flags,
                saved_focus,
                needs_focus: self.flags.take_focus,
                area: Rect::ZERO,
                shared: shared.clone(),
            });
        });
        invalidate::invalidate();
        Overlay {
            id,
            shared,
            detached: false,
        }
    }
}

impl fmt::Debug for OverlayBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OverlayBuilder")
            .field("placement", &self.placement)
            .field("flags", &self.flags)
            .finish_non_exhaustive()
    }
}

/// State shared between a layer and its handles.
#[derive(Default)]
struct Shared {
    /// Tasks awaiting [`Closed`].
    wakers: RefCell<Vec<Waker>>,
}

/// An open overlay. Dropping it closes the overlay, unless it was
/// [`detach`](Self::detach)ed.
pub struct Overlay {
    id: u64,
    shared: Rc<Shared>,
    detached: bool,
}

impl Overlay {
    /// Close it. Does nothing if it is already closed.
    pub fn close(&self) {
        close(self.id);
    }
    /// Whether it is still open: not closed, dismissed or ended with the loop.
    pub fn is_open(&self) -> bool {
        LAYERS.with_borrow(|layers| layers.iter().any(|l| l.id == self.id))
    }
    /// Resolves once it closes, for whatever reason.
    pub fn closed(&self) -> Closed {
        Closed {
            id: self.id,
            shared: self.shared.clone(),
        }
    }
    /// Keep it open without the handle. It closes when dismissed, or through
    /// a [`Closer`] taken earlier.
    pub fn detach(mut self) {
        self.detached = true;
    }
    /// A handle that can close the overlay but does not keep it open; for
    /// the overlay's own component, like a dialog's buttons.
    pub fn closer(&self) -> Closer {
        Closer(self.id)
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        if !self.detached {
            close(self.id);
        }
    }
}

impl fmt::Debug for Overlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Overlay")
            .field("id", &self.id)
            .field("open", &self.is_open())
            .finish()
    }
}

/// Closes an overlay without keeping it open. Cloneable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closer(u64);

impl Closer {
    /// Close the overlay. Does nothing if it is already closed.
    pub fn close(self) {
        close(self.0);
    }
    /// Whether the overlay is still open.
    pub fn is_open(self) -> bool {
        LAYERS.with_borrow(|layers| layers.iter().any(|l| l.id == self.0))
    }
}

/// Resolves once an overlay closes. See [`Overlay::closed`].
#[derive(Clone)]
pub struct Closed {
    id: u64,
    shared: Rc<Shared>,
}

impl Future for Closed {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if LAYERS.with_borrow(|layers| layers.iter().any(|l| l.id == self.id)) {
            self.shared.wakers.borrow_mut().push(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl fmt::Debug for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closed").field("id", &self.id).finish()
    }
}

/// An open overlay.
struct Layer {
    id: u64,
    component: Rc<dyn Component>,
    placement: Placement,
    flags: Flags,
    /// Focus to restore on close.
    saved_focus: Option<ComponentRef>,
    /// Focus the first focusable component after the next frame.
    needs_focus: bool,
    /// Where it was last drawn.
    area: Rect,
    shared: Rc<Shared>,
}

thread_local! {
    static LAYERS: RefCell<Vec<Layer>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

/// Close layer `id`, restoring focus if it took it.
fn close(id: u64) {
    // During thread teardown the layers may already be gone.
    let layer = LAYERS.try_with(|layers| {
        let mut layers = layers.borrow_mut();
        let index = layers.iter().position(|l| l.id == id)?;
        Some(layers.remove(index))
    });
    let Ok(Some(layer)) = layer else {
        return;
    };
    if layer.flags.take_focus {
        let focus_inside = focus::focused().is_some_and(|focused| {
            tree::with_last(|nodes| {
                tree::find(nodes, focused.addr()).is_none_or(|i| nodes[i].layer == id)
            })
        });
        if focus_inside || focus::focused().is_none() {
            focus::restore(layer.saved_focus.clone());
        }
    }
    for waker in layer.shared.wakers.take() {
        waker.wake();
    }
    invalidate::invalidate();
    // The component drops here, outside the borrow.
}

/// Render the layers above the root.
pub(crate) fn render(screen: Rect, buf: &mut Buffer) {
    let layers: Vec<(u64, Rc<dyn Component>, Placement, bool)> = LAYERS.with_borrow(|layers| {
        layers
            .iter()
            .map(|l| (l.id, l.component.clone(), l.placement, l.flags.dim))
            .collect()
    });
    let theme = theme::current();
    for (id, component, placement, dim) in layers {
        if dim {
            dim_backdrop(buf, screen, theme.palette.background);
        }
        let area = placement.resolve(&*component, screen);
        Clear.render(area, buf);
        buf.set_style(area, theme.surface());
        tree::set_layer(id);
        component.render(area, buf);
        LAYERS.with_borrow_mut(|layers| {
            if let Some(layer) = layers.iter_mut().find(|l| l.id == id) {
                layer.area = area;
            }
        });
    }
    tree::set_layer(BASE);
}

/// Blend every cell of `area` toward `background`, so a modal stands out
/// from what it covers while that stays readable.
fn dim_backdrop(buf: &mut Buffer, area: Rect, background: ratatui::style::Color) {
    use crate::theme::{BACKDROP_DIM, color::blend};
    if crate::theme::color::channels(background).is_none() {
        buf.set_style(
            area,
            ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::DIM),
        );
        return;
    }
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.fg = blend(cell.fg, background, BACKDROP_DIM);
            cell.bg = blend(cell.bg, background, BACKDROP_DIM);
        }
    }
}

/// Focus new focus-taking layers, now that they have been rendered.
pub(crate) fn end_frame() {
    let pending: Vec<u64> = LAYERS.with_borrow_mut(|layers| {
        layers
            .iter_mut()
            .filter_map(|l| std::mem::take(&mut l.needs_focus).then_some(l.id))
            .collect()
    });
    for id in pending {
        // Something inside already asked for focus, like a dialog's default
        // button.
        let inside = focus::focused().is_some_and(|focused| {
            tree::with_last(|nodes| {
                tree::find(nodes, focused.addr()).is_some_and(|i| nodes[i].layer == id)
            })
        });
        if inside {
            continue;
        }
        let target = tree::with_last(|nodes| {
            let in_layer = || nodes.iter().filter(|n| n.layer == id);
            in_layer()
                .find(|n| n.focusable)
                .or_else(|| in_layer().next())
                .map(|n| n.target.clone())
        });
        if let Some(target) = target {
            focus::focus_ref(target);
        }
    }
}

/// The layer keys are scoped to: the topmost focus-taking layer, or
/// [`BASE`].
pub(crate) fn focus_scope() -> u64 {
    LAYERS.with_borrow(|layers| {
        layers
            .iter()
            .rev()
            .find(|l| l.flags.take_focus)
            .map_or(BASE, |l| l.id)
    })
}

/// The lowest layer that receives mouse input: the topmost blocking layer,
/// or [`BASE`].
pub(crate) fn mouse_floor() -> u64 {
    LAYERS.with_borrow(|layers| {
        layers
            .iter()
            .rev()
            .find(|l| l.flags.block)
            .map_or(BASE, |l| l.id)
    })
}

/// Stack position of `layer`; [`BASE`] is lowest.
fn rank(layers: &[Layer], layer: u64) -> Option<usize> {
    if layer == BASE {
        return Some(0);
    }
    layers.iter().position(|l| l.id == layer).map(|i| i + 1)
}

/// Whether `layer` is `floor` or above it.
pub(crate) fn in_scope(layer: u64, floor: u64) -> bool {
    LAYERS.with_borrow(|layers| match (rank(layers, layer), rank(layers, floor)) {
        (Some(layer), Some(floor)) => layer >= floor,
        _ => false,
    })
}

/// Close the topmost layer at or above `floor` that closes on Esc. Returns
/// whether one closed.
pub(crate) fn dismiss_on_esc(floor: u64) -> bool {
    let id = LAYERS.with_borrow(|layers| {
        let floor = rank(layers, floor).unwrap_or(0);
        layers
            .iter()
            .enumerate()
            .rev()
            .take_while(|(i, _)| i + 1 >= floor)
            .find(|(_, l)| l.flags.close_on_esc)
            .map(|(_, l)| l.id)
    });
    id.inspect(|id| close(*id)).is_some()
}

/// Close the light-dismiss layers above `floor` that a press at `position`
/// is outside of, topmost first, stopping at a layer containing it. Returns
/// whether any closed.
pub(crate) fn dismiss_outside(position: ratatui_core::layout::Position, floor: u64) -> bool {
    let ids: Vec<u64> = LAYERS.with_borrow(|layers| {
        let floor = rank(layers, floor).unwrap_or(0);
        let mut ids = Vec::new();
        for (i, layer) in layers.iter().enumerate().rev() {
            if i + 1 < floor || layer.area.contains(position) {
                break;
            }
            if layer.flags.close_on_click_outside {
                ids.push(layer.id);
            } else if layer.flags.block {
                break;
            }
        }
        ids
    });
    for id in &ids {
        close(*id);
    }
    !ids.is_empty()
}

/// Whether any overlay is open.
pub fn any_open() -> bool {
    LAYERS.with_borrow(|layers| !layers.is_empty())
}

/// Close every overlay.
pub fn close_all() {
    let ids: Vec<u64> = LAYERS.with_borrow(|layers| layers.iter().map(|l| l.id).collect());
    for id in ids.into_iter().rev() {
        close(id);
    }
}

/// Closes the remaining layers when a loop run ends. Overlays opened before
/// the run starts are kept.
pub(crate) struct Active;

impl Active {
    pub(crate) fn start() -> Self {
        Self
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        let old = LAYERS.with_borrow_mut(std::mem::take);
        for layer in old {
            for waker in layer.shared.wakers.take() {
                waker.wake();
            }
        }
    }
}
