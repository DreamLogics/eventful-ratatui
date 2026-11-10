//! [`KeyHints`].
use super::handle;
use crate::{Component, KeyChord, invalidate, theme};
use ratatui::{
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{cell::RefCell, rc::Rc};

handle! {
    /// A status-bar strip of key hints: `tab switch  esc quit`.
    ///
    /// Build it by hand, or from a `#[key_events]` bridge's `BINDINGS`.
    /// Keys are drawn in the accent style and actions muted. It only
    /// displays the keys; bind them with [`key_events`](crate::key_events)
    /// or in [`on_input`](crate::Component::on_input).
    ///
    /// ```
    /// use eventful_ratatui::{components::KeyHints, testing::Harness};
    ///
    /// let hints = KeyHints::new().hint("tab", "switch").hint("esc", "quit");
    /// let mut ui = Harness::new(hints, 30, 1);
    /// assert_eq!(ui.lines(), [" tab  switch   esc  quit"]);
    /// ```
    KeyHints(State)
}

#[doc(hidden)]
pub struct State {
    hints: RefCell<Vec<(String, String)>>,
}

impl KeyHints {
    /// No hints yet; see also [`Default`].
    pub fn new() -> Self {
        Self(Rc::new(State {
            hints: RefCell::default(),
        }))
    }
    /// Add a hint after the others: the keys, then what they do.
    pub fn hint(self, keys: impl Into<String>, action: impl Into<String>) -> Self {
        self.0.hints.borrow_mut().push((keys.into(), action.into()));
        self
    }
    /// Hints from `BINDINGS` of a `#[key_events]` bridge, in declaration
    /// order. Chords bound to the same event are joined with `/`; event
    /// names have `_` replaced by spaces.
    ///
    /// ```
    /// use eventful_ratatui::{components::KeyHints, testing::Harness, KeyChord};
    ///
    /// let bindings: [(KeyChord, &str); 3] = [
    ///     ("j".parse().unwrap(), "move_down"),
    ///     ("down".parse().unwrap(), "move_down"),
    ///     ("q".parse().unwrap(), "quit"),
    /// ];
    /// let mut ui = Harness::new(KeyHints::from_bindings(&bindings), 30, 1);
    /// assert_eq!(ui.lines(), [" j/down  move down   q  quit"]);
    /// ```
    pub fn from_bindings(bindings: &[(KeyChord, &str)]) -> Self {
        let mut hints: Vec<(String, String)> = Vec::new();
        for (chord, event) in bindings {
            let action = event.replace('_', " ");
            match hints.iter_mut().find(|(_, a)| *a == action) {
                Some((keys, _)) => {
                    keys.push('/');
                    keys.push_str(&chord.to_string());
                }
                None => hints.push((chord.to_string(), action)),
            }
        }
        Self(Rc::new(State {
            hints: RefCell::new(hints),
        }))
    }
    /// Replace all hints, as `(keys, action)` pairs, and redraw.
    pub fn set_hints(&self, hints: impl IntoIterator<Item = (String, String)>) {
        *self.0.hints.borrow_mut() = hints.into_iter().collect();
        invalidate::invalidate();
    }
}

impl Default for KeyHints {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let mut line = Line::default();
        for (i, (keys, action)) in self.hints.borrow().iter().enumerate() {
            if i > 0 {
                line.push_span("  ");
            }
            // Keys as small chips, the action beside them.
            let chip = ratatui::style::Style::new()
                .fg(theme.palette.secondary_foreground)
                .bg(theme.palette.secondary);
            line.push_span(Span::styled(format!(" {keys} "), chip));
            line.push_span(Span::styled(format!(" {action}"), theme.muted()));
        }
        line.render(area, buf);
    }
    fn preferred_size(&self, available: Size) -> Size {
        let width: u16 = self
            .hints
            .borrow()
            .iter()
            .map(|(k, a)| super::width(k) + super::width(a) + 5)
            .sum();
        Size::new(width.saturating_sub(2).min(available.width), 1)
    }
}
