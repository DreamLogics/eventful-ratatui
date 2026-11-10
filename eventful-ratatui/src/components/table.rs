//! [`Table`].
use super::{SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    rc::Rc,
    time::{Duration, Instant},
};

/// A [`Table`] column: its header, width and alignment.
///
/// Widths are ratatui layout constraints, solved across the table's width
/// like a horizontal [`Layout`]: `Constraint::Length(6)` for a fixed column,
/// `Constraint::Fill(1)` to share the rest.
#[derive(Clone, Debug)]
pub struct Column {
    title: String,
    width: Constraint,
    alignment: Alignment,
}

impl Column {
    /// A left-aligned column titled `title`, sized by `width`.
    pub fn new(title: impl Into<String>, width: Constraint) -> Self {
        Self {
            title: title.into(),
            width,
            alignment: Alignment::Left,
        }
    }
    /// Align the cells and header to the right, for numbers.
    pub fn right(mut self) -> Self {
        self.alignment = Alignment::Right;
        self
    }
    /// Center the cells and header.
    pub fn center(mut self) -> Self {
        self.alignment = Alignment::Center;
        self
    }
}

handle! {
    /// Rows of cells under column headers, with a selected row and optional
    /// sorting.
    ///
    /// Up/Down, PageUp/PageDown and Home/End move the selection as in
    /// [`List`](super::List), and Enter activates the selected row; there is
    /// no type-ahead search or multi-select. A click selects a row, a double
    /// click activates it, the wheel scrolls, and with [`sortable`](Self::sortable) a
    /// click on a header sorts by that column (again to reverse). Sorting
    /// compares numbers inside the text numerically. Events report indices
    /// into the rows as added, whatever the sort order:
    /// [`selected`](super::SelectEventsSignalsExt::selected) and
    /// [`activated`](super::SelectEventsSignalsExt::activated).
    ///
    /// The header takes the first row and stays in place while the rows
    /// scroll. A row with fewer cells than columns leaves the rest empty.
    ///
    /// ```
    /// use eventful_ratatui::{components::{Column, Table}, prelude::*, testing::Harness};
    ///
    /// let people = Table::new([
    ///     Column::new("Name", Constraint::Fill(1)),
    ///     Column::new("Age", Constraint::Length(4)).right(),
    /// ])
    /// .sortable()
    /// .row(["Ada", "36"])
    /// .row(["Grace", "85"])
    /// .row(["Alan", "9"]);
    /// let mut ui = Harness::new(people.clone(), 16, 4);
    /// assert_eq!(ui.lines(), [
    ///     " Name        Age",
    ///     " Ada          36",
    ///     " Grace        85",
    ///     " Alan          9",
    /// ]);
    ///
    /// // Click the "Age" header: numbers sort as numbers, not as text.
    /// ui.click(12, 0);
    /// assert_eq!(people.sorting(), Some((1, true)));
    /// assert_eq!(people.display_order(), [2, 0, 1]);
    /// // Indices stay those of the rows as added.
    /// ui.click(2, 1);
    /// assert_eq!(people.selection(), Some(2));
    /// assert_eq!(people.row_cells(2).unwrap(), ["Alan", "9"]);
    /// ```
    Table(State), events = (SelectEventsEventSet, SelectEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    columns: Vec<Column>,
    rows: RefCell<Vec<Vec<Line<'static>>>>,
    /// Display position to row index.
    order: RefCell<Vec<usize>>,
    sort: Cell<Option<(usize, bool)>>,
    sortable: Cell<bool>,
    /// Selected row index.
    selected: Cell<Option<usize>>,
    offset: Cell<usize>,
    reveal: Cell<bool>,
    height: Cell<usize>,
    /// Column areas at the last render, for header clicks.
    columns_at: RefCell<Vec<Rect>>,
    last_click: Cell<Option<(usize, Instant)>>,
    events: SelectEventsEmissions,
}

impl Table {
    /// An empty, unsorted table with `columns`. The columns are fixed; add
    /// rows with [`row`](Self::row) or [`push`](Self::push).
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self(Rc::new(State {
            columns: columns.into_iter().collect(),
            rows: RefCell::default(),
            order: RefCell::default(),
            sort: Cell::new(None),
            sortable: Cell::new(false),
            selected: Cell::new(None),
            offset: Cell::new(0),
            reveal: Cell::new(false),
            height: Cell::new(0),
            columns_at: RefCell::default(),
            last_click: Cell::new(None),
            events: SelectEventsEmissions::default(),
        }))
    }
    /// Let header clicks sort: a click sorts ascending by that column, and
    /// another click on the same header reverses it. The sorted header shows
    /// an arrow. Without this, [`sort_by`](Self::sort_by) still works.
    pub fn sortable(self) -> Self {
        self.0.sortable.set(true);
        self
    }
    /// Add a row of cells, one per column in order; extra cells are not
    /// shown.
    pub fn row<I: IntoIterator<Item = C>, C: Into<Line<'static>>>(self, cells: I) -> Self {
        self.push(cells);
        self
    }
    /// Append a row of cells to a table in use. It takes its place in the
    /// current sort order; the selection stays on the same row.
    pub fn push<I: IntoIterator<Item = C>, C: Into<Line<'static>>>(&self, cells: I) {
        self.0
            .rows
            .borrow_mut()
            .push(cells.into_iter().map(Into::into).collect());
        self.0.resort();
    }
    /// Replace all rows; the selection is cleared and the sort order kept.
    pub fn set_rows<R, I, C>(&self, rows: R)
    where
        R: IntoIterator<Item = I>,
        I: IntoIterator<Item = C>,
        C: Into<Line<'static>>,
    {
        *self.0.rows.borrow_mut() = rows
            .into_iter()
            .map(|r| r.into_iter().map(Into::into).collect())
            .collect();
        self.0.selected.set(None);
        self.0.resort();
    }
    /// Sort by `column` (from zero), ascending or not, without emitting an
    /// event. Text is compared with runs of digits as numbers, so "item 9"
    /// comes before "item 10". The sort is stable, and rows missing the cell
    /// sort as empty.
    pub fn sort_by(&self, column: usize, ascending: bool) {
        self.0.sort.set(Some((column, ascending)));
        self.0.resort();
    }
    /// The sort column and whether it is ascending, if sorted.
    pub fn sorting(&self) -> Option<(usize, bool)> {
        self.0.sort.get()
    }
    /// How many rows there are.
    pub fn len(&self) -> usize {
        self.0.rows.borrow().len()
    }
    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The plain text of the cells of row `index`, as added, or `None` if out
    /// of range.
    pub fn row_cells(&self, index: usize) -> Option<Vec<String>> {
        self.0
            .rows
            .borrow()
            .get(index)
            .map(|r| r.iter().map(ToString::to_string).collect())
    }
    /// The selected row, as an index into the rows as added, not the display
    /// position.
    pub fn selection(&self) -> Option<usize> {
        self.0.selected.get()
    }
    /// Select a row (index as added) and scroll it into view, without
    /// emitting `selected`. `None`, or an index out of range, clears the
    /// selection.
    pub fn set_selection(&self, index: Option<usize>) {
        self.0.selected.set(index.filter(|&i| i < self.len()));
        self.0.reveal.set(true);
        invalidate::invalidate();
    }
    /// Row indices (as added) in display order: the identity until sorted.
    pub fn display_order(&self) -> Vec<usize> {
        self.0.order.borrow().clone()
    }
}

