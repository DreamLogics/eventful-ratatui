//! `Choices`: the pickable rows inside menus, selects, combo boxes and the
//! command palette. Internal.
//!
//! It owns the rows, the highlighted row and scrolling, draws them (with an
//! optional border), and handles navigation keys and mouse picks. It knows
//! nothing about what a pick means: the owning component installs an
//! [`on_pick`](Choices::on_pick) hook, and can see keys first with
//! [`on_key`](Choices::on_key), for example to filter by typed text.
//! Separators and disabled rows are shown but never highlighted or picked.
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Widget},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// One row: a label, an optional hint, or a separator line.
#[derive(Clone, Debug, Default)]
pub(crate) struct Row {
    pub(crate) label: Line<'static>,
    /// Right-aligned text, like a shortcut.
    pub(crate) hint: String,
    pub(crate) enabled: bool,
    /// Draw a horizontal rule instead of the label; never pickable.
    pub(crate) separator: bool,
}

impl Row {
    /// An enabled row showing `label`, without a hint.
    pub(crate) fn new(label: impl Into<Line<'static>>) -> Self {
        Self {
            label: label.into(),
            hint: String::new(),
            enabled: true,
            separator: false,
        }
    }
    /// A separator between groups of rows.
    pub(crate) fn separator() -> Self {
        Self {
            separator: true,
            enabled: false,
            ..Self::default()
        }
    }
    /// Whether the highlight can rest on it.
    fn pickable(&self) -> bool {
        self.enabled && !self.separator
    }
}

/// Runs when a row is picked.
type PickHook = Rc<dyn Fn(usize)>;
/// Sees input first.
type KeyHook = Rc<dyn Fn(&Event) -> Handled>;

/// A scrolling column of [`Row`]s with one highlighted row.
pub(crate) struct Choices {
    rows: RefCell<Vec<Row>>,
    /// Index into `rows`; only ever a pickable row.
    highlight: Cell<Option<usize>>,
    /// First visible row.
    offset: Cell<usize>,
    /// Rows shown at the last render, for paging.
    height: Cell<usize>,
    /// Draw a border around the rows, as in a dropdown.
    bordered: bool,
    /// Take keyboard focus; off when another component (an input) keeps it
    /// and forwards keys.
    focusable: bool,
    /// Rows shown at most.
    max_rows: Cell<u16>,
    on_pick: RefCell<Option<PickHook>>,
    on_key: RefCell<Option<KeyHook>>,
}

impl Choices {
    /// Choices over `rows`, with the first pickable row highlighted.
    pub(crate) fn new(rows: Vec<Row>, bordered: bool, focusable: bool) -> Rc<Self> {
        let choices = Rc::new(Self {
            rows: RefCell::new(rows),
            highlight: Cell::new(None),
            offset: Cell::new(0),
            height: Cell::new(0),
            bordered,
            focusable,
            max_rows: Cell::new(12),
            on_pick: RefCell::default(),
            on_key: RefCell::default(),
        });
        choices.highlight_first();
        choices
    }

    /// Run `hook` with the row index when a row is picked, by Enter or a
    /// click. Replaces an earlier hook.
    pub(crate) fn on_pick(&self, hook: impl Fn(usize) + 'static) {
        *self.on_pick.borrow_mut() = Some(Rc::new(hook));
    }

    /// Let `hook` see input before the built-in navigation; it returns
    /// whether it handled the event. Replaces an earlier hook.
    pub(crate) fn on_key(&self, hook: impl Fn(&Event) -> Handled + 'static) {
        *self.on_key.borrow_mut() = Some(Rc::new(hook));
    }

    /// Replace the rows, scroll to the top and highlight the first pickable
    /// row, as after filtering.
    pub(crate) fn set_rows(&self, rows: Vec<Row>) {
        *self.rows.borrow_mut() = rows;
        self.offset.set(0);
        self.highlight_first();
        invalidate::invalidate();
    }

    /// The highlighted row index, if any row is pickable.
    pub(crate) fn highlight(&self) -> Option<usize> {
        self.highlight.get()
    }

    /// Highlight `index` and scroll it into view; an index that is out of
    /// range or not pickable clears the highlight.
    pub(crate) fn set_highlight(&self, index: Option<usize>) {
        let index = index.filter(|&i| self.rows.borrow().get(i).is_some_and(Row::pickable));
        self.highlight.set(index);
        self.reveal();
        invalidate::invalidate();
    }

    /// How many rows there are, separators and disabled rows included.
    pub(crate) fn len(&self) -> usize {
        self.rows.borrow().len()
    }

    fn highlight_first(&self) {
        let first = self.rows.borrow().iter().position(Row::pickable);
        self.highlight.set(first);
    }

