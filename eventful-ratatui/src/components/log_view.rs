//! [`LogView`].
use super::handle;
use crate::{Component, Handled, invalidate, theme};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    text::Line,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

handle! {
    /// An append-only view of lines, like a log or a chat history. It
    /// follows new lines while scrolled to the bottom and holds still while
    /// scrolled up. Old lines are dropped beyond [`max_lines`](Self::max_lines).
    ///
    /// Up/Down, PageUp/PageDown and Home/End scroll while it has focus; the
    /// wheel scrolls. End returns to following.
    LogView(State)
}

#[doc(hidden)]
pub struct State {
    lines: RefCell<VecDeque<Line<'static>>>,
    max: Cell<usize>,
    /// Rows scrolled up from the bottom; 0 follows new lines.
    back: Cell<usize>,
    height: Cell<usize>,
    /// Text width at the last render, for wrapping.
    width: Cell<usize>,
    wrap: Cell<bool>,
}

impl LogView {
    /// An empty log keeping up to 10 000 lines.
    pub fn new() -> Self {
        Self(Rc::new(State {
            lines: RefCell::default(),
            max: Cell::new(10_000),
            back: Cell::new(0),
            height: Cell::new(0),
            width: Cell::new(0),
            wrap: Cell::new(false),
        }))
    }
    /// Wrap long lines at word boundaries instead of cutting them off.
    pub fn wrap(self) -> Self {
        self.0.wrap.set(true);
        self
    }
    /// Keep at most `max` lines.
    pub fn max_lines(self, max: usize) -> Self {
        self.0.max.set(max.max(1));
        self
    }
    /// Append a line.
    pub fn push(&self, line: impl Into<Line<'static>>) {
        let mut lines = self.0.lines.borrow_mut();
        lines.push_back(line.into());
        let dropped = lines.len().saturating_sub(self.0.max.get());
        lines.drain(..dropped);
        if self.0.back.get() > 0 {
            // Hold the view still: the new line's rows push the bottom down.
            let added = self.0.rows_of(lines.back().expect("just pushed"));
            self.0.back.set(self.0.back.get() + added);
        }
        invalidate::invalidate();
    }
    /// Remove every line.
    pub fn clear(&self) {
        self.0.lines.borrow_mut().clear();
        self.0.back.set(0);
        invalidate::invalidate();
    }
    /// How many lines it holds.
    pub fn len(&self) -> usize {
        self.0.lines.borrow().len()
    }
    /// Whether it holds no lines.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Lines scrolled up from the bottom; 0 while following.
    pub fn scrolled_back(&self) -> usize {
        self.0.back.get()
    }
    /// Scroll to the bottom and follow new lines.
    pub fn follow(&self) {
        self.0.back.set(0);
        invalidate::invalidate();
    }
}

impl Default for LogView {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// The rows `line` takes at the last rendered width.
    fn rows_of(&self, line: &Line<'static>) -> usize {
        if self.wrap.get() {
            wrap(line, self.width.get()).len()
        } else {
            1
        }
    }

    fn total_rows(&self) -> usize {
        let lines = self.lines.borrow();
        if self.wrap.get() {
            lines.iter().map(|l| self.rows_of(l)).sum()
        } else {
            lines.len()
        }
    }

    fn scroll_back(&self, rows: isize) {
        let max = self.total_rows().saturating_sub(self.height.get());
        self.back
            .set(self.back.get().saturating_add_signed(rows).min(max));
        invalidate::invalidate();
    }
}

/// Split `line` into rows of at most `width` cells, breaking after spaces
/// where possible and inside words otherwise. Styles are kept.
pub(crate) fn wrap(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    use ratatui::{style::Style, text::Span};
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    if width == 0 {
        return vec![line.clone()];
    }
    let cells: Vec<(String, Style, usize)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let style = line.style.patch(span.style);
            span.content
                .graphemes(true)
                .map(move |g| (g.to_owned(), style, g.width()))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut rows = Vec::new();
    let mut start = 0;
    while start < cells.len() {
        let mut used = 0;
        let mut end = start;
        let mut last_space = None;
        while end < cells.len() && used + cells[end].2 <= width {
            if cells[end].0.chars().all(char::is_whitespace) {
                last_space = Some(end);
            }
            used += cells[end].2;
            end += 1;
        }
        let is_space = |i: usize| cells[i].0.chars().all(char::is_whitespace);
        // Where the next row starts.
        let mut next = end;
        if end < cells.len() && is_space(end) {
            // The row is full right before a space: break there, drop it.
            next = end + 1;
        } else if end < cells.len()
            && let Some(space) = last_space.filter(|&s| s > start)
        {
            end = space + 1;
            next = end;
        }
        if end == start {
            end += 1; // A grapheme wider than the row.
            next = end;
        }
        let spans: Vec<Span<'static>> = cells[start..end]
            .iter()
            .map(|(g, style, _)| Span::styled(g.clone(), *style))
            .collect();
        rows.push(Line::from(spans));
        start = next;
    }
    if rows.is_empty() {
        rows.push(Line::default());
    }
    rows
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let height = usize::from(area.height);
        self.height.set(height);
        // Assume a scrollbar while measuring; it costs one column at most.
        self.width.set(usize::from(area.width.saturating_sub(1)));
        let len = self.total_rows();
        let back = self.back.get().min(len.saturating_sub(height));
        self.back.set(back);
        let scrollbar = len > height;
        let width = area.width.saturating_sub(u16::from(scrollbar));
        // Collect rows from the bottom up: skip `back`, take `height`.
        let lines = self.lines.borrow();
        let mut rows: Vec<Line<'static>> = Vec::with_capacity(height);
        let mut skip = back;
        for line in lines.iter().rev() {
            let wrapped = if self.wrap.get() {
                wrap(line, self.width.get())
            } else {
                vec![line.clone()]
            };
            for row in wrapped.into_iter().rev() {
                if skip > 0 {
                    skip -= 1;
                } else if rows.len() < height {
                    rows.push(row);
                }
            }
            if rows.len() >= height {
                break;
            }
        }
        rows.reverse();
        for (y, row) in rows.iter().enumerate() {
            row.render(Rect::new(area.x, area.y + y as u16, width, 1), buf);
        }
        if scrollbar {
            let theme = theme::current();
            let max = len - height;
            let mut state = ScrollbarState::new(max + 1)
                .position(max - back)
                .viewport_content_length(height);
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some(theme.symbols.scrollbar[0]))
                .thumb_symbol(theme.symbols.scrollbar[1])
                .track_style(theme.scrollbar)
                .thumb_style(theme.scrollbar_thumb)
                .render(area, buf, &mut state);
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Some(lines) = super::wheel(event) {
            self.scroll_back(-(lines as isize) * 3);
            return Handled::Yes;
        }
        let page = self.height.get().max(1) as isize;
        let target = match () {
            () if super::pressed(event, KeyCode::Up) => 1,
            () if super::pressed(event, KeyCode::Down) => -1,
            () if super::pressed(event, KeyCode::PageUp) => page,
            () if super::pressed(event, KeyCode::PageDown) => -page,
            () if super::pressed(event, KeyCode::Home) => isize::MAX / 2,
            () if super::pressed(event, KeyCode::End) => isize::MIN / 2,
            () => return Handled::No,
        };
        self.scroll_back(target);
        Handled::Yes
    }

    fn focusable(&self) -> bool {
        true
    }

    fn preferred_size(&self, available: Size) -> Size {
        available
    }
}
