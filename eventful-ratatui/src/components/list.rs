//! [`List`].
use super::{SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    time::{Duration, Instant},
};

/// Time within which typed characters extend the type-ahead search, and two
/// clicks make a double click.
const QUICK: Duration = Duration::from_millis(500);

handle! {
    /// A scrollable, selectable list of lines.
    ///
    /// Keys:
    /// - Up/Down move the selection, PageUp/PageDown move by a page, and
    ///   Home/End go to the ends.
    /// - Typing jumps to the next item starting with the typed text, or use
    ///   [`vim_keys`](Self::vim_keys) for j/k/g/G instead.
    /// - Enter activates the selected item.
    /// - In [`multi_select`](Self::multi_select) mode, Space marks items.
    ///
    /// A click selects, a double click activates, and the wheel scrolls.
    /// Emits [`selected`](super::SelectEventsSignalsExt::selected) and
    /// [`activated`](super::SelectEventsSignalsExt::activated).
    ///
    /// The selected item is drawn highlighted with a bar at its left edge, and
    /// a scrollbar appears when the items do not fit. Moving the selection
    /// scrolls it into view; the wheel scrolls the view only.
    ///
    /// ```
    /// use eventful_ratatui::{components::List, testing::Harness};
    ///
    /// let fruit = List::new(["apple", "banana", "blueberry", "cherry"]);
    /// let mut ui = Harness::new(fruit.clone(), 12, 4);
    /// assert_eq!(fruit.selection(), None);
    ///
    /// // The first Down selects the first item.
    /// ui.key("tab").key("down");
    /// assert_eq!(fruit.selection(), Some(0));
    /// assert!(ui.lines()[0].starts_with("▌apple"));
    /// // Type-ahead: a repeated letter cycles through its items.
    /// ui.type_text("b").type_text("b");
    /// assert_eq!(fruit.selection(), Some(2));
    /// ui.click(1, 3);
    /// assert_eq!(fruit.selection(), Some(3));
    /// ```
    List(State), events = (SelectEventsEventSet, SelectEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    items: RefCell<Vec<Line<'static>>>,
    selected: Cell<Option<usize>>,
    /// First visible item.
    offset: Cell<usize>,
    /// Scroll the selection into view at the next render.
    reveal: Cell<bool>,
    /// Rows shown at the last render.
    height: Cell<usize>,
    marked: RefCell<BTreeSet<usize>>,
    multi: Cell<bool>,
    vim: Cell<bool>,
    enabled: Cell<bool>,
    /// Type-ahead text and when it was last extended.
    search: RefCell<(String, Option<Instant>)>,
    /// The last click: item and time, for double clicks.
    last_click: Cell<Option<(usize, Instant)>>,
    events: SelectEventsEmissions,
}

impl List {
    /// A list of `items`, nothing selected. Items are lines, so they can be
    /// styled: `List::new([Line::from("ok").green()])`.
    pub fn new<I: IntoIterator<Item = L>, L: Into<Line<'static>>>(items: I) -> Self {
        Self(Rc::new(State {
            items: RefCell::new(items.into_iter().map(Into::into).collect()),
            selected: Cell::new(None),
            offset: Cell::new(0),
            reveal: Cell::new(false),
            height: Cell::new(0),
            marked: RefCell::default(),
            multi: Cell::new(false),
            vim: Cell::new(false),
            enabled: Cell::new(true),
            search: RefCell::default(),
            last_click: Cell::new(None),
            events: SelectEventsEmissions::default(),
        }))
    }
    /// Let Space mark and unmark the selected item, so several can be picked;
    /// see [`marked`](Self::marked). Each item gets a checkbox showing its
    /// mark. Marking does not emit an event; read the marks when needed.
    pub fn multi_select(self) -> Self {
        self.0.multi.set(true);
        self
    }
    /// Use j/k for down and up and g/G for the first and last item, instead
    /// of type-ahead search. The arrow keys keep working.
    pub fn vim_keys(self) -> Self {
        self.0.vim.set(true);
        self
    }
    /// Start with `index` selected; ignored if out of range. See
    /// [`set_selection`](Self::set_selection).
    pub fn with_selection(self, index: usize) -> Self {
        self.set_selection(Some(index));
        self
    }
    /// Replace the items. The selection is clamped and marks are cleared.
    pub fn set_items<I: IntoIterator<Item = L>, L: Into<Line<'static>>>(&self, items: I) {
        *self.0.items.borrow_mut() = items.into_iter().map(Into::into).collect();
        let len = self.len();
        self.0.selected.set(
            self.0
                .selected
                .get()
                .map(|i| i.min(len.saturating_sub(1)))
                .filter(|_| len > 0),
        );
        self.0.marked.borrow_mut().clear();
        invalidate::invalidate();
    }
    /// Append an item. The selection and scroll position stay where they are.
    pub fn push(&self, item: impl Into<Line<'static>>) {
        self.0.items.borrow_mut().push(item.into());
        invalidate::invalidate();
    }
    /// How many items there are.
    pub fn len(&self) -> usize {
        self.0.items.borrow().len()
    }
    /// Whether there are no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The item at `index`, or `None` if out of range.
    pub fn item(&self, index: usize) -> Option<Line<'static>> {
        self.0.items.borrow().get(index).cloned()
    }
    /// The selected index, if an item is selected. A new list has none until
    /// the user moves or clicks, or [`set_selection`](Self::set_selection).
    pub fn selection(&self) -> Option<usize> {
        self.0.selected.get()
    }
    /// Select `index` and scroll it into view at the next render, without
    /// emitting `selected`. `None`, or an index out of range, clears the
    /// selection.
    pub fn set_selection(&self, index: Option<usize>) {
        let len = self.len();
        self.0.selected.set(index.filter(|i| *i < len));
        self.0.reveal.set(true);
        invalidate::invalidate();
    }
    /// The marked indices, in ascending order. Always empty unless
    /// [`multi_select`](Self::multi_select) is on.
    pub fn marked(&self) -> Vec<usize> {
        self.0.marked.borrow().iter().copied().collect()
    }
    /// Whether it can be used and focused.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get()
    }
    /// Enable or disable it. A disabled list is drawn in the disabled style,
    /// skipped by Tab and ignores keys and clicks.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        invalidate::invalidate();
    }
}

