//! [`Tree`].
use super::{TreeEventsEmissions, TreeEventsEventSet, TreeEventsSignalsExt, handle};
use crate::{Component, Handled, focus, invalidate, mouse, theme};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

/// A node of a [`Tree`]: an id, a label, and children.
#[derive(Clone, Debug)]
pub struct TreeNode {
    id: String,
    label: Line<'static>,
    children: Vec<TreeNode>,
    expanded: bool,
}

impl TreeNode {
    /// A collapsed node; `id` is what events report.
    pub fn new(id: impl Into<String>, label: impl Into<Line<'static>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            expanded: false,
        }
    }
    /// Add a child.
    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }
    /// Add children.
    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(children);
        self
    }
    /// Start expanded.
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self
    }
}

/// A visible row: depth, path of child indices, and whether it has children.
#[derive(Clone)]
struct Visible {
    depth: usize,
    path: Vec<usize>,
    branch: bool,
    expanded: bool,
    id: String,
}

handle! {
    /// An expandable hierarchy, like a file tree.
    ///
    /// Up/Down move; Right expands or enters a node; Left collapses or goes
    /// to the parent; Space toggles; Enter activates; Home/End and
    /// PageUp/PageDown jump. A click selects, a click on the arrow toggles,
    /// and a double click activates. Emits [`TreeEvents`](super::TreeEvents)
    /// with node ids.
    Tree(State), events = (TreeEventsEventSet, TreeEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    roots: RefCell<Vec<TreeNode>>,
    selected: RefCell<Option<String>>,
    offset: Cell<usize>,
    reveal: Cell<bool>,
    height: Cell<usize>,
    last_click: RefCell<Option<(String, Instant)>>,
    events: TreeEventsEmissions,
}

impl Tree {
    /// A tree of `roots`.
    pub fn new(roots: impl IntoIterator<Item = TreeNode>) -> Self {
        Self(Rc::new(State {
            roots: RefCell::new(roots.into_iter().collect()),
            selected: RefCell::default(),
            offset: Cell::new(0),
            reveal: Cell::new(false),
            height: Cell::new(0),
            last_click: RefCell::default(),
            events: TreeEventsEmissions::default(),
        }))
    }
    /// Replace the nodes; the selection is kept if its id still exists.
    pub fn set_roots(&self, roots: impl IntoIterator<Item = TreeNode>) {
        *self.0.roots.borrow_mut() = roots.into_iter().collect();
        invalidate::invalidate();
    }
    /// The selected node's id.
    pub fn selection(&self) -> Option<String> {
        self.0.selected.borrow().clone()
    }
    /// Select the node with `id`, expanding its ancestors; without emitting.
    pub fn select(&self, id: &str) {
        let found = self.0.with_path(id, |roots, path| {
            let mut nodes = roots;
            for &i in &path[..path.len() - 1] {
                nodes[i].expanded = true;
                nodes = &mut nodes[i].children;
            }
        });
        if found {
            *self.0.selected.borrow_mut() = Some(id.to_owned());
            self.0.reveal.set(true);
            invalidate::invalidate();
        }
    }
    /// Expand or collapse the node with `id`.
    pub fn set_expanded(&self, id: &str, expanded: bool) {
        self.0
            .with_path(id, |roots, path| node_at(roots, path).expanded = expanded);
        invalidate::invalidate();
    }
    /// Whether the node with `id` is expanded.
    pub fn is_expanded(&self, id: &str) -> bool {
        let mut expanded = false;
        self.0
            .with_path(id, |roots, path| expanded = node_at(roots, path).expanded);
        expanded
    }
}

fn node_at<'a>(roots: &'a mut [TreeNode], path: &[usize]) -> &'a mut TreeNode {
    let mut node = &mut roots[path[0]];
    for &i in &path[1..] {
        node = &mut node.children[i];
    }
    node
}

fn find(nodes: &[TreeNode], id: &str, path: &mut Vec<usize>) -> bool {
    for (i, node) in nodes.iter().enumerate() {
        path.push(i);
        if node.id == id || find(&node.children, id, path) {
            return true;
        }
        path.pop();
    }
    false
}

impl State {
    /// Run `f` with the path to `id`; returns whether it was found.
    fn with_path(&self, id: &str, f: impl FnOnce(&mut [TreeNode], &[usize])) -> bool {
        let mut path = Vec::new();
        let mut roots = self.roots.borrow_mut();
        if find(&roots, id, &mut path) {
            f(&mut roots, &path);
            true
        } else {
            false
        }
    }

    fn visible(&self) -> Vec<Visible> {
        fn walk(nodes: &[TreeNode], depth: usize, path: &mut Vec<usize>, out: &mut Vec<Visible>) {
            for (i, node) in nodes.iter().enumerate() {
                path.push(i);
                out.push(Visible {
                    depth,
                    path: path.clone(),
                    branch: !node.children.is_empty(),
                    expanded: node.expanded,
                    id: node.id.clone(),
                });
                if node.expanded {
                    walk(&node.children, depth + 1, path, out);
                }
                path.pop();
            }
        }
        let mut out = Vec::new();
        walk(&self.roots.borrow(), 0, &mut Vec::new(), &mut out);
        out
    }

    fn select_row(&self, row: &Visible) {
        self.reveal.set(true);
        invalidate::invalidate();
        if self.selected.borrow().as_deref() != Some(row.id.as_str()) {
            *self.selected.borrow_mut() = Some(row.id.clone());
            self.events.selected().emit(row.id.clone());
        }
    }

    fn toggle(&self, row: &Visible, expanded: bool) {
        if !row.branch || row.expanded == expanded {
            return;
        }
        node_at(&mut self.roots.borrow_mut(), &row.path).expanded = expanded;
        self.events.toggled().emit(row.id.clone(), expanded);
        invalidate::invalidate();
    }

