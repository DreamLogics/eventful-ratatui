//! [`ScrollView`].
use super::handle;
use crate::{Component, Handled, focus, invalidate, tree};
use crossterm::event::{Event, KeyCode};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

handle! {
    /// A vertically scrolling viewport around content taller than the
    /// screen, like a long form. The content's height comes from its
    /// [`preferred_size`](crate::Component::preferred_size).
    ///
    /// The wheel scrolls; PageUp/PageDown and Ctrl+Home/End scroll when the
    /// content does not handle them. Moving the focus scrolls the focused
    /// component into view. Mouse input, focus and the cursor work inside.
    ScrollView(State)
}

#[doc(hidden)]
pub struct State {
    child: RefCell<Option<Rc<dyn Component>>>,
    offset: Cell<u16>,
    /// Viewport and content height at the last render.
    sizes: Cell<(u16, u16)>,
    /// The focused component last scrolled into view, so it is revealed once
    /// per focus change and the user can scroll away from it.
    revealed: Cell<*const ()>,
}

impl ScrollView {
    /// Scroll `content`.
    pub fn new(content: impl Component + 'static) -> Self {
        Self(Rc::new(State {
            child: RefCell::new(Some(Rc::new(content))),
            offset: Cell::new(0),
            sizes: Cell::new((0, 0)),
            revealed: Cell::new(std::ptr::null()),
        }))
    }
    /// The first visible row of the content.
    pub fn offset(&self) -> u16 {
        self.0.offset.get()
    }
    /// Scroll so `row` of the content is at the top.
    pub fn scroll_to(&self, row: u16) {
        self.0.offset.set(row);
        invalidate::invalidate();
    }
}

impl State {
    fn scroll_by(&self, rows: i32) {
        let (viewport, content) = self.sizes.get();
        let max = content.saturating_sub(viewport);
        let offset = (i32::from(self.offset.get()) + rows).clamp(0, i32::from(max));
        self.offset.set(offset as u16);
        invalidate::invalidate();
    }

    /// The content area of the focused component, if it is inside this view
    /// and was not revealed yet.
    fn focused_content_area(&self) -> Option<Rect> {
        let focused = focus::focused()?;
        if std::ptr::eq(self.revealed.get(), focused.addr()) {
            return None;
        }
        let me = crate::component::addr_of(self);
        tree::with_last(|nodes| {
            let index = tree::find(nodes, focused.addr())?;
            let mut next = nodes[index].parent;
            while let Some(i) = next {
                if nodes[i].target.is(me) {
                    self.revealed.set(focused.addr());
                    return Some(nodes[index].content_area);
                }
                next = nodes[i].parent;
            }
            // Focus is elsewhere: reveal it again when it comes back.
            self.revealed.set(std::ptr::null());
            None
        })
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let Some(child) = self.child.borrow().clone() else {
            return;
        };
        let full = child.preferred_size(area.as_size());
        if full.height <= area.height {
            self.sizes.set((area.height, full.height));
            self.offset.set(0);
            child.render(area, buf);
            return;
        }
        let width = area.width.saturating_sub(1);
        let height = child
            .preferred_size(Size::new(width, u16::MAX - area.y))
            .height
            .min(u16::MAX - area.y);
        self.sizes.set((area.height, height));
        let max = height.saturating_sub(area.height);
        let mut offset = self.offset.get().min(max);
        // Content coordinates: the content starts at the viewport's top.
        if let Some(focused) = self.focused_content_area() {
            let top = focused.y.saturating_sub(area.y);
            let bottom = focused.bottom().saturating_sub(area.y);
            if top < offset {
                offset = top;
            } else if bottom > offset + area.height {
                offset = (bottom - area.height).min(top);
            }
        }
        self.offset.set(offset);

        let content = Rect::new(area.x, area.y, width, height);
        let mut canvas = Buffer::empty(content);
        // Start from the colors underneath, so empty cells don't punch holes.
        let under = &buf[(area.x, area.y)];
        let mut base = ratatui::style::Style::new().bg(under.bg).fg(under.fg);
        base.add_modifier = ratatui::style::Modifier::empty();
        canvas.set_style(content, base);
        {
            let viewport = Rect::new(area.x, area.y, width, area.height);
            let _transform = tree::push_viewport(0, -i32::from(offset), viewport);
            child.render(content, &mut canvas);
        }
        for row in 0..area.height {
            for column in 0..width {
                let (x, y) = (area.x + column, area.y + row);
                buf[(x, y)] = canvas[(x, y + offset)].clone();
            }
        }
        let theme = crate::theme::current();
        super::scrollbar(
            buf,
            area,
            &theme,
            usize::from(max) + 1,
            usize::from(offset),
            usize::from(area.height),
        );
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Some(lines) = super::wheel(event) {
            self.scroll_by(lines * 3);
            return Handled::Yes;
        }
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        let page = i32::from(self.sizes.get().0.max(1));
        let ctrl = key.modifiers == crossterm::event::KeyModifiers::CONTROL;
        let plain = key.modifiers.is_empty();
        match key.code {
            KeyCode::PageDown if plain => self.scroll_by(page),
            KeyCode::PageUp if plain => self.scroll_by(-page),
            KeyCode::Home if ctrl => self.scroll_by(i32::MIN / 2),
            KeyCode::End if ctrl => self.scroll_by(i32::MAX / 2),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    fn preferred_size(&self, available: Size) -> Size {
        let content = self
            .child
            .borrow()
            .clone()
            .map_or(Size::ZERO, |c| c.preferred_size(available));
        Size::new(
            content.width.min(available.width),
            content.height.min(available.height),
        )
    }
}
