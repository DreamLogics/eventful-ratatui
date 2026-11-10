//! [`Select`].
use super::{
    SelectEventsEmissions, SelectEventsEventSet, SelectEventsSignalsExt,
    choices::{Choices, Row},
    handle,
};
use crate::{
    Component, Handled, invalidate, mouse,
    overlay::{Overlay, Placement, overlay},
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{text::Span, widgets::Widget};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

handle! {
    /// A dropdown picking one of several options: `Medium ▾`.
    ///
    /// Enter, Space, Alt+Down or a click opens the list below it; Up/Down
    /// change the value directly while closed. Emits
    /// [`selected`](super::SelectEventsSignalsExt::selected).
    Select(State), events = (SelectEventsEventSet, SelectEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    options: RefCell<Vec<String>>,
    selected: Cell<Option<usize>>,
    placeholder: RefCell<String>,
    enabled: Cell<bool>,
    popup: RefCell<Option<Overlay>>,
    events: SelectEventsEmissions,
    /// This state, for opening the list from `on_input`.
    me: Weak<State>,
}

impl Select {
    /// A select over `options`, nothing selected.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(options: I) -> Self {
        let options = options.into_iter().map(Into::into).collect();
        Self(Rc::new_cyclic(|me| State {
            options: RefCell::new(options),
            selected: Cell::new(None),
            placeholder: RefCell::new("Select…".into()),
            enabled: Cell::new(true),
            popup: RefCell::default(),
            events: SelectEventsEmissions::default(),
            me: me.clone(),
        }))
    }
    /// Start with `index` selected.
    pub fn selected(self, index: usize) -> Self {
        self.set_selection(Some(index));
        self
    }
    /// Text shown while nothing is selected.
    pub fn placeholder(self, placeholder: impl Into<String>) -> Self {
        *self.0.placeholder.borrow_mut() = placeholder.into();
        self
    }
    /// The selected option.
    pub fn selection(&self) -> Option<usize> {
        self.0.selected.get()
    }
    /// The selected option's text.
    pub fn value(&self) -> Option<String> {
        self.selection()
            .and_then(|i| self.0.options.borrow().get(i).cloned())
    }
    /// Select `index` without emitting `selected`.
    pub fn set_selection(&self, index: Option<usize>) {
        let len = self.0.options.borrow().len();
        self.0.selected.set(index.filter(|i| *i < len));
        invalidate::invalidate();
    }
    /// Replace the options; the selection is cleared.
    pub fn set_options<I: IntoIterator<Item = S>, S: Into<String>>(&self, options: I) {
        *self.0.options.borrow_mut() = options.into_iter().map(Into::into).collect();
        self.0.selected.set(None);
        invalidate::invalidate();
    }
    /// Whether the list is open.
    pub fn is_open(&self) -> bool {
        self.0.popup.borrow().as_ref().is_some_and(Overlay::is_open)
    }
    /// Whether it can be used and focused.
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
        if index < self.options.borrow().len() && self.selected.get() != Some(index) {
            self.selected.set(Some(index));
            self.events.selected().emit(index);
            invalidate::invalidate();
        }
    }

    fn open(&self) {
        let Some(anchor) = mouse::area_of(self) else {
            return;
        };
        let rows = self
            .options
            .borrow()
            .iter()
            .map(|o| Row::new(o.clone()))
            .collect();
        let choices = Choices::new(rows, true, true);
        choices.set_highlight(self.selected.get());
        let me = self.me.clone();
        choices.on_pick(move |index| {
            if let Some(me) = me.upgrade() {
                me.select(index);
                let _closed = me.popup.borrow_mut().take();
            }
        });
        let popup = overlay(choices).placement(Placement::Below(anchor)).open();
        *self.popup.borrow_mut() = Some(popup);
    }

    fn input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if super::left_press(event).is_some() {
            self.open();
            return Handled::Yes;
        }
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        let len = self.options.borrow().len();
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => self.open(),
            KeyCode::Down if key.modifiers == KeyModifiers::ALT => self.open(),
            KeyCode::Down if key.modifiers.is_empty() && len > 0 => {
                self.select(self.selected.get().map_or(0, |i| (i + 1).min(len - 1)));
            }
            KeyCode::Up if key.modifiers.is_empty() && len > 0 => {
                self.select(self.selected.get().map_or(0, |i| i.saturating_sub(1)));
            }
            _ => return Handled::No,
        }
        Handled::Yes
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let state = Interaction::of(self).disabled(!self.enabled.get());
        let style = theme.well(state);
        let row = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        buf.set_style(row, style);
        let (text, text_style) = match self
            .selected
            .get()
            .and_then(|i| self.options.borrow().get(i).cloned())
        {
            Some(value) => (value, style),
            None => (
                self.placeholder.borrow().clone(),
                style.patch(theme.placeholder()),
            ),
        };
        Span::styled(format!(" {text}"), text_style).render(row, buf);
        let arrow = Rect::new(
            row.right().saturating_sub(2),
            row.y,
            2.min(row.width),
            row.height,
        );
        let chevron = if state.is_active() {
            theme.text()
        } else {
            theme.muted()
        };
        Span::styled("▾", chevron).render(arrow, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        self.input(event)
    }
    fn focusable(&self) -> bool {
        self.enabled.get()
    }
    fn preferred_size(&self, available: Size) -> Size {
        let widest = self
            .options
            .borrow()
            .iter()
            .map(|o| super::width(o))
            .chain([super::width(&self.placeholder.borrow())])
            .max()
            .unwrap_or(0);
        Size::new((widest + 4).min(available.width), 1)
    }
}
