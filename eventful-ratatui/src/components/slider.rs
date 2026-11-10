//! [`Slider`] and [`SpinBox`].
use super::{ValueEventsEmissions, ValueEventsEventSet, ValueEventsSignalsExt, handle};
use crate::{
    Component, Handled, invalidate, mouse,
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{cell::Cell, rc::Rc};

/// A number within a range, moved in steps.
#[doc(hidden)]
pub struct Range {
    value: Cell<f64>,
    min: Cell<f64>,
    max: Cell<f64>,
    step: Cell<f64>,
    enabled: Cell<bool>,
    events: ValueEventsEmissions,
}

impl Range {
    fn new(min: f64, max: f64, step: f64) -> Self {
        Self {
            value: Cell::new(min),
            min: Cell::new(min),
            max: Cell::new(max.max(min)),
            step: Cell::new(step.abs().max(f64::EPSILON)),
            enabled: Cell::new(true),
            events: ValueEventsEmissions::default(),
        }
    }
    /// Clamp to the range and snap to the step grid.
    fn snap(&self, value: f64) -> f64 {
        let (min, max, step) = (self.min.get(), self.max.get(), self.step.get());
        let snapped = min + ((value - min) / step).round() * step;
        snapped.clamp(min, max)
    }
    fn set(&self, value: f64, emit: bool) {
        let value = self.snap(value);
        if value != self.value.get() {
            self.value.set(value);
            if emit {
                self.events.changed().emit(value);
            }
            invalidate::invalidate();
        }
    }
    fn nudge(&self, steps: f64) {
        self.set(self.value.get() + steps * self.step.get(), true);
    }
    /// Arrow, page and end keys; `back`/`forward` are the arrow keys.
    fn keys(&self, event: &Event, back: KeyCode, forward: KeyCode) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        if !key.modifiers.is_empty() {
            return Handled::No;
        }
        match key.code {
            code if code == forward => self.nudge(1.0),
            code if code == back => self.nudge(-1.0),
            KeyCode::PageUp => self.nudge(10.0),
            KeyCode::PageDown => self.nudge(-10.0),
            KeyCode::Home => self.set(self.min.get(), true),
            KeyCode::End => self.set(self.max.get(), true),
            _ => return Handled::No,
        }
        Handled::Yes
    }
    /// The value as text, with as many decimals as the step needs.
    fn text(&self) -> String {
        let step = self.step.get();
        let decimals = if step.fract() == 0.0 {
            0
        } else {
            (-step.log10().floor()).max(0.0) as usize
        };
        format!("{:.*}", decimals, self.value.get())
    }
}