/// Compare text with runs of digits as numbers: "item 9" < "item 10".
pub(crate) fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut n = String::new();
                    while let Some(c) = it.next_if(char::is_ascii_digit) {
                        n.push(c);
                    }
                    n
                };
                let (n, m) = (take(&mut a), take(&mut b));
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                match n.len().cmp(&m.len()).then_with(|| n.cmp(m)) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(x), Some(y)) => {
                match x.to_lowercase().cmp(y.to_lowercase()) {
                    Ordering::Equal => {}
                    other => return other,
                }
                a.next();
                b.next();
            }
        }
    }
}

impl State {
    fn resort(&self) {
        let rows = self.rows.borrow();
        let mut order: Vec<usize> = (0..rows.len()).collect();
        if let Some((column, ascending)) = self.sort.get() {
            let text = |i: usize| {
                rows[i]
                    .get(column)
                    .map(ToString::to_string)
                    .unwrap_or_default()
            };
            order.sort_by(|&x, &y| {
                let ordering = natural(&text(x), &text(y));
                if ascending {
                    ordering
                } else {
                    ordering.reverse()
                }
            });
        }
        *self.order.borrow_mut() = order;
        self.reveal.set(true);
        invalidate::invalidate();
    }

    /// The display position of the selection.
    fn position(&self) -> Option<usize> {
        let selected = self.selected.get()?;
        self.order.borrow().iter().position(|&i| i == selected)
    }