    /// Move the highlight by `delta` pickable rows. Single steps wrap around
    /// the ends; larger ones (pages, Home/End) stop at them.
    pub(crate) fn step(&self, delta: isize) {
        let rows = self.rows.borrow();
        let pickable: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].pickable()).collect();
        drop(rows);
        if pickable.is_empty() {
            return;
        }
        let at = self
            .highlight
            .get()
            .and_then(|h| pickable.iter().position(|&i| i == h));
        let n = pickable.len() as isize;
        let next = match at {
            Some(at) if delta.abs() == 1 => (at as isize + delta).rem_euclid(n),
            Some(at) => (at as isize + delta).clamp(0, n - 1),
            None if delta < 0 => n - 1,
            None => 0,
        };
        self.set_highlight(Some(pickable[next as usize]));
    }

    /// Run the pick hook for `index`, if it is pickable.
    fn pick(&self, index: usize) {
        if !self.rows.borrow().get(index).is_some_and(Row::pickable) {
            return;
        }
        let hook = self.on_pick.borrow().clone();
        if let Some(hook) = hook {
            hook(index);
        }
    }

    /// Pick the highlighted row; returns whether a row was highlighted.
    pub(crate) fn pick_highlighted(&self) -> bool {
        match self.highlight.get() {
            Some(index) => {
                self.pick(index);
                true
            }
            None => false,
        }
    }

    /// Scroll the highlighted row into view, by the last rendered height.
    fn reveal(&self) {
        let (Some(h), height) = (self.highlight.get(), self.height.get().max(1)) else {
            return;
        };
        let offset = self.offset.get();
        if h < offset {
            self.offset.set(h);
        } else if h >= offset + height {
            self.offset.set(h + 1 - height);
        }
    }

    /// Navigation keys shared by every user: arrows, pages, ends, Enter.
    pub(crate) fn navigate(&self, event: &Event) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        if !(key.modifiers - KeyModifiers::SHIFT).is_empty() {
            return Handled::No;
        }
        let page = self.height.get().max(1) as isize;
        match key.code {
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::PageUp => self.step(-page),
            KeyCode::PageDown => self.step(page),
            KeyCode::Home => self.step(-isize::MAX / 2),
            KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Enter => return self.pick_highlighted().into(),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    /// The row at screen `row`, if any.
    fn row_at(&self, row: u16) -> Option<usize> {
        let area = mouse::area_of(self)?;
        let top = area.y + u16::from(self.bordered);
        let index = self.offset.get() + usize::from(row.checked_sub(top)?);
        (index < self.len() && row < area.bottom() - u16::from(self.bordered)).then_some(index)
    }
}

impl Component for Choices {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let inner = if self.bordered {
            let block = Block::bordered()
                .border_type(theme.border_type)
                .border_style(theme.border(false));
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };
        self.height.set(usize::from(inner.height));
        self.reveal();
        let rows = self.rows.borrow();
        let offset = self.offset.get().min(rows.len().saturating_sub(1));
        for (y, index) in (offset..rows.len())
            .take(usize::from(inner.height))
            .enumerate()
        {
            let row = &rows[index];
            let rect = Rect::new(inner.x, inner.y + y as u16, inner.width, 1);
            if row.separator {
                Line::styled("─".repeat(usize::from(rect.width)), theme.scrollbar())
                    .render(rect, buf);
                continue;
            }
            let highlighted = self.highlight.get() == Some(index);
            let style = if !row.enabled {
                theme.disabled()
            } else if highlighted {
                theme.row(Interaction::default().focused(true))
            } else {
                theme.text()
            };
            buf.set_style(rect, style);
            let mut line = Line::default().style(style);
            line.push_span(super::row_marker(&theme, highlighted && row.enabled));
            line.spans.extend(row.label.spans.iter().cloned());
            line.render(rect, buf);
            if !row.hint.is_empty() {
                let hint_style = theme.muted();
                let width = super::width(&row.hint) + 1;
                let hint = Rect::new(
                    rect.right().saturating_sub(width),
                    rect.y,
                    width.min(rect.width),
                    1,
                );
                Span::styled(row.hint.as_str(), hint_style).render(hint, buf);
            }
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        let hook = self.on_key.borrow().clone();
        if let Some(hook) = hook
            && hook(event) == Handled::Yes
        {
            return Handled::Yes;
        }
        if let Event::Mouse(m) = event {
            match m.kind {
                MouseEventKind::Moved => {
                    // Separators and disabled rows keep the highlight.
                    if let Some(index) = self.row_at(m.row)
                        && self.highlight.get() != Some(index)
                        && self.rows.borrow()[index].pickable()
                    {
                        self.set_highlight(Some(index));
                    }
                    return Handled::Yes;
                }
                MouseEventKind::Down(MouseButton::Left) => return Handled::Yes,
                MouseEventKind::Up(MouseButton::Left) => {
                    if let Some(index) = self.row_at(m.row) {
                        self.pick(index);
                    }
                    return Handled::Yes;
                }
                MouseEventKind::ScrollDown => self.step(1),
                MouseEventKind::ScrollUp => self.step(-1),
                _ => return Handled::No,
            }
            return Handled::Yes;
        }
        if self.navigate(event) == Handled::Yes {
            return Handled::Yes;
        }
        // Type-ahead on the first letter.
        if let Some(key) = super::key(event)
            && let KeyCode::Char(c) = key.code
            && (key.modifiers - KeyModifiers::SHIFT).is_empty()
        {
            let c = c.to_lowercase().to_string();
            let rows = self.rows.borrow();
            let start = self.highlight.get().map_or(0, |h| h + 1);
            let found = (0..rows.len())
                .map(|k| (start + k) % rows.len())
                .find(|&i| {
                    rows[i].pickable()
                        && rows[i]
                            .label
                            .to_string()
                            .trim_start()
                            .to_lowercase()
                            .starts_with(&c)
                });
            drop(rows);
            if let Some(index) = found {
                self.set_highlight(Some(index));
                return Handled::Yes;
            }
        }
        Handled::No
    }

    fn focusable(&self) -> bool {
        self.focusable
    }

    fn preferred_size(&self, available: Size) -> Size {
        let rows = self.rows.borrow();
        let width = rows
            .iter()
            .map(|r| {
                let hint = super::width(&r.hint);
                r.label.width() as u16 + 2 + if hint > 0 { hint + 3 } else { 0 }
            })
            .max()
            .unwrap_or(0);
        let border = 2 * u16::from(self.bordered);
        let height = (rows.len() as u16).min(self.max_rows.get()).max(1);
        Size::new(
            (width + border).min(available.width),
            (height + border).min(available.height),
        )
    }
}
