//! [`Button`].
use super::{ButtonEventsEmissions, ButtonEventsEventSet, ButtonEventsSignalsExt, handle};
use crate::{Component, Handled, invalidate, mouse, theme};
use crossterm::event::Event;
use ratatui::{style::Modifier, text::Line, widgets::Widget};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::cell::{Cell, RefCell};

/// How a button is emphasized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonKind {
    /// A plain button.
    #[default]
    Normal,
    /// The main action, in the accent color.
    Primary,
    /// A destructive action, in the error color.
    Danger,
}

handle! {
    /// A push button: `[ Save ]`. Click it, or focus it and press Enter or
    /// Space, to emit [`clicked`](super::ButtonEventsSignalsExt::clicked).
    ///
    /// A press followed by a release inside the button clicks it; releasing
    /// outside cancels, as in desktop toolkits.
    Button(State), events = (ButtonEventsEventSet, ButtonEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    label: RefCell<String>,
    kind: Cell<ButtonKind>,
    enabled: Cell<bool>,
    events: ButtonEventsEmissions,
}

impl Button {
    /// A button labelled `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self(std::rc::Rc::new(State {
            label: RefCell::new(label.into()),
            kind: Cell::new(ButtonKind::Normal),
            enabled: Cell::new(true),
            events: ButtonEventsEmissions::default(),
        }))
    }
    /// Emphasize it as the main action.
    pub fn primary(self) -> Self {
        self.set_kind(ButtonKind::Primary);
        self
    }
    /// Emphasize it as a destructive action.
    pub fn danger(self) -> Self {
        self.set_kind(ButtonKind::Danger);
        self
    }
    /// Change the emphasis.
    pub fn set_kind(&self, kind: ButtonKind) {
        self.0.kind.set(kind);
        invalidate::invalidate();
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
    /// Whether it can be clicked and focused.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get()
    }
    /// Enable or disable it.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        invalidate::invalidate();
    }
    /// Click it, as the user would.
    pub fn click(&self) {
        self.0.click();
    }
}

impl State {
    fn click(&self) {
        if self.enabled.get() {
            self.events.clicked().emit();
        }
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let style = if !self.enabled.get() {
            theme.disabled
        } else if crate::focus::is_focused(self) {
            theme.focused
        } else {
            match self.kind.get() {
                ButtonKind::Normal => theme.text,
                ButtonKind::Primary => theme.accent.add_modifier(Modifier::BOLD),
                ButtonKind::Danger => theme.error,
            }
        };
        Line::styled(format!("[ {} ]", self.label.borrow()), style).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if super::activates(event) {
            self.click();
            return Handled::Yes;
        }
        if super::left_press(event).is_some() {
            return Handled::Yes; // Capture the mouse until the release.
        }
        if let Some(position) = super::left_release(event) {
            if mouse::area_of(self).is_some_and(|area| area.contains(position)) {
                self.click();
            }
            return Handled::Yes;
        }
        Handled::No
    }
    fn focusable(&self) -> bool {
        self.enabled.get()
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(super::width(&self.label.borrow()) + 4, 1)
    }
}
