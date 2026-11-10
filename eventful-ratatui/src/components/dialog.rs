//! [`Dialog`] and the [`alert`], [`confirm`] and [`prompt`] helpers.
use super::{Button, Label, TextInput};
use crate::{
    Component, Handled,
    overlay::{Closer, overlay},
    theme,
};
use crossterm::event::Event;
use ratatui::{
    text::Text,
    widgets::{Block, Padding, Widget},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{cell::Cell, fmt, rc::Rc};

/// A modal dialog: a title, a message, optional content and a row of
/// buttons. [`show`](Self::show) it and await the button the user pressed.
///
/// The dialog dims and blocks the screen below it. Tab cycles inside it,
/// Esc dismisses it, and Enter in a text input inside presses the default
/// button. Showing needs an async context on the UI shard, like a task
/// spawned with `ShardRc::spawn_owned`:
///
/// ```no_run
/// # use eventful_ratatui::prelude::*;
/// # async fn example() {
/// let choice = Dialog::new("Unsaved changes")
///     .message("Save the document before closing?")
///     .button("Cancel")
///     .destructive_button("Discard")
///     .primary_button("Save")
///     .show()
///     .await;
/// match choice {
///     Some(2) => { /* save */ }
///     Some(1) => { /* discard */ }
///     _ => { /* cancelled or dismissed */ }
/// }
/// # }
/// ```
pub struct Dialog {
    title: String,
    message: Option<Text<'static>>,
    content: Option<Rc<dyn Component>>,
    buttons: Vec<Button>,
    default: Option<usize>,
    focus: Option<Rc<dyn Fn()>>,
    width: u16,
}

impl Dialog {
    /// A dialog titled `title`, with no buttons yet.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: None,
            content: None,
            buttons: Vec::new(),
            default: None,
            focus: None,
            width: 50,
        }
    }
    /// The text above the content and buttons; wrapped.
    pub fn message(mut self, message: impl Into<Text<'static>>) -> Self {
        self.message = Some(message.into());
        self
    }
    /// A component below the message, like a form.
    pub fn content(mut self, content: impl Component + 'static) -> Self {
        self.content = Some(Rc::new(content));
        self
    }
    /// Add a plain button.
    pub fn button(self, label: impl Into<String>) -> Self {
        self.add(Button::new(label), false)
    }
    /// Add the default button: focused at first and pressed by Enter in an
    /// input.
    pub fn primary_button(self, label: impl Into<String>) -> Self {
        self.add(Button::new(label).primary(), true)
    }
    /// Add a button for a destructive action.
    pub fn destructive_button(self, label: impl Into<String>) -> Self {
        self.add(Button::new(label).destructive(), false)
    }
    /// The widest the dialog gets, in cells; 50 by default.
    pub fn max_width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }
    fn add(mut self, button: Button, default: bool) -> Self {
        if default {
            self.default = Some(self.buttons.len());
        }
        self.buttons.push(button);
        self
    }
    /// Focus `component` when the dialog opens instead of the default button.
    fn focus_first(mut self, focus: impl Fn() + 'static) -> Self {
        self.focus = Some(Rc::new(focus));
        self
    }

    /// Show the dialog and wait until a button is pressed, returning its
    /// index in the order added, or `None` if it was dismissed.
    pub async fn show(self) -> Option<usize> {
        let result = Rc::new(Cell::new(None));
        let closer: Rc<Cell<Option<Closer>>> = Rc::default();
        for (index, button) in self.buttons.iter().enumerate() {
            let (result, closer) = (result.clone(), closer.clone());
            button.on_click(move || {
                result.set(Some(index));
                if let Some(closer) = closer.get() {
                    closer.close();
                }
            });
        }
        let focus = self.focus.clone();
        let default_button = self
            .default
            .or(self.buttons.len().checked_sub(1))
            .map(|i| self.buttons[i].clone());
        let view = View {
            title: self.title,
            message: self.message.map(|m| Label::new(m).wrap().muted()),
            content: self.content,
            buttons: self.buttons,
            default: self.default,
            width: self.width,
        };
        // Open first: the overlay remembers the focus to restore.
        let shown = overlay(view).modal().open();
        closer.set(Some(shown.closer()));
        match (&focus, default_button) {
            (Some(focus), _) => focus(),
            (None, Some(button)) => button.focus(),
            (None, None) => {}
        }
        shown.closed().await;
        result.get()
    }
}

impl fmt::Debug for Dialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dialog")
            .field("title", &self.title)
            .field("buttons", &self.buttons.len())
            .finish_non_exhaustive()
    }
}

/// Cells between buttons.
const BUTTON_GAP: u16 = 1;