impl State {
    fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Select `index` (clamped), scroll to it, and report a change.
    fn select(&self, index: usize) {
        let len = self.len();
        if len == 0 {
            return;
        }
        let index = index.min(len - 1);
        self.reveal.set(true);
        invalidate::invalidate();
        if self.selected.get() != Some(index) {
            self.selected.set(Some(index));
            self.events.selected().emit(index);
        }
    }

    /// Move the selection by `delta` items.
    fn step(&self, delta: isize) {
        let next = match self.selected.get() {
            Some(i) => i.saturating_add_signed(delta),
            None if delta < 0 => self.len().saturating_sub(1),
            None => 0,
        };
        self.select(next);
    }

    fn activate(&self) {
        if let Some(index) = self.selected.get() {
            self.events.activated().emit(index);
        }
    }

    /// Jump to the next item starting with the typed text.
    fn type_ahead(&self, c: char) {
        let mut search = self.search.borrow_mut();
        let now = Instant::now();
        if search.1.is_none_or(|at| now - at > QUICK) {
            search.0.clear();
        }
        search.0.extend(c.to_lowercase());
        search.1 = Some(now);
        // The same letter typed repeatedly cycles through the items starting
        // with it; a longer prefix may match the current item.
        let mut chars = search.0.chars();
        let first = chars.next();
        let cycle = chars.all(|c| Some(c) == first);
        let needle = if cycle {
            first.map(String::from).unwrap_or_default()
        } else {
            search.0.clone()
        };
        drop(search);
        let items = self.items.borrow();
        let start = self
            .selected
            .get()
            .map_or(0, |i| if cycle { i + 1 } else { i });
        let found = (0..items.len())
            .map(|k| (start + k) % items.len())
            .find(|&i| line_text(&items[i]).to_lowercase().starts_with(&needle));
        drop(items);
        if let Some(index) = found {
            self.select(index);
        }
    }

