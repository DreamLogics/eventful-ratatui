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
    KeyHints(State)
}

#[doc(hidden)]
pub struct State {
    hints: RefCell<Vec<(String, String)>>,
}

impl KeyHints {
    /// No hints yet.
    pub fn new() -> Self {
        Self(Rc::new(State {
            hints: RefCell::default(),
        }))
    }
    /// Add a hint: the keys, then what they do.
    pub fn hint(self, keys: impl Into<String>, action: impl Into<String>) -> Self {
        self.0.hints.borrow_mut().push((keys.into(), action.into()));
        self
    }
    /// Hints from `BINDINGS` of a `#[key_events]` bridge. Chords bound to
    /// the same event are joined with `/`; event names have `_` replaced by
    /// spaces.
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
    /// Replace all hints.
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
            line.push_span(Span::styled(keys.clone(), theme.accent));
            line.push_span(Span::styled(format!(" {action}"), theme.muted));
        }
        line.render(area, buf);
    }
    fn preferred_size(&self, available: Size) -> Size {
        let width: u16 = self
            .hints
            .borrow()
            .iter()
            .map(|(k, a)| super::width(k) + super::width(a) + 3)
            .sum();
        Size::new(width.saturating_sub(2).min(available.width), 1)
    }
}
