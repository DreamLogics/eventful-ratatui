//! [`Label`].
use super::handle;
use crate::{Component, invalidate, theme};
use ratatui::{
    style::Style,
    text::Text,
    widgets::{Paragraph, Widget, Wrap},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Alignment, Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

handle! {
    /// Static text: captions, field labels, messages. Wraps at word
    /// boundaries when [`wrap`](Self::wrap)ped.
    Label(State)
}

#[doc(hidden)]
pub struct State {
    text: RefCell<Text<'static>>,
    style: Cell<Option<Style>>,
    muted: Cell<bool>,
    alignment: Cell<Alignment>,
    wrap: Cell<bool>,
}

impl Label {
    /// A label showing `text`, which may hold styled lines.
    pub fn new(text: impl Into<Text<'static>>) -> Self {
        Self(Rc::new(State {
            text: RefCell::new(text.into()),
            style: Cell::new(None),
            muted: Cell::new(false),
            alignment: Cell::new(Alignment::Left),
            wrap: Cell::new(false),
        }))
    }
    /// Draw in the theme's muted style.
    pub fn muted(self) -> Self {
        self.0.muted.set(true);
        self
    }
    /// Draw in `style` instead of the theme's.
    pub fn style(self, style: Style) -> Self {
        self.0.style.set(Some(style));
        self
    }
    /// Align the lines.
    pub fn alignment(self, alignment: Alignment) -> Self {
        self.0.alignment.set(alignment);
        self
    }
    /// Wrap long lines at word boundaries.
    pub fn wrap(self) -> Self {
        self.0.wrap.set(true);
        self
    }
    /// Change the text.
    pub fn set_text(&self, text: impl Into<Text<'static>>) {
        *self.0.text.borrow_mut() = text.into();
        invalidate::invalidate();
    }
}

impl State {
    fn paragraph(&self) -> Paragraph<'static> {
        let theme = theme::current();
        let style = self.style.get().unwrap_or(if self.muted.get() {
            theme.muted()
        } else {
            theme.text()
        });
        let mut paragraph = Paragraph::new(self.text.borrow().clone())
            .style(style)
            .alignment(self.alignment.get());
        if self.wrap.get() {
            paragraph = paragraph.wrap(Wrap { trim: true });
        }
        paragraph
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.paragraph().render(area, buf);
    }
    fn preferred_size(&self, available: Size) -> Size {
        let text = self.text.borrow();
        let width = (text.width() as u16).min(available.width);
        let height = if self.wrap.get() && width > 0 {
            // Greedy word wrap estimate: enough for most labels.
            text.lines
                .iter()
                .map(|line| (line.width() as u16).div_ceil(width).max(1))
                .sum()
        } else {
            text.height() as u16
        };
        Size::new(width, height)
    }
}
