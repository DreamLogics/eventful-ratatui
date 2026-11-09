//! [`TextInput`].
use super::{TextEventsEmissions, TextEventsEventSet, TextEventsSignalsExt, handle};
use crate::{Component, Handled, cursor, focus, invalidate, mouse, theme};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    ops::Range,
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Checks the text after every change; `Err` holds the message to show.
type Validator = Box<dyn Fn(&str) -> Result<(), String>>;

handle! {
    /// A single-line text field.
    ///
    /// Editing keys:
    /// - Left/Right move by character, Ctrl or Alt with them by word, and
    ///   Shift extends the selection.
    /// - Home/End go to the ends.
    /// - Ctrl+A selects all.
    /// - Backspace/Delete delete a character, and with Ctrl or Alt a word;
    ///   Ctrl+W deletes the word before the cursor.
    /// - Ctrl+U deletes to the start, and Ctrl+K to the end.
    /// - Enter emits [`submitted`](super::TextEventsSignalsExt::submitted).
    /// - Pastes are inserted, with newlines turned into spaces.
    ///
    /// A click places the cursor and a drag selects. Every edit emits
    /// [`changed`](super::TextEventsSignalsExt::changed). Editing works by
    /// grapheme cluster and display width, so emoji and CJK text behave.
    TextInput(State), events = (TextEventsEventSet, TextEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    text: RefCell<String>,
    /// Byte offset of the cursor, on a grapheme boundary.
    cursor: Cell<usize>,
    /// The other end of the selection, if any.
    anchor: Cell<Option<usize>>,
    /// First visible display column.
    scroll: Cell<usize>,
    placeholder: RefCell<String>,
    mask: Cell<Option<char>>,
    max_chars: Cell<Option<usize>>,
    enabled: Cell<bool>,
    validator: RefCell<Option<Validator>>,
    error: RefCell<Option<String>>,
    events: TextEventsEmissions,
}

impl TextInput {
    /// An empty input.
    pub fn new() -> Self {
        Self(Rc::new(State {
            text: RefCell::default(),
            cursor: Cell::new(0),
            anchor: Cell::new(None),
            scroll: Cell::new(0),
            placeholder: RefCell::default(),
            mask: Cell::new(None),
            max_chars: Cell::new(None),
            enabled: Cell::new(true),
            validator: RefCell::default(),
            error: RefCell::default(),
            events: TextEventsEmissions::default(),
        }))
    }
    /// Start with `text`, cursor at the end.
    pub fn value(self, text: impl Into<String>) -> Self {
        self.set_text(text);
        self
    }
    /// Text shown while the input is empty.
    pub fn placeholder(self, placeholder: impl Into<String>) -> Self {
        *self.0.placeholder.borrow_mut() = placeholder.into();
        self
    }
    /// Show every character as the theme's mask symbol, for passwords.
    pub fn password(self) -> Self {
        self.0.mask.set(Some(theme::current().symbols.mask));
        self
    }
    /// Refuse input beyond `max` characters.
    pub fn max_chars(self, max: usize) -> Self {
        self.0.max_chars.set(Some(max));
        self
    }
    /// Check the text after every change. The error message is available
    /// through [`error`](Self::error), and the text is drawn in the error
    /// style while invalid.
    pub fn validator(self, validate: impl Fn(&str) -> Result<(), String> + 'static) -> Self {
        *self.0.validator.borrow_mut() = Some(Box::new(validate));
        self.0.validate();
        self
    }
    /// The current text.
    pub fn text(&self) -> String {
        self.0.text.borrow().clone()
    }
    /// Replace the text without emitting `changed`; the cursor goes to the
    /// end.
    pub fn set_text(&self, text: impl Into<String>) {
        let text = text.into();
        self.0.cursor.set(text.len());
        *self.0.text.borrow_mut() = text;
        self.0.anchor.set(None);
        self.0.validate();
        invalidate::invalidate();
    }
    /// Empty it without emitting `changed`.
    pub fn clear(&self) {
        self.set_text("");
    }
    /// The validator's message for the current text, if invalid.
    pub fn error(&self) -> Option<String> {
        self.0.error.borrow().clone()
    }
    /// Whether the validator accepts the current text.
    pub fn is_valid(&self) -> bool {
        self.0.error.borrow().is_none()
    }
    /// The cursor as a byte offset into the text.
    pub fn cursor(&self) -> usize {
        self.0.cursor.get()
    }
    /// The selected byte range, if anything is selected.
    pub fn selection(&self) -> Option<Range<usize>> {
        self.0.selection()
    }
    /// Select everything.
    pub fn select_all(&self) {
        self.0.anchor.set(Some(0));
        self.0.cursor.set(self.0.text.borrow().len());
        invalidate::invalidate();
    }
    /// Whether it can be edited and focused.
    pub fn is_enabled(&self) -> bool {
        self.0.enabled.get()
    }
    /// Enable or disable it.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        invalidate::invalidate();
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Character classes for word motion.
#[derive(PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(grapheme: &str) -> Class {
    match grapheme.chars().next() {
        Some(c) if c.is_whitespace() => Class::Space,
        Some(c) if c.is_alphanumeric() || c == '_' => Class::Word,
        _ => Class::Punct,
    }
}

impl State {
    fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor.get()?;
        let cursor = self.cursor.get();
        (anchor != cursor).then(|| anchor.min(cursor)..anchor.max(cursor))
    }

    /// The grapheme boundary before `at`.
    fn prev(&self, at: usize) -> usize {
        let text = self.text.borrow();
        text[..at]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    /// The grapheme boundary after `at`.
    fn next(&self, at: usize) -> usize {
        let text = self.text.borrow();
        text[at..]
            .graphemes(true)
            .next()
            .map_or(at, |g| at + g.len())
    }

    /// The start of the word before `at`.
    fn prev_word(&self, at: usize) -> usize {
        let text = self.text.borrow();
        let mut graphemes = text[..at].grapheme_indices(true).rev().peekable();
        while graphemes
            .next_if(|(_, g)| class(g) == Class::Space)
            .is_some()
        {}
        let Some((mut start, first)) = graphemes.next() else {
            return 0;
        };
        let kind = class(first);
        while let Some((i, _)) = graphemes.next_if(|(_, g)| class(g) == kind) {
            start = i;
        }
        start
    }

    /// The end of the word after `at`.
    fn next_word(&self, at: usize) -> usize {
        let text = self.text.borrow();
        let mut end = at;
        let mut graphemes = text[at..].graphemes(true).peekable();
        while let Some(g) = graphemes.next_if(|g| class(g) == Class::Space) {
            end += g.len();
        }
        let Some(first) = graphemes.next() else {
            return end;
        };
        end += first.len();
        let kind = class(first);
        while let Some(g) = graphemes.next_if(|g| class(g) == kind) {
            end += g.len();
        }
        end
    }

    /// Move the cursor, extending the selection if `extend`.
    fn move_to(&self, at: usize, extend: bool) {
        if extend {
            if self.anchor.get().is_none() {
                self.anchor.set(Some(self.cursor.get()));
            }
        } else {
            self.anchor.set(None);
        }
        self.cursor.set(at);
        invalidate::invalidate();
    }

    /// Replace `range` with `insert`, respecting the length limit, and report
    /// the change.
    fn edit(&self, range: Range<usize>, insert: &str) {
        let insert = {
            let text = self.text.borrow();
            match self.max_chars.get() {
                Some(max) => {
                    let kept = text.chars().count() - text[range.clone()].chars().count();
                    let room = max.saturating_sub(kept);
                    let end = insert
                        .char_indices()
                        .nth(room)
                        .map_or(insert.len(), |(i, _)| i);
                    insert[..end].to_owned()
                }
                None => insert.to_owned(),
            }
        };
        if range.is_empty() && insert.is_empty() {
            return;
        }
        self.text.borrow_mut().replace_range(range.clone(), &insert);
        self.cursor.set(range.start + insert.len());
        self.anchor.set(None);
        self.validate();
        self.events.changed().emit(self.text.borrow().clone());
        invalidate::invalidate();
    }

    /// Delete the selection, or `range` without one.
    fn delete(&self, range: Range<usize>) {
        self.edit(self.selection().unwrap_or(range), "");
    }

    fn insert(&self, text: &str) {
        let at = self.cursor.get();
        self.edit(self.selection().unwrap_or(at..at), text);
    }

    fn validate(&self) {
        let error = self
            .validator
            .borrow()
            .as_ref()
            .and_then(|validate| validate(&self.text.borrow()).err());
        *self.error.borrow_mut() = error;
    }

    /// Displayed graphemes: (byte offset, text, width).
    fn cells(&self) -> Vec<(usize, String, usize)> {
        let text = self.text.borrow();
        text.grapheme_indices(true)
            .map(|(i, g)| match self.mask.get() {
                Some(mask) => (i, mask.to_string(), 1),
                None => (i, g.to_owned(), g.width()),
            })
            .collect()
    }

    /// The display column of byte offset `at`.
    fn column(cells: &[(usize, String, usize)], at: usize) -> usize {
        cells
            .iter()
            .take_while(|(i, ..)| *i < at)
            .map(|(.., w)| w)
            .sum()
    }

    /// The byte offset of the cell at display `column`: a click puts the
    /// cursor before the character under it.
    fn offset_at(&self, cells: &[(usize, String, usize)], column: usize) -> usize {
        let mut x = 0;
        for (i, _, w) in cells {
            x += w;
            if column < x {
                return *i;
            }
        }
        self.text.borrow().len()
    }

    /// Handle a key; `None` if it is not an editing key.
    fn key(&self, event: &Event) -> Option<()> {
        let key = super::key(event)?;
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let word = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let at = self.cursor.get();
        let len = self.text.borrow().len();
        match key.code {
            KeyCode::Char('a') if ctrl => {
                self.anchor.set(Some(0));
                self.move_to(len, true);
            }
            KeyCode::Char('w') if ctrl => self.delete(self.prev_word(at)..at),
            KeyCode::Char('u') if ctrl => self.delete(0..at),
            KeyCode::Char('k') if ctrl => self.delete(at..len),
            KeyCode::Char(c) if !word => self.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Backspace if word => self.delete(self.prev_word(at)..at),
            KeyCode::Backspace => self.delete(self.prev(at)..at),
            KeyCode::Delete if word => self.delete(at..self.next_word(at)),
            KeyCode::Delete => self.delete(at..self.next(at)),
            KeyCode::Left => {
                let to = match (word, self.selection()) {
                    (true, _) => self.prev_word(at),
                    (false, Some(sel)) if !shift => sel.start,
                    _ => self.prev(at),
                };
                self.move_to(to, shift);
            }
            KeyCode::Right => {
                let to = match (word, self.selection()) {
                    (true, _) => self.next_word(at),
                    (false, Some(sel)) if !shift => sel.end,
                    _ => self.next(at),
                };
                self.move_to(to, shift);
            }
            KeyCode::Home => self.move_to(0, shift),
            KeyCode::End => self.move_to(len, shift),
            KeyCode::Enter => self.events.submitted().emit(self.text.borrow().clone()),
            KeyCode::Esc if self.selection().is_some() => self.move_to(at, false),
            _ => return None,
        }
        Some(())
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let focused = focus::is_focused(self);
        let width = usize::from(area.width);
        if width == 0 || area.height == 0 {
            return;
        }
        let cells = self.cells();
        if cells.is_empty() {
            Span::styled(self.placeholder.borrow().as_str(), theme.placeholder).render(area, buf);
            if focused {
                cursor::show_at(area.as_position());
            }
            return;
        }
        // Keep the cursor in view, with room for it at the end.
        let column = Self::column(&cells, self.cursor.get());
        let mut scroll = self.scroll.get();
        if column < scroll {
            scroll = column;
        } else if column >= scroll + width {
            scroll = column + 1 - width;
        }
        self.scroll.set(scroll);

        let base = if !self.enabled.get() {
            theme.disabled
        } else if self.error.borrow().is_some() {
            theme.error
        } else {
            theme.input
        };
        let selection = self.selection().filter(|_| focused);
        let mut spans = Vec::new();
        let mut x = 0;
        for (i, g, w) in cells {
            if x + w > scroll + width {
                break;
            }
            if x >= scroll {
                let selected = selection.as_ref().is_some_and(|s| s.contains(&i));
                let style: Style = if selected {
                    base.patch(theme.text_selection)
                } else {
                    base
                };
                spans.push(Span::styled(g, style));
            }
            x += w;
        }
        Line::from(spans).render(area, buf);
        if focused {
            let offset = u16::try_from(column - scroll).unwrap_or(0);
            cursor::show_at(Position::new(area.x + offset, area.y));
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }
        if let Event::Paste(text) = event {
            self.insert(&text.replace(['\r', '\n'], " "));
            return Handled::Yes;
        }
        if let Event::Mouse(m) = event {
            let area = mouse::area_of(self).unwrap_or_default();
            let column = self.scroll.get() + usize::from(m.column.saturating_sub(area.x));
            let at = self.offset_at(&self.cells(), column);
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => self.move_to(at, false),
                MouseEventKind::Drag(MouseButton::Left) => self.move_to(at, true),
                MouseEventKind::Up(MouseButton::Left) => {}
                _ => return Handled::No,
            }
            return Handled::Yes;
        }
        self.key(event).map_or(Handled::No, |()| Handled::Yes)
    }

    fn focusable(&self) -> bool {
        self.enabled.get()
    }

    fn preferred_size(&self, available: Size) -> Size {
        Size::new(available.width, 1)
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextInput")
            .field("text", &self.text.borrow())
            .finish_non_exhaustive()
    }
}
