//! Input routing and drawing, shared by the loop and the test harness.
use crate::{
    Component, Error, Handled, Result,
    component::ComponentRef,
    cursor, focus, invalidate, overlay, theme,
    tree::{self, BASE, Handle},
};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::Backend};
use ratatui_core::layout::Position;

/// The per-thread state of one loop run: render tree, focus, overlays and
/// timers. Everything is reset on start and on drop.
pub(crate) struct Session {
    _tree: tree::Active,
    _focus: focus::Active,
    _overlays: overlay::Active,
}

impl Session {
    pub(crate) fn start() -> Self {
        invalidate::reset();
        let _ = cursor::take();
        Self {
            _tree: tree::Active::start(),
            _focus: focus::Active::start(),
            _overlays: overlay::Active::start(),
        }
    }
}

type Path = Vec<(ComponentRef, Handle)>;

/// Routes input to components.
#[derive(Default)]
pub(crate) struct Dispatcher {
    /// Route mouse events by hit-testing; otherwise the root gets them.
    pub(crate) mouse: bool,
    /// The path that handled the last button press, until it is released.
    capture: Option<Path>,
}

impl Dispatcher {
    pub(crate) fn new(mouse: bool) -> Self {
        Self {
            mouse,
            capture: None,
        }
    }

    /// Route one input event. Resizes only request a redraw.
    pub(crate) fn handle<C: Component + ?Sized + 'static>(&mut self, root: &C, event: &Event) {
        match event {
            Event::Resize(..) => invalidate::invalidate(),
            Event::Mouse(mouse) if self.mouse => self.mouse(root, event, mouse),
            Event::Mouse(_) => {
                root.on_input(event);
            }
            _ => self.key(root, event),
        }
    }

    /// Keys and other non-mouse input: the focused path, then the scope root,
    /// then the defaults for Tab, BackTab and Esc.
    fn key<C: Component + ?Sized + 'static>(&mut self, root: &C, event: &Event) {
        let scope = overlay::focus_scope();
        let path = focus::key_path(|layer| overlay::in_scope(layer, scope))
            .or_else(|| (scope != BASE).then(|| layer_root(scope)).flatten())
            .unwrap_or_default();
        if offer(&path, event) == Handled::Yes {
            return;
        }
        if scope == BASE && !on_path(&path, root) && root.on_input(event) == Handled::Yes {
            return;
        }
        let Event::Key(key) = event else {
            return;
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.is_empty() => focus::next(),
            KeyCode::BackTab => focus::previous(),
            KeyCode::Esc => {
                overlay::dismiss_on_esc(scope);
            }
            _ => {}
        }
    }

    /// Mouse input: light dismiss, click to focus, then the path under the
    /// pointer or the capturing path.
    fn mouse<C: Component + ?Sized + 'static>(
        &mut self,
        root: &C,
        event: &Event,
        mouse: &MouseEvent,
    ) {
        let position = Position::new(mouse.column, mouse.row);
        tree::set_pointer(position);
        update_hover();
        let floor = overlay::mouse_floor();
        let press = matches!(mouse.kind, MouseEventKind::Down(_));
        if press {
            self.capture = None;
            if overlay::dismiss_outside(position, floor) {
                return;
            }
        }
        let captured = match mouse.kind {
            MouseEventKind::Drag(_) | MouseEventKind::Up(_) => self.capture.clone(),
            _ => None,
        };
        let (path, hit_layer) = match captured {
            Some(path) => (path, None),
            None => tree::with_last(|nodes| {
                match tree::hit(nodes, position, |n| overlay::in_scope(n.layer, floor)) {
                    Some(index) => (tree::path(nodes, index), Some(nodes[index].layer)),
                    None => (Vec::new(), None),
                }
            }),
        };
        if matches!(mouse.kind, MouseEventKind::Up(_)) {
            self.capture = None;
        }
        if press {
            click_to_focus(position, floor);
        }
        for (index, (target, _)) in path.iter().enumerate() {
            if target.offer(event) == Some(Handled::Yes) {
                if press {
                    self.capture = Some(path[index..].to_vec());
                }
                return;
            }
        }
        // The root gets what nothing in the base layer handled, unless it was
        // on the path already or an overlay blocks it.
        let base = hit_layer.is_none_or(|layer| layer == BASE);
        if floor == BASE
            && base
            && !on_path(&path, root)
            && root.on_input(event) == Handled::Yes
            && press
        {
            // An empty path: only the root, below, gets the drags.
            self.capture = Some(Vec::new());
        }
    }

    /// Draw one frame of `root` and the overlays above it. Returns where the
    /// cursor was shown, if anywhere.
    pub(crate) fn draw<B, C>(
        &self,
        terminal: &mut Terminal<B>,
        root: &C,
    ) -> Result<Option<Position>>
    where
        B: Backend,
        B::Error: Send + Sync + 'static,
        C: Component + ?Sized,
    {
        invalidate::clear();
        tree::begin_frame();
        let _ = cursor::take();
        let mut shown = None;
        terminal
            .draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();
                buf.set_style(area, theme::current().screen());
                root.render(area, buf);
                overlay::render(area, buf);
                shown = cursor::take();
                if let Some(position) = shown {
                    frame.set_cursor_position(position);
                }
            })
            .map_err(|err| Error::Backend(Box::new(err)))?;
        tree::end_frame();
        focus::end_frame();
        overlay::end_frame();
        if self.mouse {
            update_hover();
        }
        Ok(shown)
    }
}

/// Offer `event` along `path`, innermost first.
fn offer(path: &Path, event: &Event) -> Handled {
    for (target, _) in path {
        if target.offer(event) == Some(Handled::Yes) {
            return Handled::Yes;
        }
    }
    Handled::No
}

/// Whether the root itself was recorded on `path`.
fn on_path<C: ?Sized + 'static>(path: &Path, root: &C) -> bool {
    let root = Handle::of(root);
    path.iter().any(|(_, handle)| *handle == root)
}

/// The path to the root node of overlay `layer`.
fn layer_root(layer: u64) -> Option<Path> {
    tree::with_last(|nodes| {
        let index = nodes.iter().position(|n| n.layer == layer)?;
        Some(tree::path(nodes, index))
    })
}

/// Recompute what is under the pointer; redraw if it changed.
fn update_hover() {
    let Some(position) = tree::pointer() else {
        return;
    };
    let floor = overlay::mouse_floor();
    let hovered = tree::with_last(|nodes| {
        let mut hovered = Vec::new();
        let mut next = tree::hit(nodes, position, |n| overlay::in_scope(n.layer, floor));
        while let Some(index) = next {
            hovered.push(nodes[index].target.addr());
            next = nodes[index].parent;
        }
        hovered
    });
    if tree::set_hovered(hovered) {
        invalidate::invalidate();
    }
}

/// Focus the innermost focusable component under `position`.
fn click_to_focus(position: Position, floor: u64) {
    let target = tree::with_last(|nodes| {
        let mut next = tree::hit(nodes, position, |n| overlay::in_scope(n.layer, floor));
        while let Some(index) = next {
            if nodes[index].focusable {
                return Some(nodes[index].target.clone());
            }
            next = nodes[index].parent;
        }
        None
    });
    if let Some(target) = target
        && focus::focused().is_none_or(|focused| !focused.is(target.addr()))
    {
        focus::focus_ref(target);
    }
}