/// The shared API of slider and spin box handles.
macro_rules! range_api {
    ($name:ident) => {
        impl $name {
            /// The value.
            pub fn value(&self) -> f64 {
                self.0.value.get()
            }
            /// Set the value, clamped and snapped, without emitting `changed`.
            pub fn set_value(&self, value: f64) {
                self.0.set(value, false);
            }
            /// Start at `value`.
            pub fn with_value(self, value: f64) -> Self {
                self.set_value(value);
                self
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
    };
}

handle! {
    /// A horizontal slider: `━━━━━━●───── 42`. Left/Right move a step,
    /// PageUp/PageDown ten, Home/End to the ends; click or drag to set.
    /// Emits [`changed`](super::ValueEventsSignalsExt::changed).
    Slider(Range), events = (ValueEventsEventSet, ValueEventsSignalsExt)
}
range_api!(Slider);

impl Slider {
    /// A slider from `min` to `max` in steps of `step`, at `min`.
    pub fn new(min: f64, max: f64, step: f64) -> Self {
        Self(Rc::new(Range::new(min, max, step)))
    }
}

/// Columns reserved for the value label.
fn label_width(range: &Range) -> u16 {
    let longest = [range.min.get(), range.max.get()]
        .map(|v| {
            format!(
                "{:.*}",
                range.text().split('.').nth(1).map_or(0, str::len),
                v
            )
            .len()
        })
        .into_iter()
        .max()
        .unwrap_or(1);
    longest as u16 + 1
}

/// The track of a slider in `area`, left of the label.
fn track(range: &Range, area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y,
        area.width.saturating_sub(label_width(range)),
        1.min(area.height),
    )
}

impl Component for Range {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let track = track(self, area);
        let span = (self.max.get() - self.min.get()).max(f64::EPSILON);
        let ratio = (self.value.get() - self.min.get()) / span;
        let width = usize::from(track.width.max(1));
        let knob = ((width - 1) as f64 * ratio).round() as usize;
        let state = Interaction::of(self).disabled(!self.enabled.get());
        let p = &theme.palette;
        let (fill, knob_style) = if state.disabled {
            (theme.disabled(), theme.disabled())
        } else if state.is_active() {
            let knob = Style::new()
                .fg(p.primary)
                .bg(theme.well_color(state.hovered(true)))
                .add_modifier(Modifier::BOLD);
            (theme.accent(), knob)
        } else {
            (theme.accent(), theme.text())
        };
        let rest = Style::new().fg(theme.well_color(state.hovered(true)));
        let label = format!(
            " {:>w$}",
            self.text(),
            w = usize::from(label_width(self) - 1)
        );
        Line::from(vec![
            Span::styled("━".repeat(knob), fill),
            Span::styled("●", knob_style),
            Span::styled("━".repeat(width - knob - 1), rest),
            Span::styled(label, theme.text()),
        ])
        .render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if let Event::Mouse(m) = event {
            let dragging = matches!(
                m.kind,
                MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left)
            );
            if let Some(wheel) = super::wheel(event) {
                self.nudge(-f64::from(wheel));
                return Handled::Yes;
            }
            if dragging && let Some(area) = mouse::area_of(self) {
                let track = track(self, area);
                let x = m.column.clamp(track.x, track.right().saturating_sub(1)) - track.x;
                let ratio = f64::from(x) / f64::from(track.width.saturating_sub(1).max(1));
                self.set(
                    self.min.get() + ratio * (self.max.get() - self.min.get()),
                    true,
                );
                return Handled::Yes;
            }
            return (matches!(m.kind, MouseEventKind::Up(MouseButton::Left))).into();
        }
        self.keys(event, KeyCode::Left, KeyCode::Right)
    }
    fn focusable(&self) -> bool {
        self.enabled.get()
    }
    fn preferred_size(&self, available: Size) -> Size {
        Size::new(available.width.min(40), 1)
    }
}

handle! {
    /// A number with step buttons: `‹ 42 ›`. Up/Down (or Left/Right) step,
    /// PageUp/PageDown ten steps, Home/End to the ends; click the arrows or
    /// use the wheel. Emits [`changed`](super::ValueEventsSignalsExt::changed).
    SpinBox(SpinState), events = (ValueEventsEventSet, ValueEventsSignalsExt)
}
range_api!(SpinBox);

#[doc(hidden)]
pub struct SpinState(Range);

impl std::ops::Deref for SpinState {
    type Target = Range;
    fn deref(&self) -> &Range {
        &self.0
    }
}

impl SpinBox {
    /// A spin box from `min` to `max` in steps of `step`, at `min`.
    pub fn new(min: f64, max: f64, step: f64) -> Self {
        Self(Rc::new(SpinState(Range::new(min, max, step))))
    }
}

impl Component for SpinState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let state = Interaction::of(self).disabled(!self.enabled.get());
        let style = theme.well(state);
        let row = Rect {
            width: self.preferred_size(area.as_size()).width.min(area.width),
            height: area.height.min(1),
            ..area
        };
        buf.set_style(row, style);
        let width = usize::from(label_width(self) - 1);
        let arrow = if state.is_active() {
            theme.text()
        } else {
            theme.muted()
        };
        Line::from(vec![
            Span::styled("‹", arrow),
            Span::styled(format!(" {:>width$} ", self.text()), style),
            Span::styled("›", arrow),
        ])
        .render(row, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if let Some(wheel) = super::wheel(event) {
            self.nudge(-f64::from(wheel));
            return Handled::Yes;
        }
        if let Some(position) = super::left_press(event) {
            if let Some(area) = mouse::area_of(self) {
                let right = area.x + label_width(self) + 2;
                if position.x == area.x {
                    self.nudge(-1.0);
                } else if position.x >= right {
                    self.nudge(1.0);
                }
            }
            return Handled::Yes;
        }
        match self.keys(event, KeyCode::Down, KeyCode::Up) {
            Handled::No => self.keys(event, KeyCode::Left, KeyCode::Right),
            handled => handled,
        }
    }
    fn focusable(&self) -> bool {
        self.enabled.get()
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(label_width(self) + 3, 1)
    }
}
