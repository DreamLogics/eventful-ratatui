//! [`ProgressBar`] and [`Spinner`].
use super::handle;
use crate::{
    Component, invalidate,
    theme::{self, Interaction},
};
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
    time::{Duration, Instant},
};

/// Time between animation frames.
const FRAME: Duration = Duration::from_millis(80);

handle! {
    /// A slim progress bar on a track: `█████▋      42%`, with an optional
    /// label above it. Without a value it is indeterminate and animates.
    ///
    /// The fill moves in eighths of a cell. Not interactive. Set the value
    /// from a worker's events as work completes; each change redraws.
    ///
    /// ```
    /// use eventful_ratatui::{components::ProgressBar, testing::Harness};
    ///
    /// let download = ProgressBar::new();
    /// download.set_value(Some(0.5));
    /// let mut ui = Harness::new(download.clone(), 14, 1);
    /// assert_eq!(ui.lines(), ["████▌      50%"]);
    ///
    /// // With a label and two rows, the percentage moves up beside it.
    /// download.set_label(Some("Downloading".into()));
    /// ui.resize(20, 2);
    /// assert_eq!(ui.lines(), ["Downloading      50%", "██████████"]);
    /// ```
    ProgressBar(BarState)
}

#[doc(hidden)]
pub struct BarState {
    value: Cell<Option<f64>>,
    label: RefCell<Option<String>>,
    started: Instant,
}

impl ProgressBar {
    /// An indeterminate bar with no label; see also [`Default`].
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
    /// Text above the bar, with the percentage at its right, or `None` for
    /// no label. Given one row, the label goes after the bar instead of the
    /// percentage. A label makes the bar's preferred height two rows.
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
        if area.is_empty() {
            return;
        }
        let theme = theme::current();
        let value = self.value.get();
        let percent = value.map(|v| format!("{:.0}%", v * 100.0));
        let label = self.label.borrow().clone();
        // The label above the bar, the percentage at its right; on one row,
        // the percentage after the bar.
        let bar = if let Some(label) = &label
            && area.height >= 2
        {
            let row = Rect { height: 1, ..area };
            Span::styled(label.as_str(), theme.text()).render(row, buf);
            if let Some(percent) = &percent {
                Line::styled(percent.as_str(), theme.muted())
                    .alignment(ratatui::layout::Alignment::Right)
                    .render(row, buf);
            }
            Rect::new(area.x, area.y + 1, area.width, 1)
        } else {
            let text = match (&label, &percent) {
                (Some(label), _) => format!(" {label}"),
                (None, Some(percent)) => format!(" {percent:>4}"),
                (None, None) => String::new(),
            };
            let width = super::width(&text).min(area.width);
            let tail = Rect::new(area.right() - width, area.y, width, 1);
            Span::styled(text, theme.muted()).render(tail, buf);
            Rect::new(area.x, area.y, area.width - width, 1)
        };
        let track = theme.well_color(Interaction::default());
        let cells = usize::from(bar.width);
        // In eighths of a cell, so the fill moves smoothly.
        let (start, filled) = match value {
            Some(v) => (0, (v * cells as f64 * 8.0).round() as usize),
            None => {
                // A quarter of the track bouncing back and forth.
                let segment = (cells / 4).max(1);
                let span = cells.saturating_sub(segment).max(1);
                let step = (self.started.elapsed().as_millis() / FRAME.as_millis()) as usize;
                let phase = step % (2 * span);
                let start = if phase < span {
                    phase
                } else {
                    2 * span - phase
                };
                invalidate::after(FRAME);
                (start * 8, segment * 8)
            }
        };
        let fill = Style::new().fg(theme.palette.primary).bg(track);
        for i in 0..cells {
            let cell = &mut buf[(bar.x + i as u16, bar.y)];
            let from = i * 8;
            // Segments start on a cell boundary, so only the end is partial.
            let symbol = if from < start {
                " "
            } else {
                LEFT[(start + filled).saturating_sub(from).min(8)]
            };
            cell.set_symbol(symbol).set_style(fill);
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        let height = if self.label.borrow().is_some() { 2 } else { 1 };
        Size::new(available.width, height.min(available.height))
    }
}

/// Eighths of a cell filled from the left.
const LEFT: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

handle! {
    /// An activity indicator: `⠋ Loading`. Animates while spinning.
    ///
    /// The frames come from the theme's
    /// [symbols](crate::theme::Theme::symbols). Not interactive.
    ///
    /// ```
    /// use eventful_ratatui::{components::Spinner, testing::Harness};
    ///
    /// let loading = Spinner::new("Loading");
    /// let mut ui = Harness::new(loading.clone(), 12, 1);
    /// assert!(ui.lines()[0].ends_with(" Loading"));
    ///
    /// loading.set_spinning(false);
    /// ui.render();
    /// assert_eq!(ui.lines(), ["Loading"]);
    /// ```
    Spinner(SpinnerState)
}

#[doc(hidden)]
pub struct SpinnerState {
    label: RefCell<String>,
    spinning: Cell<bool>,
    started: Instant,
}

impl Spinner {
    /// A spinning spinner labelled `label`; the label may be empty.
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
    /// Change the label and redraw.
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
            line.push_span(Span::styled(frames[step % frames.len()], theme.accent()));
            if !label.is_empty() {
                line.push_span(" ");
            }
            invalidate::after(FRAME);
        }
        line.push_span(Span::styled(label.as_str(), theme.text()));
        line.render(area, buf);
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(super::width(&self.label.borrow()) + 2, 1)
    }
}
