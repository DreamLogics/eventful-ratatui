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
    /// Lines scrolled up from the bottom; 0 follows new lines.
    back: Cell<usize>,
    height: Cell<usize>,
}

impl LogView {
    /// An empty log keeping up to 10 000 lines.
    pub fn new() -> Self {
        Self(Rc::new(State {
            lines: RefCell::default(),
            max: Cell::new(10_000),
            back: Cell::new(0),
            height: Cell::new(0),
        }))
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
            // Hold the view still.
            let back = self.0.back.get() + 1 - dropped.min(1);
            self.0
                .back
                .set(back.min(lines.len().saturating_sub(self.0.height.get())));
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
    fn scroll_back(&self, rows: isize) {
        let max = self.lines.borrow().len().saturating_sub(self.height.get());
        self.back
            .set(self.back.get().saturating_add_signed(rows).min(max));
        invalidate::invalidate();
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let height = usize::from(area.height);
        self.height.set(height);
        let lines = self.lines.borrow();
        let len = lines.len();
        let back = self.back.get().min(len.saturating_sub(height));
        let end = len - back;
        let start = end.saturating_sub(height);
        let scrollbar = len > height;
        let width = area.width.saturating_sub(u16::from(scrollbar));
        for (row, line) in lines.range(start..end).enumerate() {
            line.render(Rect::new(area.x, area.y + row as u16, width, 1), buf);
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
