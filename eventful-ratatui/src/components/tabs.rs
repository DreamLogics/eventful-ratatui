//! [`Tabs`].
use super::{SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt, handle};
use crate::{Component, Handled, focus, invalidate, theme};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

handle! {
    /// Pages behind a row of tabs: ` General │ Network │ About `.
    ///
    /// The tab row takes focus: Left/Right switch pages. Ctrl+PageUp and
    /// Ctrl+PageDown switch from anywhere inside, and a click on a title
    /// switches too. Emits [`selected`](super::SelectEventsSignalsExt::selected).
    Tabs(State), events = (SelectEventsEventSet, SelectEventsSignalsExt)
}

/// A title and the content shown under it, if any.
type Page = (String, Option<Rc<dyn Component>>);

#[doc(hidden)]
pub struct State {
    pages: RefCell<Vec<Page>>,
    selected: Cell<usize>,
    /// Title areas at the last render, for clicks.
    titles: RefCell<Vec<Rect>>,
    events: SelectEventsEmissions,
}

impl Tabs {
    /// No pages yet.
    pub fn new() -> Self {
        Self(Rc::new(State {
            pages: RefCell::default(),
            selected: Cell::new(0),
            titles: RefCell::default(),
            events: SelectEventsEmissions::default(),
        }))
    }
    /// Add a page titled `title` showing `content`.
    pub fn page(self, title: impl Into<String>, content: impl Component + 'static) -> Self {
        self.0
            .pages
            .borrow_mut()
            .push((title.into(), Some(Rc::new(content))));
        self
    }
    /// Add a tab without content: use the tabs as a switcher and render the
    /// content yourself, by [`selection`](Self::selection).
    pub fn tab(self, title: impl Into<String>) -> Self {
        self.0.pages.borrow_mut().push((title.into(), None));
        self
    }
    /// The selected page.
    pub fn selection(&self) -> usize {
        self.0.selected.get()
    }
    /// Switch to `index` without emitting `selected`.
    pub fn set_selection(&self, index: usize) {
        self.0.show(index);
    }
    /// The page titles.
    pub fn titles(&self) -> Vec<String> {
        self.0
            .pages
            .borrow()
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Show page `index`. Focus inside the old page moves to the tab row,
    /// since the old page is no longer drawn.
    fn show(&self, index: usize) -> bool {
        let len = self.pages.borrow().len();
        if len == 0 || index >= len || index == self.selected.get() {
            return false;
        }
        if focus::within(self) && !focus::is_focused(self) {
            focus::set(self);
        }
        self.selected.set(index);
        invalidate::invalidate();
        true
    }

    fn select(&self, index: usize) {
        if self.show(index) {
            self.events.selected().emit(index);
        }
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let focused = focus::is_focused(self);
        let pages = self.pages.borrow();
        let selected = self.selected.get().min(pages.len().saturating_sub(1));
        let mut titles = Vec::new();
        let mut x = area.x;
        let mut line = Line::default();
        for (i, (title, _)) in pages.iter().enumerate() {
            if i > 0 {
                line.push_span(Span::styled("│", theme.border));
                x += 1;
            }
            let text = format!(" {title} ");
            let style = match (i == selected, focused) {
                (true, true) => theme.focused,
                (true, false) => theme.accent.add_modifier(Modifier::BOLD),
                _ => theme.muted,
            };
            let width = super::width(&text);
            titles.push(Rect::new(x, area.y, width, 1).intersection(area));
            x = x.saturating_add(width);
            line.push_span(Span::styled(text, style));
        }
        line.render(
            Rect::new(area.x, area.y, area.width, 1.min(area.height)),
            buf,
        );
        *self.titles.borrow_mut() = titles;
        let content = pages.get(selected).and_then(|(_, c)| c.clone());
        drop(pages);
        if let Some(content) = content
            && area.height > 1
        {
            let body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
            content.render(body, buf);
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        let len = self.pages.borrow().len();
        if len == 0 {
            return Handled::No;
        }
        let current = self.selected.get();
        if let Some(position) = super::left_press(event) {
            // Only presses on the tab row reach here unhandled by a page;
            // check the titles.
            let index = self
                .titles
                .borrow()
                .iter()
                .position(|r| r.contains(position));
            if let Some(index) = index {
                self.select(index);
                focus::set(self);
                return Handled::Yes;
            }
            return Handled::No;
        }
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        let next = (current + 1) % len;
        let previous = (current + len - 1) % len;
        let target = match key.code {
            KeyCode::PageDown if ctrl => next,
            KeyCode::PageUp if ctrl => previous,
            // Arrows only while the tab row itself has focus, so pages keep
            // their own arrow keys.
            KeyCode::Right if focus::is_focused(self) && key.modifiers.is_empty() => next,
            KeyCode::Left if focus::is_focused(self) && key.modifiers.is_empty() => previous,
            KeyCode::Home if focus::is_focused(self) => 0,
            KeyCode::End if focus::is_focused(self) => len - 1,
            _ => return Handled::No,
        };
        self.select(target);
        Handled::Yes
    }

    fn focusable(&self) -> bool {
        !self.pages.borrow().is_empty()
    }

    fn preferred_size(&self, available: Size) -> Size {
        let pages = self.pages.borrow();
        let content = pages
            .get(self.selected.get())
            .and_then(|(_, c)| c.clone())
            .map_or(Size::ZERO, |c| {
                c.preferred_size(Size::new(
                    available.width,
                    available.height.saturating_sub(1),
                ))
            });
        let titles: u16 = pages.iter().map(|(t, _)| super::width(t) + 3).sum();
        Size::new(
            titles.max(content.width).min(available.width),
            content.height + 1,
        )
    }
}
