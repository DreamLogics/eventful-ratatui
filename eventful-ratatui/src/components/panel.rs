//! [`Panel`].
use super::handle;
use crate::{Component, focus, invalidate, theme};
use ratatui::widgets::{Block, Padding, Widget};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

handle! {
    /// A bordered, titled container around one child, like a group box or
    /// card. Its border lights up while focus is inside it.
    Panel(State)
}

#[doc(hidden)]
pub struct State {
    title: RefCell<String>,
    footer: RefCell<String>,
    child: RefCell<Option<Rc<dyn Component>>>,
    padding: Cell<Padding>,
}

impl Panel {
    /// An empty panel titled `title`; may be empty.
    pub fn new(title: impl Into<String>) -> Self {
        Self(Rc::new(State {
            title: RefCell::new(title.into()),
            footer: RefCell::default(),
            child: RefCell::default(),
            padding: Cell::new(Padding::ZERO),
        }))
    }
    /// Show `child` inside.
    pub fn child(self, child: impl Component + 'static) -> Self {
        self.set_child(child);
        self
    }
    /// Replace the child.
    pub fn set_child(&self, child: impl Component + 'static) {
        *self.0.child.borrow_mut() = Some(Rc::new(child));
        invalidate::invalidate();
    }
    /// Space between the border and the child.
    pub fn padding(self, padding: Padding) -> Self {
        self.0.padding.set(padding);
        self
    }
    /// Text in the bottom border, like a hint or a count.
    pub fn footer(self, footer: impl Into<String>) -> Self {
        self.set_footer(footer);
        self
    }
    /// Change the bottom border text.
    pub fn set_footer(&self, footer: impl Into<String>) {
        *self.0.footer.borrow_mut() = footer.into();
        invalidate::invalidate();
    }
    /// Change the title.
    pub fn set_title(&self, title: impl Into<String>) {
        *self.0.title.borrow_mut() = title.into();
        invalidate::invalidate();
    }
}

impl State {
    fn block(&self, active: bool) -> Block<'static> {
        let theme = theme::current();
        let mut block = Block::bordered()
            .border_type(theme.border_type)
            .border_style(if active {
                theme.border_focused
            } else {
                theme.border
            })
            .padding(self.padding.get());
        let title = self.title.borrow();
        if !title.is_empty() {
            block = block.title(format!(" {title} ")).title_style(theme.title);
        }
        let footer = self.footer.borrow();
        if !footer.is_empty() {
            block = block.title_bottom(ratatui::text::Line::styled(
                format!(" {footer} "),
                theme.muted,
            ));
        }
        block
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = self.block(focus::within(self));
        let inner = block.inner(area);
        block.render(area, buf);
        let child = self.child.borrow().clone();
        if let Some(child) = child {
            child.render(inner, buf);
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        let block = self.block(false);
        let outer = Rect::new(0, 0, available.width, available.height);
        let inner = block.inner(outer);
        let extra = Size::new(outer.width - inner.width, outer.height - inner.height);
        let child = self.child.borrow().clone();
        let content = child.map_or(Size::ZERO, |c| c.preferred_size(inner.as_size()));
        Size::new(
            (content.width + extra.width).max(super::width(&self.title.borrow()) + 4),
            content.height + extra.height,
        )
    }
}
