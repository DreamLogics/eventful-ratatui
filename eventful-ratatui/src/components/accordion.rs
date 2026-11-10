//! [`Accordion`].
use super::handle;
use crate::{
    Component, Handled, focus, invalidate, mouse,
    theme::{self, Fill, Interaction},
};
use crossterm::event::{Event, KeyCode};
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
    /// Collapsible sections, each a header and content.
    ///
    /// The headers take focus as one Tab stop: Up/Down move between headers,
    /// and Enter, Space or a click toggles a section. With
    /// [`exclusive`](Self::exclusive), opening one closes the others. Open
    /// sections are indented under their header and share the rows left over
    /// after all headers; the last open section takes whatever remains.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, testing::Harness};
    ///
    /// let settings = Accordion::new()
    ///     .section("General", Label::new("general settings"))
    ///     .section("Advanced", Label::new("advanced settings"))
    ///     .exclusive();
    /// let mut ui = Harness::new(settings.clone(), 24, 3);
    /// assert_eq!(ui.lines(), [" ▸ General", " ▸ Advanced", ""]);
    ///
    /// ui.key("tab").key("enter");
    /// assert_eq!(ui.lines(), [" ▾ General", "   general settings", " ▸ Advanced"]);
    ///
    /// // Exclusive: opening Advanced closes General.
    /// settings.set_open(1, true);
    /// assert!(!settings.is_open(0) && settings.is_open(1));
    /// ```
    Accordion(State)
}

#[doc(hidden)]
pub struct State {
    sections: RefCell<Vec<Section>>,
    exclusive: Cell<bool>,
    /// The header with the keyboard highlight.
    current: Cell<usize>,
    /// Header rows at the last render.
    headers: RefCell<Vec<u16>>,
}

struct Section {
    title: String,
    content: Rc<dyn Component>,
    open: bool,
}

impl Accordion {
    /// No sections yet. An empty accordion takes no focus.
    pub fn new() -> Self {
        Self(Rc::new(State {
            sections: RefCell::default(),
            exclusive: Cell::new(false),
            current: Cell::new(0),
            headers: RefCell::default(),
        }))
    }
    /// Add a closed section below the others: a header showing `title`, and
    /// `content` shown under it while open.
    pub fn section(self, title: impl Into<String>, content: impl Component + 'static) -> Self {
        self.0.sections.borrow_mut().push(Section {
            title: title.into(),
            content: Rc::new(content),
            open: false,
        });
        self
    }
    /// Let only one section be open at a time: opening a section, by the
    /// user or with [`set_open`](Self::set_open), closes the others.
    pub fn exclusive(self) -> Self {
        self.0.exclusive.set(true);
        self
    }
    /// Open or close section `index`, counted from 0 in the order added.
    /// Indices past the end are ignored.
    ///
    /// If focus is inside a section that closes, it moves to the headers.
    pub fn set_open(&self, index: usize, open: bool) {
        self.0.set_open(index, open);
    }
    /// Whether section `index` is open; `false` past the end.
    pub fn is_open(&self, index: usize) -> bool {
        self.0.sections.borrow().get(index).is_some_and(|s| s.open)
    }
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    fn set_open(&self, index: usize, open: bool) {
        let mut sections = self.sections.borrow_mut();
        if index >= sections.len() {
            return;
        }
        if open && self.exclusive.get() {
            for section in sections.iter_mut() {
                section.open = false;
            }
        }
        sections[index].open = open;
        drop(sections);
        // Focus inside a closing section has nowhere to go but the header.
        if !open && focus::within(self) && !focus::is_focused(self) {
            focus::set(self);
        }
        invalidate::invalidate();
    }
    fn toggle(&self, index: usize) {
        let open = self.sections.borrow().get(index).is_some_and(|s| s.open);
        self.current.set(index);
        self.set_open(index, !open);
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let focused = focus::is_focused(self);
        let pointer = mouse::hover_position(self);
        let sections = self.sections.borrow();
        let open: u16 = sections.iter().filter(|s| s.open).count() as u16;
        let closed_rows = sections.len() as u16;
        // Open sections share the rows left after the headers.
        let mut spare = area.height.saturating_sub(closed_rows);
        let mut open_left = open;
        let mut y = area.y;
        let mut headers = Vec::new();
        for (i, section) in sections.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            headers.push(y);
            let symbol = theme.symbols.disclosure[usize::from(section.open)];
            let header = Rect::new(area.x, y, area.width, 1);
            let state = Interaction {
                focused: focused && self.current.get() == i,
                hovered: pointer.is_some_and(|p| header.contains(p)),
                disabled: false,
            };
            let fill = theme.fill(Fill::Ghost, state);
            buf.set_style(header, fill);
            Line::from(vec![
                Span::styled(format!(" {symbol} "), theme.muted()),
                Span::styled(section.title.as_str(), fill.add_modifier(Modifier::BOLD)),
            ])
            .render(header, buf);
            y += 1;
            if section.open {
                let wanted = section
                    .content
                    .preferred_size(Size::new(area.width.saturating_sub(3), spare))
                    .height;
                let share = if open_left <= 1 {
                    spare
                } else {
                    wanted.min(spare)
                };
                let rows = share.min(area.bottom() - y);
                section.content.render(
                    Rect::new(area.x + 3, y, area.width.saturating_sub(3), rows),
                    buf,
                );
                y += rows;
                spare -= rows;
                open_left -= 1;
            }
        }
        *self.headers.borrow_mut() = headers;
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Some(position) = super::left_press(event) {
            let hit = self
                .headers
                .borrow()
                .iter()
                .position(|&row| row == position.y);
            if let Some(index) = hit {
                focus::set(self);
                self.toggle(index);
                return Handled::Yes;
            }
            return Handled::No;
        }
        if !focus::is_focused(self) {
            return Handled::No;
        }
        let len = self.sections.borrow().len();
        let current = self.current.get().min(len.saturating_sub(1));
        if super::activates(event) {
            self.toggle(current);
        } else if super::pressed(event, KeyCode::Down) && current + 1 < len {
            self.current.set(current + 1);
            invalidate::invalidate();
        } else if super::pressed(event, KeyCode::Up) && current > 0 {
            self.current.set(current - 1);
            invalidate::invalidate();
        } else {
            return Handled::No;
        }
        Handled::Yes
    }

    fn focusable(&self) -> bool {
        !self.sections.borrow().is_empty()
    }

    fn preferred_size(&self, available: Size) -> Size {
        let sections = self.sections.borrow();
        let mut height = sections.len() as u16;
        let mut width = 0;
        for section in sections.iter() {
            width = width.max(super::width(&section.title) + 3);
            if section.open {
                let size = section.content.preferred_size(Size::new(
                    available.width.saturating_sub(3),
                    available.height,
                ));
                height = height.saturating_add(size.height);
                width = width.max(size.width + 3);
            }
        }
        Size::new(width.min(available.width), height.min(available.height))
    }
}
