//! [`Badge`], [`Chip`] and [`Breadcrumb`].
use super::{
    ActionEventsEmissions, ActionEventsEventSet, ActionEventsSignalsExt, ToggleEventsEmissions,
    ToggleEventsEventSet, ToggleEventsSignalsExt, handle,
};
use crate::{
    Component, Handled, focus, invalidate, mouse,
    theme::{self, Fill, Interaction, Tone, color::blend},
};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    style::{Modifier, Style},
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
    /// A small status pill: `3`, `NEW`, `failed`, tinted by its
    /// [`Tone`]. Not interactive.
    Badge(BadgeState)
}

#[doc(hidden)]
pub struct BadgeState {
    text: RefCell<String>,
    tone: Cell<Tone>,
    solid: Cell<bool>,
}

impl Badge {
    /// A badge in the primary tone.
    pub fn new(text: impl Into<String>) -> Self {
        Self(Rc::new(BadgeState {
            text: RefCell::new(text.into()),
            tone: Cell::new(Tone::Primary),
            solid: Cell::new(false),
        }))
    }
    /// Color it by meaning.
    pub fn tone(self, tone: Tone) -> Self {
        self.0.tone.set(tone);
        self
    }
    /// Fill it with its tone instead of tinting it.
    pub fn solid(self) -> Self {
        self.0.solid.set(true);
        self
    }
    /// Change the text.
    pub fn set_text(&self, text: impl Into<String>) {
        *self.0.text.borrow_mut() = text.into();
        invalidate::invalidate();
    }
}

impl Component for BadgeState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let tone = theme.tone(self.tone.get());
        let style = if self.solid.get() {
            Style::new()
                .fg(theme.palette.background)
                .bg(tone)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new()
                .fg(tone)
                .bg(blend(tone, theme.palette.background, 78))
        };
        Line::from(super::pill(&theme, &self.text.borrow(), style)).render(area, buf);
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(super::width(&self.text.borrow()) + 4, 1)
    }
}

handle! {
    /// A toggleable filter chip: a pill, filled with the primary color while
    /// selected. Space,
    /// Enter or a click toggles it. Emits
    /// [`toggled`](super::ToggleEventsSignalsExt::toggled).
    Chip(ChipState), events = (ToggleEventsEventSet, ToggleEventsSignalsExt)
}

#[doc(hidden)]
pub struct ChipState {
    label: RefCell<String>,
    on: Cell<bool>,
    events: ToggleEventsEmissions,
}

impl Chip {
    /// An unselected chip.
    pub fn new(label: impl Into<String>) -> Self {
        Self(Rc::new(ChipState {
            label: RefCell::new(label.into()),
            on: Cell::new(false),
            events: ToggleEventsEmissions::default(),
        }))
    }
    /// Start selected or not.
    pub fn selected(self, on: bool) -> Self {
        self.0.on.set(on);
        self
    }
    /// Whether it is selected.
    pub fn is_selected(&self) -> bool {
        self.0.on.get()
    }
    /// Select it or not, without emitting `toggled`.
    pub fn set_selected(&self, on: bool) {
        self.0.on.set(on);
        invalidate::invalidate();
    }
}

impl Component for ChipState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let state = Interaction::of(self);
        let fill = if self.on.get() {
            Fill::Primary
        } else {
            Fill::Secondary
        };
        let style = theme.fill(fill, state);
        Line::from(super::pill(&theme, &self.label.borrow(), style)).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        if super::activates(event) || super::left_press(event).is_some() {
            self.on.set(!self.on.get());
            self.events.toggled().emit(self.on.get());
            invalidate::invalidate();
            return Handled::Yes;
        }
        Handled::No
    }
    fn focusable(&self) -> bool {
        true
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(super::width(&self.label.borrow()) + 4, 1)
    }
}

handle! {
    /// A path of places: `Home › Projects › eventful`. Each item but the
    /// last can be clicked, or reached with Left/Right and Enter while
    /// focused. Emits [`triggered`](super::ActionEventsSignalsExt::triggered)
    /// with the item's id.
    Breadcrumb(CrumbState), events = (ActionEventsEventSet, ActionEventsSignalsExt)
}

