//! [`Toast`] notifications.
use crate::{
    Component, Handled, invalidate, mouse,
    overlay::{Closer, Placement, overlay},
    theme::{self, Tone},
};
use crossterm::event::Event;
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

/// A short notification in the bottom-right corner that disappears by
/// itself. Toasts stack, take no focus, and close early on a click.
///
/// ```
/// use eventful_ratatui::prelude::*;
///
/// Toast::success("Saved").show();
/// Toast::error("Connection lost").duration(std::time::Duration::from_secs(10)).show();
/// ```
#[derive(Clone, Debug)]
pub struct Toast {
    message: String,
    tone: Tone,
    duration: Duration,
}

impl Toast {
    /// A toast in `tone` saying `message`, shown for four seconds. The tone
    /// colors its border and marker.
    pub fn new(tone: Tone, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            tone,
            duration: Duration::from_secs(4),
        }
    }
    /// Neutral information.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Tone::Info, message)
    }
    /// Something worked.
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(Tone::Success, message)
    }
    /// Something needs attention.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Tone::Warning, message)
    }
    /// Something failed.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Tone::Error, message)
    }
    /// How long it stays.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    /// Show it.
    pub fn show(self) {
        let until = Instant::now() + self.duration;
        TOASTS.with_borrow_mut(|toasts| toasts.entries.push((self, until)));
        let open = TOASTS.with_borrow(|t| t.overlay.is_some_and(Closer::is_open));
        if !open {
            let shown = overlay(Toaster)
                .placement(Placement::BottomRight)
                .passive()
                .open();
            let closer = shown.closer();
            shown.detach();
            TOASTS.with_borrow_mut(|t| t.overlay = Some(closer));
        }
        invalidate::invalidate();
    }
}

/// The toasts of this thread's loop.
#[derive(Default)]
struct Toasts {
    entries: Vec<(Toast, Instant)>,
    /// The stack's overlay; not owned, so dropping the thread-local at
    /// thread exit touches no other thread-local.
    overlay: Option<Closer>,
}

thread_local! {
    static TOASTS: RefCell<Toasts> = RefCell::default();
}

/// Widest a toast gets.
const WIDTH: u16 = 40;

/// Cells of a toast's text, with its marker.
fn message_width(toast: &Toast) -> u16 {
    super::width(&toast.message) + 2
}

/// Renders the stack of toasts.
struct Toaster;

impl Toaster {
    fn style(tone: Tone) -> Style {
        Style::new().fg(theme::current().tone(tone))
    }
    /// Height of each toast's box for an inner `width`.
    fn heights(width: u16) -> Vec<u16> {
        TOASTS.with_borrow(|t| {
            t.entries
                .iter()
                .map(|(toast, _)| (message_width(toast).div_ceil(width.max(1))).max(1) + 2)
                .collect()
        })
    }
}

impl Component for Toaster {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let now = Instant::now();
        let (entries, next) = TOASTS.with_borrow_mut(|t| {
            t.entries.retain(|(_, until)| *until > now);
            let next = t.entries.iter().map(|(_, until)| *until).min();
            (
                t.entries
                    .iter()
                    .map(|(toast, _)| toast.clone())
                    .collect::<Vec<_>>(),
                next,
            )
        });
        match next {
            Some(next) => invalidate::after(next - now),
            None => {
                if let Some(closer) = TOASTS.with_borrow_mut(|t| t.overlay.take()) {
                    closer.close();
                }
                return;
            }
        }
        let theme = theme::current();
        let heights = Self::heights(area.width.saturating_sub(4));
        // Newest at the bottom.
        let mut bottom = area.bottom();
        for (toast, height) in entries.iter().zip(heights).rev() {
            let Some(top) = bottom.checked_sub(height).filter(|top| *top >= area.y) else {
                break;
            };
            let rect = Rect::new(area.x, top, area.width, height);
            let style = Self::style(toast.tone);
            Paragraph::new(Line::from(vec![
                Span::styled("● ", style),
                Span::styled(toast.message.as_str(), theme.text()),
            ]))
            .wrap(Wrap { trim: true })
            .block(
                Block::bordered()
                    .border_type(theme.border_type)
                    .border_style(style)
                    .padding(ratatui::widgets::Padding::horizontal(1)),
            )
            .render(rect, buf);
            bottom = top;
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        let Some(position) = super::left_press(event) else {
            return Handled::No;
        };
        let Some(area) = mouse::area_of(self) else {
            return Handled::No;
        };
        // Find the clicked toast, counting from the bottom.
        let heights = Self::heights(area.width.saturating_sub(4));
        let mut bottom = area.bottom();
        let count = heights.len();
        for (i, height) in heights.into_iter().enumerate().rev() {
            let top = bottom.saturating_sub(height);
            if (top..bottom).contains(&position.y) {
                TOASTS.with_borrow_mut(|t| {
                    if i < count && i < t.entries.len() {
                        t.entries.remove(i);
                    }
                });
                invalidate::invalidate();
                return Handled::Yes;
            }
            bottom = top;
        }
        Handled::No
    }

    fn preferred_size(&self, available: Size) -> Size {
        let widest = TOASTS.with_borrow(|t| {
            t.entries
                .iter()
                .map(|(toast, _)| message_width(toast))
                .max()
                .unwrap_or(0)
        });
        let width = (widest + 4).clamp(16, WIDTH).min(available.width);
        let height: u16 = Self::heights(width.saturating_sub(4)).iter().sum();
        Size::new(width, height.min(available.height))
    }
}
