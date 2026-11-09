//! [`RadioGroup`].
use super::{SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt, handle};
use crate::{Component, Handled, invalidate, mouse, theme};
use crossterm::event::{Event, KeyCode};
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
    rc::Rc,
};

handle! {
    /// One choice out of several: `(•) Small  ( ) Large`. The group takes
    /// focus as a whole; arrows move the choice, a click picks one. Emits
    /// [`selected`](super::SelectEventsSignalsExt::selected).
    RadioGroup(State), events = (SelectEventsEventSet, SelectEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    options: RefCell<Vec<String>>,
    selected: Cell<Option<usize>>,
    horizontal: Cell<bool>,
    enabled: Cell<bool>,
    events: SelectEventsEmissions,
}

/// Columns between horizontal options.
const GAP: u16 = 2;

impl RadioGroup {
    /// A vertical group of `options` with nothing selected.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(options: I) -> Self {
        Self(Rc::new(State {
            options: RefCell::new(options.into_iter().map(Into::into).collect()),
            selected: Cell::new(None),
            horizontal: Cell::new(false),
            enabled: Cell::new(true),
            events: SelectEventsEmissions::default(),
        }))
    }
    /// Lay the options out in a row.
    pub fn horizontal(self) -> Self {
        self.0.horizontal.set(true);
        self
    }
    /// Start with `index` selected.
    pub fn selected(self, index: usize) -> Self {
        self.0.selected.set(Some(index));
        self
    }
    /// The selected option, if any.
    pub fn selection(&self) -> Option<usize> {
        self.0.selected.get()
    }
    /// Select `index` without emitting `selected`.
    pub fn set_selection(&self, index: Option<usize>) {
        self.0.selected.set(index);
        invalidate::invalidate();
    }
    /// The options.
    pub fn options(&self) -> Vec<String> {
        self.0.options.borrow().clone()
    }
    /// Whether it can be changed and focused.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get()
    }
    /// Enable or disable it.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        invalidate::invalidate();
    }
}

impl State {
    fn select(&self, index: usize) {
        if self.selected.get() != Some(index) {
            self.selected.set(Some(index));
            self.events.selected().emit(index);
            invalidate::invalidate();
        }
    }
    /// The area of each option within `area`.
    fn option_areas(&self, area: Rect) -> Vec<Rect> {
        let theme = theme::current();
        let symbol = super::width(theme.symbols.radio[0]) + 1;
        let mut x = area.x;
        self.options
            .borrow()
            .iter()
            .enumerate()
            .map(|(i, option)| {
                if self.horizontal.get() {
                    let width = symbol + super::width(option);
                    let rect = Rect::new(x, area.y, width, 1).intersection(area);
                    x = x.saturating_add(width + GAP);
                    rect
                } else {
                    Rect::new(area.x, area.y + i as u16, area.width, 1).intersection(area)
                }
            })
            .collect()
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let focused = crate::focus::is_focused(self);
        let selected = self.selected.get();
        let options = self.options.borrow();
        for (i, rect) in self.option_areas(area).into_iter().enumerate() {
            let on = selected == Some(i);
            let style = if !self.enabled.get() {
                theme.disabled
            } else if focused && (on || (selected.is_none() && i == 0)) {
                theme.focused
            } else {
                theme.text
            };
            let symbol_style = if on && self.enabled.get() {
                theme.accent
            } else {
                theme.muted
            };
            Line::from(vec![
                Span::styled(theme.symbols.radio[usize::from(on)], symbol_style),
                Span::raw(" "),
                Span::styled(options[i].as_str(), style),
            ])
            .render(rect, buf);
        }
    }
    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        let count = self.options.borrow().len();
        if count == 0 {
            return Handled::No;
        }
        if let Some(position) = super::left_press(event) {
            let area = mouse::area_of(self).unwrap_or_default();
            if let Some(i) = self
                .option_areas(area)
                .iter()
                .position(|r| r.contains(position))
            {
                self.select(i);
            }
            return Handled::Yes;
        }
        let current = self.selected.get();
        let (back, forward) = if self.horizontal.get() {
            (KeyCode::Left, KeyCode::Right)
        } else {
            (KeyCode::Up, KeyCode::Down)
        };
        let next = if super::pressed(event, forward) {
            current.map_or(0, |i| (i + 1).min(count - 1))
        } else if super::pressed(event, back) {
            current.map_or(0, |i| i.saturating_sub(1))
        } else if super::pressed(event, KeyCode::Home) {
            0
        } else if super::pressed(event, KeyCode::End) {
            count - 1
        } else if super::activates(event) {
            current.unwrap_or(0)
        } else {
            return Handled::No;
        };
        self.select(next);
        Handled::Yes
    }
    fn focusable(&self) -> bool {
        self.enabled.get() && !self.options.borrow().is_empty()
    }
    fn preferred_size(&self, available: Size) -> Size {
        let theme = theme::current();
        let symbol = super::width(theme.symbols.radio[0]) + 1;
        let options = self.options.borrow();
        let widths = options.iter().map(|o| symbol + super::width(o));
        if self.horizontal.get() {
            let total: u16 = widths.map(|w| w + GAP).sum();
            Size::new(total.saturating_sub(GAP).min(available.width), 1)
        } else {
            Size::new(widths.max().unwrap_or(0), options.len() as u16)
        }
    }
}