/// The dialog on screen.
struct View {
    title: String,
    message: Option<Label>,
    content: Option<Rc<dyn Component>>,
    buttons: Vec<Button>,
    default: Option<usize>,
    width: u16,
}

impl View {
    fn block(&self) -> Block<'static> {
        let theme = theme::current();
        Block::bordered()
            .border_type(theme.border_type)
            .border_style(theme.border(true))
            .title(format!(" {} ", self.title))
            .title_style(theme.title())
            .padding(Padding::new(2, 2, 1, 0))
    }
    fn buttons_width(&self) -> u16 {
        let widths: u16 = self
            .buttons
            .iter()
            .map(|b| b.preferred_size(Size::new(u16::MAX, 1)).width + BUTTON_GAP)
            .sum();
        widths.saturating_sub(BUTTON_GAP)
    }
    /// The heights of the message and content for an inner `width`.
    fn heights(&self, width: u16) -> (u16, u16) {
        let message = self
            .message
            .as_ref()
            .map_or(0, |m| m.preferred_size(Size::new(width, u16::MAX)).height);
        let content = self
            .content
            .as_ref()
            .map_or(0, |c| c.preferred_size(Size::new(width, u16::MAX)).height);
        (message, content)
    }
}

impl Component for View {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = self.block();
        let inner = block.inner(area);
        block.render(area, buf);
        let (message, content) = self.heights(inner.width);
        let mut y = inner.y;
        if let Some(label) = &self.message {
            label.render(Rect::new(inner.x, y, inner.width, message), buf);
            y += message + 1;
        }
        if let Some(c) = &self.content {
            c.render(
                Rect::new(
                    inner.x,
                    y,
                    inner.width,
                    content.min(inner.bottom().saturating_sub(y)),
                ),
                buf,
            );
        }
        // Buttons right-aligned on the last row.
        let row = inner.bottom().saturating_sub(1).max(inner.y);
        let mut x = inner.right().saturating_sub(self.buttons_width());
        for button in &self.buttons {
            let width = button.preferred_size(Size::new(u16::MAX, 1)).width;
            button.render(Rect::new(x, row, width, 1).intersection(area), buf);
            x += width + BUTTON_GAP;
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        // Enter that an input inside did not handle presses the default.
        if super::pressed(event, crossterm::event::KeyCode::Enter)
            && let Some(default) = self.default
            && !self.buttons.iter().any(Button::is_focused)
        {
            self.buttons[default].click();
            return Handled::Yes;
        }
        Handled::No
    }

    fn preferred_size(&self, available: Size) -> Size {
        let title = super::width(&self.title) + 6;
        let message = self
            .message
            .as_ref()
            .map_or(0, |m| m.preferred_size(Size::new(u16::MAX, 1)).width);
        let content = self
            .content
            .as_ref()
            .map_or(0, |c| c.preferred_size(Size::new(self.width - 6, 1)).width);
        let width = (message.max(content).max(self.buttons_width()) + 6)
            .max(title)
            .clamp(30, self.width)
            .min(available.width);
        let (message, content) = self.heights(width - 6);
        let gap = u16::from(message > 0);
        let gap_after_content = u16::from(content > 0);
        Size::new(
            width,
            (3 + message + gap + content + gap_after_content + 1).min(available.height),
        )
    }
}

/// Show `message` with an OK button and wait until it is dismissed.
pub async fn alert(title: impl Into<String>, message: impl Into<Text<'static>>) {
    Dialog::new(title)
        .message(message)
        .primary_button("OK")
        .show()
        .await;
}

/// Ask a yes/no question; `true` if confirmed with OK.
pub async fn confirm(title: impl Into<String>, message: impl Into<Text<'static>>) -> bool {
    Dialog::new(title)
        .message(message)
        .button("Cancel")
        .primary_button("OK")
        .show()
        .await
        == Some(1)
}

/// Ask for a line of text, starting from `initial`; `None` if cancelled.
pub async fn prompt(
    title: impl Into<String>,
    message: impl Into<Text<'static>>,
    initial: impl Into<String>,
) -> Option<String> {
    let input = TextInput::new().value(initial);
    let focus_input = input.clone();
    let dialog = Dialog::new(title)
        .message(message)
        .content(input.clone())
        .button("Cancel")
        .primary_button("OK")
        .focus_first(move || focus_input.focus());
    // Enter in the input presses OK.
    let ok = dialog.buttons[1].clone();
    *input.state().on_submit.borrow_mut() = Some(Rc::new(move |_| ok.click()));
    let choice = dialog.show().await;
    (choice == Some(1)).then(|| input.text())
}