#[doc(hidden)]
pub struct CrumbState {
    /// Ids and labels.
    items: RefCell<Vec<(String, String)>>,
    highlight: Cell<Option<usize>>,
    /// Item areas at the last render.
    areas: RefCell<Vec<Rect>>,
    events: ActionEventsEmissions,
}

impl Breadcrumb {
    /// An empty path.
    pub fn new() -> Self {
        Self(Rc::new(CrumbState {
            items: RefCell::default(),
            highlight: Cell::new(None),
            areas: RefCell::default(),
            events: ActionEventsEmissions::default(),
        }))
    }
    /// Append an item; `id` is what `triggered` reports.
    pub fn item(self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.0.items.borrow_mut().push((id.into(), label.into()));
        self
    }
    /// Replace the items.
    pub fn set_items(&self, items: impl IntoIterator<Item = (String, String)>) {
        *self.0.items.borrow_mut() = items.into_iter().collect();
        self.0.highlight.set(None);
        invalidate::invalidate();
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl CrumbState {
    fn trigger(&self, index: usize) {
        let id = self.items.borrow().get(index).map(|(id, _)| id.clone());
        if let Some(id) = id {
            self.events.triggered().emit(id);
        }
    }
}

impl Component for CrumbState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let items = self.items.borrow();
        let focused = focus::is_focused(self);
        let pointer = mouse::hover_position(self);
        let mut line = Line::default();
        let mut areas = Vec::new();
        let mut x = area.x;
        for (i, (_, label)) in items.iter().enumerate() {
            if i > 0 {
                line.push_span(Span::styled(theme.symbols.separator, theme.muted()));
                x += super::width(theme.symbols.separator);
            }
            let last = i + 1 == items.len();
            let width = super::width(label);
            let rect = Rect::new(x, area.y, width, 1).intersection(area);
            let hovered = !last && pointer.is_some_and(|p| rect.contains(p));
            let style = if focused && self.highlight.get() == Some(i) {
                theme.fill(Fill::Ghost, Interaction::default().focused(true))
            } else if last {
                theme.title()
            } else if hovered {
                theme.text().add_modifier(Modifier::UNDERLINED)
            } else {
                theme.muted()
            };
            line.push_span(Span::styled(label.clone(), style));
            areas.push(rect);
            x = x.saturating_add(width);
        }
        line.render(area, buf);
        *self.areas.borrow_mut() = areas;
    }
    fn on_input(&self, event: &Event) -> Handled {
        let clickable = self.items.borrow().len().saturating_sub(1);
        if let Some(position) = super::left_press(event) {
            let hit = self
                .areas
                .borrow()
                .iter()
                .position(|a| a.contains(position));
            if let Some(index) = hit.filter(|&i| i < clickable) {
                self.trigger(index);
            }
            return Handled::Yes;
        }
        if clickable == 0 {
            return Handled::No;
        }
        let current = self.highlight.get().unwrap_or(clickable);
        let next = if super::pressed(event, KeyCode::Left) {
            current.saturating_sub(1)
        } else if super::pressed(event, KeyCode::Right) {
            (current + 1).min(clickable - 1)
        } else if super::activates(event) {
            self.trigger(current.min(clickable - 1));
            return Handled::Yes;
        } else {
            return Handled::No;
        };
        self.highlight.set(Some(next));
        invalidate::invalidate();
        Handled::Yes
    }
    fn focusable(&self) -> bool {
        self.items.borrow().len() > 1
    }
    fn preferred_size(&self, available: Size) -> Size {
        let separator = super::width(theme::current().symbols.separator);
        let items = self.items.borrow();
        let width: u16 = items.iter().map(|(_, l)| super::width(l)).sum::<u16>()
            + separator * items.len().saturating_sub(1) as u16;
        Size::new(width.min(available.width), 1)
    }
}
