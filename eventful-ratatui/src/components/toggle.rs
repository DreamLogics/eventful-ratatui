//! [`Checkbox`] and [`Switch`].
use super::{ToggleEventsEmissions, ToggleEventsEventSet, ToggleEventsSignalsExt, handle};
use crate::{Component, Handled, invalidate, theme};
use crossterm::event::Event;
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

#[derive(Clone, Copy)]
enum Kind {
    Checkbox,
    Switch,
}

#[doc(hidden)]
pub struct State {
    kind: Kind,
    label: RefCell<String>,
    on: Cell<bool>,
    enabled: Cell<bool>,
    events: ToggleEventsEmissions,
}

impl State {
    fn new(kind: Kind, label: String) -> Rc<Self> {
        Rc::new(Self {
            kind,
            label: RefCell::new(label),
            on: Cell::new(false),
            enabled: Cell::new(true),
            events: ToggleEventsEmissions::default(),
        })
    }
    fn toggle(&self) {
        if self.enabled.get() {
            self.on.set(!self.on.get());
            self.events.toggled().emit(self.on.get());
            invalidate::invalidate();
        }
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let on = self.on.get();
        let symbol = match self.kind {
            Kind::Checkbox => theme.symbols.checkbox[usize::from(on)],
            Kind::Switch => theme.symbols.switch[usize::from(on)],
        };
        let (symbol_style, label_style) = if !self.enabled.get() {
            (theme.disabled, theme.disabled)
        } else if crate::focus::is_focused(self) {
            (theme.focused, theme.text)
        } else if on {
            (theme.accent, theme.text)
        } else {
            (theme.muted, theme.text)
        };
        let label = self.label.borrow();
        let mut line = Line::from(Span::styled(symbol, symbol_style));
        if !label.is_empty() {
            line.push_span(Span::styled(format!(" {label}"), label_style));
        }
        line.render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if super::activates(event) || super::left_press(event).is_some() {
            self.toggle();
            return Handled::Yes;
        }
        Handled::No
    }
    fn focusable(&self) -> bool {
        self.enabled.get()
    }
    fn preferred_size(&self, _: Size) -> Size {
        let theme = theme::current();
        let symbol = match self.kind {
            Kind::Checkbox => theme.symbols.checkbox[1],
            Kind::Switch => theme.symbols.switch[1],
        };
        let label = self.label.borrow();
        let gap = u16::from(!label.is_empty());
        Size::new(super::width(symbol) + gap + super::width(&label), 1)
    }
}

/// Shared API of the two toggles.
macro_rules! toggle_api {
    ($name:ident, $kind:expr) => {
        impl $name {
            /// An unchecked toggle labelled `label`; may be empty.
            pub fn new(label: impl Into<String>) -> Self {
                Self(State::new($kind, label.into()))
            }
            /// Start checked or not.
            pub fn checked(self, on: bool) -> Self {
                self.0.on.set(on);
                self
            }
            /// Whether it is on.
            pub fn is_checked(&self) -> bool {
                self.0.on.get()
            }
            /// Turn it on or off without emitting `toggled`.
            pub fn set_checked(&self, on: bool) {
                self.0.on.set(on);
                invalidate::invalidate();
            }
            /// Flip it as the user would, emitting `toggled`.
            pub fn toggle(&self) {
                self.0.toggle();
            }
            /// The label.
            pub fn label(&self) -> String {
                self.0.label.borrow().clone()
            }
            /// Change the label.
            pub fn set_label(&self, label: impl Into<String>) {
                *self.0.label.borrow_mut() = label.into();
                invalidate::invalidate();
            }
            /// Whether it can be toggled and focused.
            pub fn is_enabled(&self) -> bool {
                self.0.enabled.get()
            }
            /// Enable or disable it.
            pub fn set_enabled(&self, enabled: bool) {
                self.0.enabled.set(enabled);
                invalidate::invalidate();
            }
        }
    };
}

handle! {
    /// A checkbox: `[x] Label`. Space, Enter or a click toggles it and emits
    /// [`toggled`](super::ToggleEventsSignalsExt::toggled).
    Checkbox(State), events = (ToggleEventsEventSet, ToggleEventsSignalsExt)
}
toggle_api!(Checkbox, Kind::Checkbox);

handle! {
    /// An on/off switch: `──● Label`. Behaves like [`Checkbox`]; use it for
    /// settings that apply immediately.
    Switch(State), events = (ToggleEventsEventSet, ToggleEventsSignalsExt)
}
toggle_api!(Switch, Kind::Switch);
