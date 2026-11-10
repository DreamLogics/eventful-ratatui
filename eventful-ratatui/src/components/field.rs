//! [`Field`].
use super::{TextInput, handle};
use crate::{Component, focus, invalidate, theme};
use ratatui::{text::Span, widgets::Widget};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{cell::RefCell, rc::Rc};

/// Supplies an error message, or `None` while valid.
type ErrorSource = Box<dyn Fn() -> Option<String>>;

handle! {
    /// A labelled form field: a label above an input, with help text or an
    /// error message below it.
    ///
    /// The label is muted, and takes the accent color while focus is inside.
    /// An error, from [`error`](Self::error) or the validator of an
    /// [`input`](Self::input), replaces the help text while there is one.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, testing::Harness};
    ///
    /// let port = TextInput::new().value("80").validator(|t| {
    ///     t.parse::<u16>().map(|_| ()).map_err(|_| "not a port".into())
    /// });
    /// let field = Field::input("Port", port).help("1–65535");
    /// let mut ui = Harness::new(field, 16, 3);
    /// assert_eq!(ui.lines(), ["Port", " 80", "1–65535"]);
    ///
    /// ui.key("tab").type_text("x");
    /// assert_eq!(ui.lines()[2], "not a port");
    /// ```
    ///
    /// The label takes one row, and the message one more if there is help
    /// text or an error source, whether or not a message shows right now, so
    /// the layout does not jump. The child gets the rows in between.
    Field(State)
}

#[doc(hidden)]
pub struct State {
    label: RefCell<String>,
    help: RefCell<String>,
    child: Rc<dyn Component>,
    error: RefCell<Option<ErrorSource>>,
}

impl Field {
    /// A field labelled `label` around any component, like a
    /// [`Select`](super::Select) or [`Slider`](super::Slider).
    pub fn new(label: impl Into<String>, child: impl Component + 'static) -> Self {
        Self(Rc::new(State {
            label: RefCell::new(label.into()),
            help: RefCell::default(),
            child: Rc::new(child),
            error: RefCell::default(),
        }))
    }
    /// A field around a text input, showing the message of its
    /// [`validator`](super::TextInput::validator) as the error.
    pub fn input(label: impl Into<String>, input: TextInput) -> Self {
        let source = input.clone();
        Self::new(label, input).error(move || source.error())
    }
    /// Text below the child while there is no error; empty for none.
    pub fn help(self, help: impl Into<String>) -> Self {
        *self.0.help.borrow_mut() = help.into();
        self
    }
    /// Where the error message comes from: `error` returns the message, or
    /// `None` while the field is valid. It is called on every render, so it
    /// can read the current state of other components. Replaces the source
    /// set by [`input`](Self::input).
    pub fn error(self, error: impl Fn() -> Option<String> + 'static) -> Self {
        *self.0.error.borrow_mut() = Some(Box::new(error));
        self
    }
    /// Change the label of a field in use, and redraw.
    pub fn set_label(&self, label: impl Into<String>) {
        *self.0.label.borrow_mut() = label.into();
        invalidate::invalidate();
    }
}

impl State {
    fn message(&self) -> Option<(String, bool)> {
        let error = self.error.borrow().as_ref().and_then(|source| source());
        match error {
            Some(error) => Some((error, true)),
            None => {
                let help = self.help.borrow();
                (!help.is_empty()).then(|| (help.clone(), false))
            }
        }
    }
    fn has_message_row(&self) -> bool {
        self.error.borrow().is_some() || !self.help.borrow().is_empty()
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        if area.height == 0 {
            return;
        }
        let label_style = if focus::within(self) {
            theme.accent()
        } else {
            theme.muted()
        };
        Span::styled(self.label.borrow().as_str(), label_style)
            .render(Rect::new(area.x, area.y, area.width, 1), buf);
        let message_rows = u16::from(self.has_message_row());
        let child_height = area.height.saturating_sub(1 + message_rows);
        self.child
            .render(Rect::new(area.x, area.y + 1, area.width, child_height), buf);
        if message_rows == 1 && area.height > 1 {
            let row = Rect::new(area.x, area.bottom() - 1, area.width, 1);
            if let Some((message, error)) = self.message() {
                Span::styled(message, if error { theme.error() } else { theme.muted() })
                    .render(row, buf);
            }
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        let child = self.child.preferred_size(Size::new(
            available.width,
            available.height.saturating_sub(2),
        ));
        let rows = 1 + child.height + u16::from(self.has_message_row());
        Size::new(
            child
                .width
                .max(super::width(&self.label.borrow()))
                .min(available.width),
            rows,
        )
    }
}
