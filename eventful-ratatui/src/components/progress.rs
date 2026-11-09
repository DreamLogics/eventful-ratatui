//! [`ProgressBar`] and [`Spinner`].
use super::handle;
use crate::{Component, invalidate, theme};
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
    time::{Duration, Instant},
};

/// Time between animation frames.
const FRAME: Duration = Duration::from_millis(80);

handle! {
    /// A one-line progress bar: `━━━━━━━──────── 42%`. Without a value it is
    /// indeterminate and animates.
    ProgressBar(BarState)
}

#[doc(hidden)]
pub struct BarState {
    value: Cell<Option<f64>>,
    label: RefCell<Option<String>>,
    started: Instant,
}

impl ProgressBar {
    /// An indeterminate bar.
    pub fn new() -> Self {
        Self(Rc::new(BarState {
            value: Cell::new(None),
            label: RefCell::default(),
            started: Instant::now(),
        }))
    }
    /// The progress, from 0 to 1, or `None` while indeterminate.
    pub fn value(&self) -> Option<f64> {
        self.0.value.get()
    }
    /// Set the progress, clamped to 0..=1; `None` makes it indeterminate.
    pub fn set_value(&self, value: Option<f64>) {
        self.0.value.set(value.map(|v| v.clamp(0.0, 1.0)));
        invalidate::invalidate();
    }
    /// Text shown after the bar instead of the percentage.
    pub fn set_label(&self, label: Option<String>) {
        *self.0.label.borrow_mut() = label;
        invalidate::invalidate();
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for BarState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let value = self.value.get();
        let label = match (&*self.label.borrow(), value) {
            (Some(label), _) => format!(" {label}"),
            (None, Some(v)) => format!(" {:>3.0}%", v * 100.0),
            (None, None) => String::new(),
        };
        let width = area.width.saturating_sub(super::width(&label)) as usize;
        let (start, filled) = match value {
            Some(v) => (0, (v * width as f64).round() as usize),
            None => {
                // A segment a quarter of the width bouncing back and forth.
                let segment = (width / 4).max(1);
                let span = width.saturating_sub(segment).max(1);
                let step = (self.started.elapsed().as_millis() / FRAME.as_millis()) as usize;
                let phase = step % (2 * span);
                let start = if phase < span {
                    phase
                } else {
                    2 * span - phase
                };
                invalidate::after(FRAME);
                (start, segment)
            }
        };
        let filled = filled.min(width - start.min(width));
        let rest = width - start.min(width) - filled;
        Line::from(vec![
            Span::styled("─".repeat(start.min(width)), theme.muted),
            Span::styled("━".repeat(filled), theme.accent),
            Span::styled("─".repeat(rest), theme.muted),
            Span::styled(label, theme.text),
        ])
        .render(area, buf);
    }
    fn preferred_size(&self, available: Size) -> Size {
        Size::new(available.width, 1)
    }
}

handle! {
    /// An activity indicator: `⠋ Loading`. Animates while spinning.
    Spinner(SpinnerState)
}

#[doc(hidden)]
pub struct SpinnerState {
    label: RefCell<String>,
    spinning: Cell<bool>,
    started: Instant,
}

impl Spinner {
    /// A spinning spinner labelled `label`; may be empty.
    pub fn new(label: impl Into<String>) -> Self {
        Self(Rc::new(SpinnerState {
            label: RefCell::new(label.into()),
            spinning: Cell::new(true),
            started: Instant::now(),
        }))
    }
    /// Start or stop; a stopped spinner draws only its label.
    pub fn set_spinning(&self, spinning: bool) {
        self.0.spinning.set(spinning);
        invalidate::invalidate();
    }
    /// Whether it spins.
    pub fn is_spinning(&self) -> bool {
        self.0.spinning.get()
    }
    /// Change the label.
    pub fn set_label(&self, label: impl Into<String>) {
        *self.0.label.borrow_mut() = label.into();
        invalidate::invalidate();
    }
}

impl Component for SpinnerState {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let label = self.label.borrow();
        let mut line = Line::default();
        if self.spinning.get() {
            let frames = theme.symbols.spinner;
            let step = (self.started.elapsed().as_millis() / FRAME.as_millis()) as usize;
            line.push_span(Span::styled(frames[step % frames.len()], theme.accent));
            if !label.is_empty() {
                line.push_span(" ");
            }
            invalidate::after(FRAME);
        }
        line.push_span(Span::styled(label.as_str(), theme.text));
        line.render(area, buf);
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(super::width(&self.label.borrow()) + 2, 1)
    }
}
