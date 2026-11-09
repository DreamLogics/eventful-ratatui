//! [`Stack`].
use super::handle;
use crate::{Component, invalidate};
use ratatui_core::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

handle! {
    /// Children in a row or column, sized by ratatui [`Constraint`]s, with
    /// optional spacing. The building block for forms and screens.
    ///
    /// ```
    /// use eventful_ratatui::prelude::*;
    ///
    /// let form = Stack::vertical()
    ///     .child(Constraint::Length(1), TextInput::new().placeholder("Name"))
    ///     .child(Constraint::Length(1), Checkbox::new("Subscribe"))
    ///     .child(Constraint::Length(1), Button::new("Save").primary());
    /// ```
    ///
    /// [`Constraint::Length`] sized children report their length in
    /// [`preferred_size`](crate::Component::preferred_size), so a stack
    /// inside a [`ScrollView`](super::ScrollView) scrolls correctly.
    Stack(State)
}

#[doc(hidden)]
pub struct State {
    direction: Direction,
    spacing: Cell<u16>,
    children: RefCell<Vec<(Constraint, Rc<dyn Component>)>>,
}

impl Stack {
    /// Children top to bottom.
    pub fn vertical() -> Self {
        Self::new(Direction::Vertical)
    }
    /// Children left to right.
    pub fn horizontal() -> Self {
        Self::new(Direction::Horizontal)
    }
    fn new(direction: Direction) -> Self {
        Self(Rc::new(State {
            direction,
            spacing: Cell::new(0),
            children: RefCell::default(),
        }))
    }
    /// Add `child`, sized by `constraint`.
    pub fn child(self, constraint: Constraint, child: impl Component + 'static) -> Self {
        self.push(constraint, child);
        self
    }
    /// Add `child` at its preferred size along the stack's direction.
    pub fn fit(self, child: impl Component + 'static) -> Self {
        let child: Rc<dyn Component> = Rc::new(child);
        let size = child.preferred_size(Size::new(u16::MAX, u16::MAX));
        let length = match self.0.direction {
            Direction::Vertical => size.height,
            Direction::Horizontal => size.width,
        };
        self.0
            .children
            .borrow_mut()
            .push((Constraint::Length(length), child));
        self
    }
    /// Cells between children.
    pub fn spacing(self, spacing: u16) -> Self {
        self.0.spacing.set(spacing);
        self
    }
    /// Add `child` to a stack in use.
    pub fn push(&self, constraint: Constraint, child: impl Component + 'static) {
        self.0
            .children
            .borrow_mut()
            .push((constraint, Rc::new(child)));
        invalidate::invalidate();
    }
    /// Remove every child.
    pub fn clear(&self) {
        self.0.children.borrow_mut().clear();
        invalidate::invalidate();
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let children = self.children.borrow().clone();
        let areas = Layout::default()
            .direction(self.direction)
            .spacing(self.spacing.get())
            .constraints(children.iter().map(|(c, _)| *c))
            .split(area);
        for ((_, child), area) in children.iter().zip(areas.iter()) {
            child.render(*area, buf);
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        let children = self.children.borrow();
        let gaps = self.spacing.get() * children.len().saturating_sub(1) as u16;
        let mut along: u16 = gaps;
        let mut across: u16 = 0;
        for (constraint, child) in children.iter() {
            let size = child.preferred_size(available);
            let (main, cross) = match self.direction {
                Direction::Vertical => (size.height, size.width),
                Direction::Horizontal => (size.width, size.height),
            };
            let main = match *constraint {
                Constraint::Length(n) | Constraint::Min(n) => n,
                Constraint::Max(n) => main.min(n),
                _ => main,
            };
            along = along.saturating_add(main);
            across = across.max(cross);
        }
        match self.direction {
            Direction::Vertical => Size::new(across.min(available.width), along),
            Direction::Horizontal => Size::new(along, across.min(available.height)),
        }
    }
}