    fn key(&self, event: &Event) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        if !(key.modifiers - KeyModifiers::SHIFT).is_empty() {
            return Handled::No;
        }
        let rows = self.visible();
        if rows.is_empty() {
            return Handled::No;
        }
        let selected = self.selected.borrow().clone();
        let at = selected
            .as_ref()
            .and_then(|id| rows.iter().position(|r| &r.id == id));
        let page = self.height.get().max(1);
        let current = at.map(|i| rows[i].clone());
        match (key.code, current) {
            (KeyCode::Down, _) => {
                self.select_row(&rows[at.map_or(0, |i| (i + 1).min(rows.len() - 1))])
            }
            (KeyCode::Up, _) => self.select_row(&rows[at.map_or(0, |i| i.saturating_sub(1))]),
            (KeyCode::PageDown, _) => {
                self.select_row(&rows[at.map_or(0, |i| (i + page).min(rows.len() - 1))])
            }
            (KeyCode::PageUp, _) => {
                self.select_row(&rows[at.map_or(0, |i| i.saturating_sub(page))])
            }
            (KeyCode::Home, _) => self.select_row(&rows[0]),
            (KeyCode::End, _) => self.select_row(&rows[rows.len() - 1]),
            (KeyCode::Right, Some(row)) if row.branch && !row.expanded => self.toggle(&row, true),
            (KeyCode::Right, Some(row)) if row.branch => self.select_row(&rows[at.unwrap() + 1]),
            (KeyCode::Left, Some(row)) if row.branch && row.expanded => self.toggle(&row, false),
            (KeyCode::Left, Some(row)) if row.depth > 0 => {
                let parent = &row.path[..row.path.len() - 1];
                if let Some(parent) = rows.iter().find(|r| r.path == parent) {
                    self.select_row(parent);
                }
            }
            (KeyCode::Char(' '), Some(row)) => self.toggle(&row, !row.expanded),
            (KeyCode::Enter, Some(row)) => self.events.activated().emit(row.id.clone()),
            _ => return Handled::No,
        }
        Handled::Yes
    }
}

/// Columns of indentation per level.
const INDENT: usize = 2;

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let height = usize::from(area.height);
        self.height.set(height);
        let rows = self.visible();
        let selected = self.selected.borrow().clone();
        let at = selected
            .as_ref()
            .and_then(|id| rows.iter().position(|r| &r.id == id));
        let mut offset = self.offset.get().min(rows.len().saturating_sub(height));
        if self.reveal.take()
            && let Some(at) = at
        {
            if at < offset {
                offset = at;
            } else if at >= offset + height {
                offset = at + 1 - height;
            }
        }
        self.offset.set(offset);
        let focused = focus::is_focused(self);
        let roots = self.roots.borrow();
        for (y, row) in rows.iter().skip(offset).take(height).enumerate() {
            let rect = Rect::new(area.x, area.y + y as u16, area.width, 1);
            let mut node = &roots[row.path[0]];
            for &i in &row.path[1..] {
                node = &node.children[i];
            }
            let is_selected = selected.as_deref() == Some(row.id.as_str());
            let style = match (is_selected, focused) {
                (true, true) => theme.selected,
                (true, false) => theme.selected_unfocused,
                _ => theme.text,
            };
            buf.set_style(rect, style);
            let symbol = if row.branch {
                theme.symbols.disclosure[usize::from(row.expanded)]
            } else {
                " "
            };
            let mut line = Line::from(vec![
                Span::raw(" ".repeat(row.depth * INDENT)),
                Span::styled(
                    format!("{symbol} "),
                    if is_selected { style } else { theme.accent },
                ),
            ])
            .style(style);
            line.spans.extend(node.label.spans.iter().cloned());
            line.render(rect, buf);
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Some(lines) = super::wheel(event) {
            let max = self.visible().len().saturating_sub(self.height.get());
            self.offset.set(
                self.offset
                    .get()
                    .saturating_add_signed(lines as isize * 3)
                    .min(max),
            );
            invalidate::invalidate();
            return Handled::Yes;
        }
        if let Some(position) = super::left_press(event) {
            let Some(area) = mouse::area_of(self) else {
                return Handled::No;
            };
            let rows = self.visible();
            let Some(row) = rows
                .get(self.offset.get() + usize::from(position.y - area.y))
                .cloned()
            else {
                return Handled::Yes;
            };
            let arrow = area.x + (row.depth * INDENT) as u16;
            if row.branch && position.x <= arrow + 1 {
                self.toggle(&row, !row.expanded);
                return Handled::Yes;
            }
            let now = Instant::now();
            let double =
                self.last_click.borrow().as_ref().is_some_and(|(id, at)| {
                    *id == row.id && now - *at <= Duration::from_millis(500)
                });
            self.select_row(&row);
            if double {
                self.events.activated().emit(row.id.clone());
                *self.last_click.borrow_mut() = None;
            } else {
                *self.last_click.borrow_mut() = Some((row.id.clone(), now));
            }
            return Handled::Yes;
        }
        self.key(event)
    }

    fn focusable(&self) -> bool {
        !self.roots.borrow().is_empty()
    }

    fn preferred_size(&self, available: Size) -> Size {
        let rows = self.visible();
        let roots = self.roots.borrow();
        let width = rows
            .iter()
            .map(|row| {
                let mut node = &roots[row.path[0]];
                for &i in &row.path[1..] {
                    node = &node.children[i];
                }
                row.depth * INDENT + 2 + node.label.width()
            })
            .max()
            .unwrap_or(0) as u16;
        Size::new(
            width.min(available.width),
            (rows.len() as u16).min(available.height),
        )
    }
}