    fn key(&self, event: &Event) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        let page = self.height.get().max(1) as isize;
        let plain = (key.modifiers - KeyModifiers::SHIFT).is_empty();
        if !plain {
            return Handled::No;
        }
        match key.code {
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::PageUp => self.step(-page),
            KeyCode::PageDown => self.step(page),
            KeyCode::Home => self.select(0),
            KeyCode::End => self.select(usize::MAX),
            KeyCode::Enter => self.activate(),
            KeyCode::Char(' ') if self.multi.get() => {
                if let Some(index) = self.selected.get() {
                    let mut marked = self.marked.borrow_mut();
                    if !marked.remove(&index) {
                        marked.insert(index);
                    }
                    invalidate::invalidate();
                }
            }
            KeyCode::Char('k') if self.vim.get() => self.step(-1),
            KeyCode::Char('j') if self.vim.get() => self.step(1),
            KeyCode::Char('g') if self.vim.get() => self.select(0),
            KeyCode::Char('G') if self.vim.get() => self.select(usize::MAX),
            KeyCode::Char(c) if !self.vim.get() && !c.is_control() => self.type_ahead(c),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    fn mouse(&self, event: &Event) -> Handled {
        if let Some(lines) = super::wheel(event) {
            let max = self.len().saturating_sub(self.height.get());
            let offset = self.offset.get().saturating_add_signed(lines as isize * 3);
            self.offset.set(offset.min(max));
            invalidate::invalidate();
            return Handled::Yes;
        }
        if let Some(position) = super::left_press(event) {
            let area = mouse::area_of(self).unwrap_or_default();
            let row = usize::from(position.y.saturating_sub(area.y));
            let index = self.offset.get() + row;
            if index < self.len() {
                let now = Instant::now();
                let double = self
                    .last_click
                    .get()
                    .is_some_and(|(i, at)| i == index && now - at <= QUICK);
                self.select(index);
                if double {
                    self.activate();
                    self.last_click.set(None);
                } else {
                    self.last_click.set(Some((index, now)));
                }
            }
            return Handled::Yes;
        }
        Handled::No
    }
}

/// The plain text of `line`.
fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let height = usize::from(area.height);
        self.height.set(height);
        let items = self.items.borrow();
        let len = items.len();
        let max_offset = len.saturating_sub(height);
        let mut offset = self.offset.get().min(max_offset);
        if self.reveal.take()
            && let Some(selected) = self.selected.get()
        {
            if selected < offset {
                offset = selected;
            } else if selected >= offset + height {
                offset = selected + 1 - height;
            }
        }
        self.offset.set(offset);

        let scrollbar = len > height;
        let width = area.width.saturating_sub(u16::from(scrollbar));
        // Large wells lift on focus only; rows show the hover.
        let well = Interaction::of(self)
            .hovered(false)
            .disabled(!self.enabled.get());
        super::fill(buf, area, theme.well(well));
        let hovered = super::hovered_row(self, area).map(|row| offset + usize::from(row));
        let marked = self.marked.borrow();
        for (row, index) in (offset..len.min(offset + height)).enumerate() {
            let rect = Rect::new(area.x, area.y + row as u16, width, 1);
            let selected = self.selected.get() == Some(index);
            let style = super::row_style(&theme, well, selected, hovered == Some(index));
            buf.set_style(rect, style);
            let mut line = Line::default().style(style);
            line.push_span(super::row_marker(&theme, selected && well.focused));
            if self.multi.get() {
                let on = marked.contains(&index);
                let mark = if on { theme.accent() } else { theme.muted() };
                line.push_span(Span::styled(theme.symbols.checkbox[usize::from(on)], mark));
                line.push_span(" ");
            }
            for span in &items[index].spans {
                line.push_span(span.clone());
            }
            line.render(rect, buf);
        }
        if scrollbar {
            super::scrollbar(buf, area, &theme, max_offset + 1, offset, height);
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        match event {
            Event::Mouse(_) => self.mouse(event),
            _ => self.key(event),
        }
    }

    fn focusable(&self) -> bool {
        self.enabled.get()
    }

    fn preferred_size(&self, available: Size) -> Size {
        let items = self.items.borrow();
        let extra = super::width(theme::current().symbols.highlight) + 1;
        let width = items.iter().map(|l| l.width() as u16).max().unwrap_or(0) + extra;
        Size::new(
            width.min(available.width),
            (items.len() as u16).min(available.height),
        )
    }
}