    fn select_at(&self, position: usize) {
        let order = self.order.borrow();
        let Some(&index) = order.get(position.min(order.len().saturating_sub(1))) else {
            return;
        };
        drop(order);
        self.reveal.set(true);
        invalidate::invalidate();
        if self.selected.get() != Some(index) {
            self.selected.set(Some(index));
            self.events.selected().emit(index);
        }
    }

    fn step(&self, delta: isize) {
        let next = match self.position() {
            Some(p) => p.saturating_add_signed(delta),
            None => 0,
        };
        self.select_at(next);
    }

    fn header_click(&self, column: usize) {
        let ascending = match self.sort.get() {
            Some((c, ascending)) if c == column => !ascending,
            _ => true,
        };
        self.sort.set(Some((column, ascending)));
        self.resort();
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        if area.height == 0 {
            return;
        }
        // Large wells lift on focus only; rows show the hover.
        let well = Interaction::of(self).hovered(false);
        super::fill(buf, area, theme.well(well));
        let body_height = usize::from(area.height - 1);
        self.height.set(body_height);
        let order = self.order.borrow();
        let len = order.len();
        let scrollbar = len > body_height;
        let width = area.width.saturating_sub(u16::from(scrollbar));
        let columns = Layout::horizontal(self.columns.iter().map(|c| c.width))
            .spacing(1)
            .split(Rect::new(area.x + 1, area.y, width.saturating_sub(1), 1));
        *self.columns_at.borrow_mut() = columns.to_vec();

        // Header.
        for (i, (column, rect)) in self.columns.iter().zip(columns.iter()).enumerate() {
            let marker = match self.sort.get() {
                Some((c, ascending)) if c == i => theme.symbols.sort[usize::from(!ascending)],
                _ => "",
            };
            Line::from(vec![
                Span::styled(
                    column.title.as_str(),
                    theme.muted().add_modifier(Modifier::BOLD),
                ),
                Span::styled(marker, theme.accent()),
            ])
            .alignment(column.alignment)
            .render(*rect, buf);
        }

        // Rows, keeping the selection visible after it moved.
        let position = self.position();
        let mut offset = self.offset.get().min(len.saturating_sub(body_height));
        if self.reveal.take()
            && let Some(p) = position
        {
            if p < offset {
                offset = p;
            } else if p >= offset + body_height {
                offset = p + 1 - body_height;
            }
        }
        self.offset.set(offset);
        let rows = self.rows.borrow();
        let body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
        let hovered = super::hovered_row(self, body).map(|row| offset + usize::from(row));
        for (y, display) in (offset..len.min(offset + body_height)).enumerate() {
            let index = order[display];
            let row = Rect::new(area.x, area.y + 1 + y as u16, width, 1);
            let selected = self.selected.get() == Some(index);
            let style = super::row_style(&theme, well, selected, hovered == Some(display));
            buf.set_style(row, style);
            super::row_marker(&theme, selected && well.focused).render(row, buf);
            for ((cell, column), rect) in rows[index].iter().zip(&self.columns).zip(columns.iter())
            {
                let rect = Rect::new(rect.x, row.y, rect.width, 1);
                cell.clone().alignment(column.alignment).render(rect, buf);
            }
        }
        if scrollbar {
            super::scrollbar(
                buf,
                body,
                &theme,
                len - body_height + 1,
                offset,
                body_height,
            );
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Some(lines) = super::wheel(event) {
            let max = self.order.borrow().len().saturating_sub(self.height.get());
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
            if position.y == area.y {
                if self.sortable.get()
                    && let Some(column) = self
                        .columns_at
                        .borrow()
                        .iter()
                        .position(|r| (r.x..r.right()).contains(&position.x))
                {
                    self.header_click(column);
                }
                return Handled::Yes;
            }
            let display = self.offset.get() + usize::from(position.y - area.y - 1);
            let Some(&index) = self.order.borrow().get(display) else {
                return Handled::Yes;
            };
            let now = Instant::now();
            let double = self
                .last_click
                .get()
                .is_some_and(|(i, at)| i == index && now - at <= Duration::from_millis(500));
            self.select_at(display);
            if double {
                self.events.activated().emit(index);
                self.last_click.set(None);
            } else {
                self.last_click.set(Some((index, now)));
            }
            return Handled::Yes;
        }
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
            KeyCode::Home => self.select_at(0),
            KeyCode::End => self.select_at(usize::MAX),
            KeyCode::Enter => match self.selected.get() {
                Some(index) => self.events.activated().emit(index),
                None => return Handled::No,
            },
            _ => return Handled::No,
        }
        Handled::Yes
    }

    fn focusable(&self) -> bool {
        true
    }
}
