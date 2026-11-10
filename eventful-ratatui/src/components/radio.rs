//! [`RadioGroup`].
use super::{SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Fill, Interaction},
};
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
    /// One choice out of several: `● Small  ○ Large`. The group takes
    /// focus as a whole; arrows, Home and End move the choice, a click picks
    /// one. Emits [`selected`](super::SelectEventsSignalsExt::selected).
    ///
    /// Options are stacked one per row, or laid out in a row with
    /// [`horizontal`](Self::horizontal). The arrows stop at the first and
    /// last option instead of wrapping.
    ///
    /// ```
    /// use eventful_ratatui::{components::RadioGroup, testing::Harness};
    ///
    /// let size = RadioGroup::new(["Small", "Medium", "Large"]).with_selection(0);
    /// let mut ui = Harness::new(size.clone(), 12, 3);
    /// assert_eq!(ui.lines(), [" ● Small", " ○ Medium", " ○ Large"]);
    ///
    /// ui.key("tab").key("down");
    /// assert_eq!(size.selection(), Some(1));
    /// ui.click(1, 2);
    /// assert_eq!(size.selection(), Some(2));
    /// ```
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

/// Columns between horizontal options, beside their padding.
const GAP: u16 = 1;

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
    /// Lay the options out in a row, three columns apart; Left/Right move
    /// the choice instead of Up/Down.
    pub fn horizontal(self) -> Self {
        self.0.horizontal.set(true);
        self
    }
    /// Start with `index` selected.
    pub fn with_selection(self, index: usize) -> Self {
        self.0.selected.set(Some(index));
        self
    }
    /// The index of the selected option, if any.
    pub fn selection(&self) -> Option<usize> {
        self.0.selected.get()
    }
    /// Select `index`, or nothing with `None`, without emitting `selected`.
    pub fn set_selection(&self, index: Option<usize>) {
        self.0.selected.set(index);
        invalidate::invalidate();
    }
    /// The options.
    pub fn options(&self) -> Vec<String> {
        self.0.options.borrow().clone()
    }
    /// Whether it can be changed and focused. Groups start enabled.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get()
    }
    /// Enable or disable it. Disabled, it draws dimmed, ignores input, and
    /// Tab skips it.
    /// Disabling it while it has focus moves focus to the next focusable
    /// component; see [`focus`](crate::focus).
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
        let symbol = super::width(theme.symbols.radio[0]) + 3;
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
                    let width = symbol + super::width(option);
                    Rect::new(area.x, area.y + i as u16, width, 1).intersection(area)
                }
            })
            .collect()
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let group = Interaction::of(self).disabled(!self.enabled.get());
        let pointer = mouse::hover_position(self);
        let selected = self.selected.get();
        let options = self.options.borrow();
        for (i, rect) in self.option_areas(area).into_iter().enumerate() {
            let on = selected == Some(i);
            // The option keys act on is the focused one.
            let state = Interaction {
                focused: group.focused && (on || (selected.is_none() && i == 0)),
                hovered: pointer.is_some_and(|p| rect.contains(p)),
                disabled: group.disabled,
            };
            let fill = theme.fill(Fill::Ghost, state);
            buf.set_style(rect, fill);
            let symbol_style = if group.disabled {
                theme.disabled()
            } else if on {
                theme.accent()
            } else {
                theme.muted()
            };
            let label = if group.disabled {
                theme.disabled()
            } else {
                fill
            };
            Line::from(vec![
                Span::raw(" "),
                Span::styled(theme.symbols.radio[usize::from(on)], symbol_style),
                Span::raw(" "),
                Span::styled(options[i].as_str(), label),
                Span::raw(" "),
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
        let symbol = super::width(theme.symbols.radio[0]) + 3;
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
