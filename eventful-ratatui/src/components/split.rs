//! [`Split`].
use super::handle;
use crate::{Component, Handled, focus, invalidate, mouse, theme};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{text::Line, widgets::Widget};
use ratatui_core::{
    buffer::Buffer,
    layout::{Direction, Rect, Size},
};
use std::{cell::Cell, rc::Rc};

handle! {
    /// Two panes side by side (or stacked) with a divider the user can drag,
    /// like a sidebar next to content. Alt+arrows also move the divider while
    /// focus is inside.
    Split(State)
}

#[doc(hidden)]
pub struct State {
    direction: Direction,
    first: Rc<dyn Component>,
    second: Rc<dyn Component>,
    /// Share of the first pane, 0..=1.
    ratio: Cell<f64>,
    /// Smallest pane size in cells.
    min: Cell<u16>,
    /// Size along the split at the last render.
    length: Cell<u16>,
    dragging: Cell<bool>,
}

impl Split {
    /// `left` and `right` side by side, half each.
    pub fn horizontal(left: impl Component + 'static, right: impl Component + 'static) -> Self {
        Self::new(Direction::Horizontal, Rc::new(left), Rc::new(right))
    }
    /// `top` above `bottom`, half each.
    pub fn vertical(top: impl Component + 'static, bottom: impl Component + 'static) -> Self {
        Self::new(Direction::Vertical, Rc::new(top), Rc::new(bottom))
    }
    fn new(direction: Direction, first: Rc<dyn Component>, second: Rc<dyn Component>) -> Self {
        Self(Rc::new(State {
            direction,
            first,
            second,
            ratio: Cell::new(0.5),
            min: Cell::new(3),
            length: Cell::new(0),
            dragging: Cell::new(false),
        }))
    }
    /// Give the first pane `ratio` (0..=1) of the space.
    pub fn ratio(self, ratio: f64) -> Self {
        self.set_ratio(ratio);
        self
    }
    /// The first pane's share.
    pub fn get_ratio(&self) -> f64 {
        self.0.ratio.get()
    }
    /// Move the divider.
    pub fn set_ratio(&self, ratio: f64) {
        self.0.ratio.set(ratio.clamp(0.0, 1.0));
        invalidate::invalidate();
    }
    /// Keep each pane at least `cells` wide (or tall).
    pub fn min_size(self, cells: u16) -> Self {
        self.0.min.set(cells);
        self
    }
}

impl State {
    fn length_of(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        }
    }
    /// Cells of the first pane, with one cell of divider after it.
    fn first_len(&self, length: u16) -> u16 {
        let usable = length.saturating_sub(1);
        let min = self.min.get().min(usable / 2);
        ((f64::from(usable) * self.ratio.get()).round() as u16).clamp(min, usable - min)
    }
    fn areas(&self, area: Rect) -> (Rect, Rect, Rect) {
        let first = self.first_len(self.length_of(area));
        match self.direction {
            Direction::Horizontal => (
                Rect::new(area.x, area.y, first, area.height),
                Rect::new(area.x + first, area.y, 1.min(area.width), area.height),
                Rect::new(
                    area.x + first + 1,
                    area.y,
                    area.width.saturating_sub(first + 1),
                    area.height,
                ),
            ),
            Direction::Vertical => (
                Rect::new(area.x, area.y, area.width, first),
                Rect::new(area.x, area.y + first, area.width, 1.min(area.height)),
                Rect::new(
                    area.x,
                    area.y + first + 1,
                    area.width,
                    area.height.saturating_sub(first + 1),
                ),
            ),
        }
    }
    fn move_by(&self, cells: i32) {
        let usable = f64::from(self.length.get().saturating_sub(1).max(1));
        self.ratio
            .set((self.ratio.get() + f64::from(cells) / usable).clamp(0.0, 1.0));
        invalidate::invalidate();
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        self.length.set(self.length_of(area));
        let (first, divider, second) = self.areas(area);
        self.first.render(first, buf);
        self.second.render(second, buf);
        let style = if self.dragging.get() {
            theme.border_focused
        } else {
            theme.border
        };
        match self.direction {
            Direction::Horizontal => {
                for y in divider.top()..divider.bottom() {
                    Line::styled("│", style).render(Rect::new(divider.x, y, 1, 1), buf);
                }
            }
            Direction::Vertical => {
                Line::styled("─".repeat(usize::from(divider.width)), style).render(divider, buf);
            }
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Event::Mouse(m) = event {
            let Some(area) = mouse::area_of(self) else {
                return Handled::No;
            };
            let (_, divider, _) = self.areas(area);
            let (at, start) = match self.direction {
                Direction::Horizontal => (m.column, area.x),
                Direction::Vertical => (m.row, area.y),
            };
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) if divider.contains(mouse::position(m)) => {
                    self.dragging.set(true);
                    invalidate::invalidate();
                    Handled::Yes
                }
                MouseEventKind::Drag(MouseButton::Left) if self.dragging.get() => {
                    let usable = f64::from(self.length.get().saturating_sub(1).max(1));
                    self.ratio
                        .set((f64::from(at.saturating_sub(start)) / usable).clamp(0.0, 1.0));
                    invalidate::invalidate();
                    Handled::Yes
                }
                MouseEventKind::Up(MouseButton::Left) if self.dragging.get() => {
                    self.dragging.set(false);
                    invalidate::invalidate();
                    Handled::Yes
                }
                _ => Handled::No,
            }
        } else {
            let Some(key) = super::key(event) else {
                return Handled::No;
            };
            if key.modifiers != KeyModifiers::ALT || !focus::within(self) {
                return Handled::No;
            }
            match (self.direction, key.code) {
                (Direction::Horizontal, KeyCode::Left) | (Direction::Vertical, KeyCode::Up) => {
                    self.move_by(-2)
                }
                (Direction::Horizontal, KeyCode::Right) | (Direction::Vertical, KeyCode::Down) => {
                    self.move_by(2)
                }
                _ => return Handled::No,
            }
            Handled::Yes
        }
    }

    fn preferred_size(&self, available: Size) -> Size {
        available
    }
}
