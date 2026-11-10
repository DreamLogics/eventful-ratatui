//! [`Button`].
use super::{ButtonEventsEmissions, ButtonEventsEventSet, ButtonEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Fill, Interaction},
};
use crossterm::event::Event;
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// How a button is emphasized, from loudest to quietest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonKind {
    /// The main action, filled with the primary color.
    Primary,
    /// An ordinary action, with a quieter fill.
    #[default]
    Secondary,
    /// A destructive action, filled with the destructive color.
    Destructive,
    /// No fill until focused or hovered: toolbars, dense rows.
    Ghost,
}

impl ButtonKind {
    fn fill(self) -> Fill {
        match self {
            Self::Primary => Fill::Primary,
            Self::Secondary => Fill::Secondary,
            Self::Destructive => Fill::Destructive,
            Self::Ghost => Fill::Ghost,
        }
    }
}

/// How tall a button is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonSize {
    /// One row.
    #[default]
    Small,
    /// Three rows: the label with half-block caps above and below.
    Large,
}

handle! {
    /// A push button: the label on a filled background. Click it, or focus it
    /// and press Enter or Space, to emit
    /// [`clicked`](super::ButtonEventsSignalsExt::clicked). It lights up
    /// under the pointer.
    ///
    /// A press followed by a release inside the button clicks it; releasing
    /// outside cancels, as in desktop toolkits.
    Button(State), events = (ButtonEventsEventSet, ButtonEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    label: RefCell<String>,
    kind: Cell<ButtonKind>,
    size: Cell<ButtonSize>,
    enabled: Cell<bool>,
    events: ButtonEventsEmissions,
    /// Runs synchronously on click, for composite components.
    pub(crate) on_click: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Button {
    /// A button labelled `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self(Rc::new(State {
            label: RefCell::new(label.into()),
            kind: Cell::new(ButtonKind::Secondary),
            size: Cell::new(ButtonSize::Small),
            enabled: Cell::new(true),
            events: ButtonEventsEmissions::default(),
            on_click: RefCell::default(),
        }))
    }
    /// Emphasize it as the main action.
    pub fn primary(self) -> Self {
        self.set_kind(ButtonKind::Primary);
        self
    }
    /// Emphasize it as a destructive action.
    pub fn destructive(self) -> Self {
        self.set_kind(ButtonKind::Destructive);
        self
    }
    /// Draw no fill until focused or hovered.
    pub fn ghost(self) -> Self {
        self.set_kind(ButtonKind::Ghost);
        self
    }
    /// Make it three rows tall.
    pub fn large(self) -> Self {
        self.0.size.set(ButtonSize::Large);
        self
    }
    /// Change the height.
    pub fn set_size(&self, size: ButtonSize) {
        self.0.size.set(size);
        invalidate::invalidate();
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
    /// Run `hook` synchronously on every click.
    pub(crate) fn on_click(&self, hook: impl Fn() + 'static) {
        *self.0.on_click.borrow_mut() = Some(Rc::new(hook));
    }
}

impl State {
    fn click(&self) {
        if self.enabled.get() {
            let hook = self.on_click.borrow().clone();
            if let Some(hook) = hook {
                hook();
            }
            self.events.clicked().emit();
        }
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let state = Interaction::of(self).disabled(!self.enabled.get());
        let style = theme.fill(self.kind.get().fill(), state);
        let size = self.preferred_size(area.as_size());
        let area = Rect {
            width: size.width.min(area.width),
            height: size.height.min(area.height),
            ..area
        };
        super::paint_button(buf, area, &self.label.borrow(), style, &theme);
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
        let height = match self.size.get() {
            ButtonSize::Small => 1,
            ButtonSize::Large => 3,
        };
        Size::new(super::width(&self.label.borrow()) + 4, height)
    }
}
