//! [`Checkbox`] and [`Switch`].
use super::{ToggleEventsEmissions, ToggleEventsEventSet, ToggleEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate,
    theme::{
        self, FOCUS_SHIFT, Fill, HOVER_SHIFT, Interaction, Theme,
        color::{blend, lift},
    },
};
use crossterm::event::Event;
use ratatui::{
    style::Style,
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
        let state = Interaction::of(self).disabled(!self.enabled.get());
        let row = Rect {
            width: self.preferred_size(area.as_size()).width.min(area.width),
            height: area.height.min(1),
            ..area
        };
        // The whole row is the target: a ghost fill while active.
        let fill = theme.fill(Fill::Ghost, state);
        buf.set_style(row, fill);
        let label_style = if state.disabled {
            theme.disabled()
        } else {
            fill
        };
        let mut line = Line::default();
        line.push_span(" ");
        match self.kind {
            Kind::Checkbox => {
                let mark = if state.disabled {
                    theme.disabled()
                } else if on {
                    theme.accent()
                } else {
                    theme.muted()
                };
                line.push_span(Span::styled(theme.symbols.checkbox[usize::from(on)], mark));
            }
            Kind::Switch => line.spans.extend(switch(&theme, on, state)),
        }
        let label = self.label.borrow();
        if !label.is_empty() {
            line.push_span(Span::styled(format!(" {label}"), label_style));
        }
        line.push_span(" ");
        line.render(row, buf);
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
            Kind::Checkbox => super::width(theme.symbols.checkbox[1]),
            Kind::Switch => SWITCH_WIDTH,
        };
        let label = self.label.borrow();
        let label = if label.is_empty() {
            0
        } else {
            super::width(&label) + 1
        };
        Size::new(symbol + label + 2, 1)
    }
}

/// Cells of a switch: two ends around a three-cell track.
const SWITCH_WIDTH: u16 = 5;

/// A switch's pill: `▐●  ▌` off, `▐  ●▌` on.
fn switch(theme: &Theme, on: bool, state: Interaction) -> Vec<Span<'static>> {
    let p = &theme.palette;
    let (track, knob) = if on {
        (p.primary, p.primary_foreground)
    } else {
        (theme.well_color(Interaction::default()), p.muted)
    };
    let track = if state.disabled {
        blend(track, p.background, crate::theme::DISABLED_DIM)
    } else if state.is_active() {
        lift(
            track,
            if state.hovered {
                HOVER_SHIFT
            } else {
                FOCUS_SHIFT
            },
        )
    } else {
        track
    };
    let inner = Style::new().fg(knob).bg(track);
    let [left, right] = theme.symbols.ends;
    let ends = Style::new().fg(track);
    let body = if on {
        format!("  {}", theme.symbols.knob)
    } else {
        format!("{}  ", theme.symbols.knob)
    };
    vec![
        Span::styled(left, ends),
        Span::styled(body, inner),
        Span::styled(right, ends),
    ]
}

/// Shared API of the two toggles.
macro_rules! toggle_api {
    ($name:ident, $kind:expr) => {
        impl $name {
            /// An unchecked toggle labelled `label`; may be empty.
            pub fn new(label: impl Into<String>) -> Self {
                Self(State::new($kind, label.into()))
            }
            /// Start checked (`true`) or not; toggles start unchecked.
            pub fn checked(self, on: bool) -> Self {
                self.0.on.set(on);
                self
            }
            /// Whether it is on.
            pub fn is_checked(&self) -> bool {
                self.0.on.get()
            }
            /// Turn it on or off without emitting `toggled`, for changes
            /// that did not come from the user.
            pub fn set_checked(&self, on: bool) {
                self.0.on.set(on);
                invalidate::invalidate();
            }
            /// Flip it as the user would, emitting `toggled`. Does nothing
            /// while disabled.
            pub fn toggle(&self) {
                self.0.toggle();
            }
            /// The label.
            pub fn label(&self) -> String {
                self.0.label.borrow().clone()
            }
            /// Change the label and redraw.
            pub fn set_label(&self, label: impl Into<String>) {
                *self.0.label.borrow_mut() = label.into();
                invalidate::invalidate();
            }
            /// Whether it can be toggled and focused. Toggles start enabled.
            pub fn is_enabled(&self) -> bool {
                self.0.enabled.get()
            }
            /// Enable or disable it. Disabled, it draws dimmed, ignores
            /// input, and Tab skips it.
            pub fn set_enabled(&self, enabled: bool) {
                self.0.enabled.set(enabled);
                invalidate::invalidate();
            }
        }
    };
}

handle! {
    /// A checkbox: `■ Label`. Space, Enter or a click toggles it and emits
    /// [`toggled`](super::ToggleEventsSignalsExt::toggled).
    ///
    /// A disabled checkbox draws dimmed, ignores input and is skipped by Tab.
    ///
    /// ```
    /// use eventful_ratatui::{components::Checkbox, testing::Harness};
    ///
    /// let bold = Checkbox::new("Bold");
    /// let mut ui = Harness::new(bold.clone(), 10, 1);
    /// assert_eq!(ui.lines(), [" □ Bold"]);
    ///
    /// ui.key("tab").key("space");
    /// assert!(bold.is_checked());
    /// assert_eq!(ui.lines(), [" ■ Bold"]);
    /// ```
    Checkbox(State), events = (ToggleEventsEventSet, ToggleEventsSignalsExt)
}
toggle_api!(Checkbox, Kind::Checkbox);

handle! {
    /// An on/off switch: `▐  ●▌ Label`. Behaves like [`Checkbox`]; use it for
    /// settings that apply immediately.
    ///
    /// ```
    /// use eventful_ratatui::{components::Switch, testing::Harness};
    ///
    /// let wifi = Switch::new("Wifi").checked(true);
    /// let mut ui = Harness::new(wifi.clone(), 12, 1);
    /// assert_eq!(ui.lines(), [" ▐  ●▌ Wifi"]);
    ///
    /// ui.click(1, 0);
    /// assert!(!wifi.is_checked());
    /// assert_eq!(ui.lines(), [" ▐●  ▌ Wifi"]);
    /// ```
    Switch(State), events = (ToggleEventsEventSet, ToggleEventsSignalsExt)
}
toggle_api!(Switch, Kind::Switch);
