//! [`TextArea`].
use super::{TextEventsEmissions, TextEventsEventSet, TextEventsSignalsExt, handle};
use crate::{
    Component, Handled, cursor, focus, invalidate, mouse,
    theme::{self, Interaction},
};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A place in the text: line and byte offset in it.
type Pos = (usize, usize);

/// Edits within this interval join one undo step.
const COALESCE: Duration = Duration::from_millis(800);
/// Undo steps kept.
const HISTORY: usize = 200;

handle! {
    /// A multi-line text editor.
    ///
    /// Keys:
    /// - Editing works as in [`TextInput`](super::TextInput); in addition,
    ///   Up/Down move between lines and PageUp/PageDown by a screen.
    /// - Ctrl+Home/End go to the start or end of the text.
    /// - Enter inserts a line break.
    /// - Ctrl+Z undoes; Ctrl+Y or Ctrl+Shift+Z redoes.
    /// - Tab moves the focus on, unless [`tab_inserts`](Self::tab_inserts)
    ///   is set.
    ///
    /// The mouse places the cursor, drags select, and the wheel scrolls.
    /// Long lines scroll horizontally. Every edit emits
    /// [`changed`](super::TextEventsSignalsExt::changed) with the full text.
    TextArea(State), events = (TextEventsEventSet, TextEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    lines: RefCell<Vec<String>>,
    cursor: Cell<Pos>,
    anchor: Cell<Option<Pos>>,
    /// First visible line and display column.
    scroll: Cell<(usize, usize)>,
    /// Display column kept while moving vertically.
    goal: Cell<Option<usize>>,
    placeholder: RefCell<String>,
    read_only: Cell<bool>,
    tab_inserts: Cell<bool>,
    line_numbers: Cell<bool>,
    /// Text height at the last render, for paging.
    height: Cell<usize>,
    undo: RefCell<Vec<(Vec<String>, Pos)>>,
    redo: RefCell<Vec<(Vec<String>, Pos)>>,
    last_edit: Cell<Option<Instant>>,
    events: TextEventsEmissions,
}

impl TextArea {
    /// An empty editor.
    pub fn new() -> Self {
        Self(Rc::new(State {
            lines: RefCell::new(vec![String::new()]),
            cursor: Cell::new((0, 0)),
            anchor: Cell::new(None),
            scroll: Cell::new((0, 0)),
            goal: Cell::new(None),
            placeholder: RefCell::default(),
            read_only: Cell::new(false),
            tab_inserts: Cell::new(false),
            line_numbers: Cell::new(false),
            height: Cell::new(0),
            undo: RefCell::default(),
            redo: RefCell::default(),
            last_edit: Cell::new(None),
            events: TextEventsEmissions::default(),
        }))
    }
    /// Start with `text`.
    pub fn value(self, text: &str) -> Self {
        self.set_text(text);
        self
    }
    /// Text shown while empty.
    pub fn placeholder(self, placeholder: impl Into<String>) -> Self {
        *self.0.placeholder.borrow_mut() = placeholder.into();
        self
    }
    /// Allow moving and selecting, but no edits.
    pub fn read_only(self) -> Self {
        self.0.read_only.set(true);
        self
    }
    /// Make Tab insert four spaces instead of moving the focus.
    pub fn tab_inserts(self) -> Self {
        self.0.tab_inserts.set(true);
        self
    }
    /// Show line numbers in a gutter.
    pub fn line_numbers(self) -> Self {
        self.0.line_numbers.set(true);
        self
    }
    /// The text, lines joined with `\n`.
    pub fn text(&self) -> String {
        self.0.lines.borrow().join("\n")
    }
    /// Replace the text without emitting `changed`; clears the undo history.
    pub fn set_text(&self, text: &str) {
        *self.0.lines.borrow_mut() = text
            .split('\n')
            .map(|l| l.trim_end_matches('\r').to_owned())
            .collect();
        self.0.cursor.set((0, 0));
        self.0.anchor.set(None);
        self.0.scroll.set((0, 0));
        self.0.undo.borrow_mut().clear();
        self.0.redo.borrow_mut().clear();
        invalidate::invalidate();
    }
    /// The cursor: line and byte offset in it.
    pub fn cursor(&self) -> (usize, usize) {
        self.0.cursor.get()
    }
    /// The selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.0.selection()?;
        Some(self.0.slice(start, end))
    }
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

/// Character classes for word motion.
fn class(g: &str) -> u8 {
    match g.chars().next() {
        Some(c) if c.is_whitespace() => 0,
        Some(c) if c.is_alphanumeric() || c == '_' => 1,
        _ => 2,
    }
}

fn prev_grapheme(line: &str, at: usize) -> usize {
    line[..at]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(i, _)| i)
}

fn next_grapheme(line: &str, at: usize) -> usize {
    line[at..]
        .graphemes(true)
        .next()
        .map_or(at, |g| at + g.len())
}

fn prev_word(line: &str, at: usize) -> usize {
    let mut it = line[..at].grapheme_indices(true).rev().peekable();
    while it.next_if(|(_, g)| class(g) == 0).is_some() {}
    let Some((mut start, first)) = it.next() else {
        return 0;
    };
    let kind = class(first);
    while let Some((i, _)) = it.next_if(|(_, g)| class(g) == kind) {
        start = i;
    }
    start
}

fn next_word(line: &str, at: usize) -> usize {
    let mut end = at;
    let mut it = line[at..].graphemes(true).peekable();
    while let Some(g) = it.next_if(|g| class(g) == 0) {
        end += g.len();
    }
    let Some(first) = it.next() else {
        return end;
    };
    end += first.len();
    let kind = class(first);
    while let Some(g) = it.next_if(|g| class(g) == kind) {
        end += g.len();
    }
    end
}

/// Display column of byte offset `at` in `line`.
fn column(line: &str, at: usize) -> usize {
    line[..at].width()
}

/// Byte offset at display `column` in `line` (before the cell under it).
fn offset_at(line: &str, column: usize) -> usize {
    let mut x = 0;
    for (i, g) in line.grapheme_indices(true) {
        x += g.width();
        if column < x {
            return i;
        }
    }
    line.len()
}

impl State {
    fn selection(&self) -> Option<(Pos, Pos)> {
        let anchor = self.anchor.get()?;
        let cursor = self.cursor.get();
        (anchor != cursor).then(|| (anchor.min(cursor), anchor.max(cursor)))
    }

    fn slice(&self, start: Pos, end: Pos) -> String {
        let lines = self.lines.borrow();
        if start.0 == end.0 {
            return lines[start.0][start.1..end.1].to_owned();
        }
        let mut out = lines[start.0][start.1..].to_owned();
        for line in &lines[start.0 + 1..end.0] {
            out.push('\n');
            out.push_str(line);
        }
        out.push('\n');
        out.push_str(&lines[end.0][..end.1]);
        out
    }

    fn move_to(&self, to: Pos, extend: bool, keep_goal: bool) {
        if extend {
            if self.anchor.get().is_none() {
                self.anchor.set(Some(self.cursor.get()));
            }
        } else {
            self.anchor.set(None);
        }
        if !keep_goal {
            self.goal.set(None);
        }
        self.cursor.set(to);
        invalidate::invalidate();
    }

    /// Save an undo step, joining quick successive edits.
    fn snapshot(&self, coalesce: bool) {
        let now = Instant::now();
        let recent = self.last_edit.get().is_some_and(|at| now - at < COALESCE);
        self.last_edit.set(Some(now));
        self.redo.borrow_mut().clear();
        if coalesce && recent {
            return;
        }
        let mut undo = self.undo.borrow_mut();
        undo.push((self.lines.borrow().clone(), self.cursor.get()));
        if undo.len() > HISTORY {
            undo.remove(0);
        }
    }

    /// Replace `start..end` with `text`, which may contain line breaks.
    fn replace(&self, start: Pos, end: Pos, text: &str, coalesce: bool) {
        if self.read_only.get() {
            return;
        }
        self.snapshot(coalesce);
        {
            let mut lines = self.lines.borrow_mut();
            let tail = lines[end.0][end.1..].to_owned();
            let head = lines[start.0][..start.1].to_owned();
            let mut new: Vec<String> = text.split('\n').map(str::to_owned).collect();
            let last = new.len() - 1;
            let cursor = (
                start.0 + last,
                if last == 0 { head.len() } else { 0 } + new[last].len(),
            );
            new[0].insert_str(0, &head);
            new[last].push_str(&tail);
            lines.splice(start.0..=end.0, new);
            self.cursor.set(cursor);
        }
        self.anchor.set(None);
        self.goal.set(None);
        self.events.changed().emit(self.lines.borrow().join("\n"));
        invalidate::invalidate();
    }

    fn insert(&self, text: &str) {
        let at = self.cursor.get();
        let (start, end) = self.selection().unwrap_or((at, at));
        self.replace(start, end, text, !text.contains('\n') && start == end);
    }

    /// Delete the selection, or `start..end` without one.
    fn delete(&self, start: Pos, end: Pos) {
        let (start, end) = self.selection().unwrap_or((start.min(end), start.max(end)));
        if start != end {
            self.replace(start, end, "", false);
        }
    }

    fn restore(
        &self,
        from: &RefCell<Vec<(Vec<String>, Pos)>>,
        to: &RefCell<Vec<(Vec<String>, Pos)>>,
    ) {
        let Some((lines, cursor)) = from.borrow_mut().pop() else {
            return;
        };
        to.borrow_mut()
            .push((self.lines.replace(lines), self.cursor.get()));
        self.cursor.set(cursor);
        self.anchor.set(None);
        self.last_edit.set(None);
        self.events.changed().emit(self.lines.borrow().join("\n"));
        invalidate::invalidate();
    }

    fn before(&self, (row, col): Pos) -> Pos {
        if col > 0 {
            (row, prev_grapheme(&self.lines.borrow()[row], col))
        } else if row > 0 {
            (row - 1, self.lines.borrow()[row - 1].len())
        } else {
            (0, 0)
        }
    }

    fn after(&self, (row, col): Pos) -> Pos {
        let lines = self.lines.borrow();
        if col < lines[row].len() {
            (row, next_grapheme(&lines[row], col))
        } else if row + 1 < lines.len() {
            (row + 1, 0)
        } else {
            (row, col)
        }
    }

    fn word_before(&self, (row, col): Pos) -> Pos {
        if col == 0 {
            return self.before((row, col));
        }
        (row, prev_word(&self.lines.borrow()[row], col))
    }

    fn word_after(&self, (row, col): Pos) -> Pos {
        let len = self.lines.borrow()[row].len();
        if col == len {
            return self.after((row, col));
        }
        (row, next_word(&self.lines.borrow()[row], col))
    }

    /// The cursor moved `rows` lines up or down, keeping the goal column.
    fn vertical(&self, rows: isize) -> Pos {
        let (row, col) = self.cursor.get();
        let lines = self.lines.borrow();
        let goal = self.goal.get().unwrap_or_else(|| column(&lines[row], col));
        self.goal.set(Some(goal));
        let target = row.saturating_add_signed(rows).min(lines.len() - 1);
        (target, offset_at(&lines[target], goal))
    }

    fn key(&self, event: &Event) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let word = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let at = self.cursor.get();
        let line_len = self.lines.borrow()[at.0].len();
        let last = {
            let lines = self.lines.borrow();
            (lines.len() - 1, lines[lines.len() - 1].len())
        };
        let page = self.height.get().max(1) as isize;
        match key.code {
            KeyCode::Char('z') if ctrl && shift => self.restore(&self.redo, &self.undo),
            KeyCode::Char('Z') if ctrl => self.restore(&self.redo, &self.undo),
            KeyCode::Char('z') if ctrl => self.restore(&self.undo, &self.redo),
            KeyCode::Char('y') if ctrl => self.restore(&self.redo, &self.undo),
            KeyCode::Char('a') if ctrl => {
                self.anchor.set(Some((0, 0)));
                self.move_to(last, true, false);
            }
            KeyCode::Char('w') if ctrl => self.delete(self.word_before(at), at),
            KeyCode::Char('u') if ctrl => self.delete((at.0, 0), at),
            KeyCode::Char('k') if ctrl => self.delete(at, (at.0, line_len)),
            KeyCode::Char(c) if !word => self.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Enter if !word => self.insert("\n"),
            KeyCode::Tab if self.tab_inserts.get() && key.modifiers.is_empty() => {
                self.insert("    ")
            }
            KeyCode::Backspace if word => self.delete(self.word_before(at), at),
            KeyCode::Backspace => self.delete(self.before(at), at),
            KeyCode::Delete if word => self.delete(at, self.word_after(at)),
            KeyCode::Delete => self.delete(at, self.after(at)),
            KeyCode::Left => {
                let to = match (word, self.selection()) {
                    (true, _) => self.word_before(at),
                    (false, Some((start, _))) if !shift => start,
                    _ => self.before(at),
                };
                self.move_to(to, shift, false);
            }
            KeyCode::Right => {
                let to = match (word, self.selection()) {
                    (true, _) => self.word_after(at),
                    (false, Some((_, end))) if !shift => end,
                    _ => self.after(at),
                };
                self.move_to(to, shift, false);
            }
            KeyCode::Up => self.move_to(self.vertical(-1), shift, true),
            KeyCode::Down => self.move_to(self.vertical(1), shift, true),
            KeyCode::PageUp if !ctrl => self.move_to(self.vertical(-page), shift, true),
            KeyCode::PageDown if !ctrl => self.move_to(self.vertical(page), shift, true),
            KeyCode::Home if ctrl => self.move_to((0, 0), shift, false),
            KeyCode::End if ctrl => self.move_to(last, shift, false),
            KeyCode::Home => self.move_to((at.0, 0), shift, false),
            KeyCode::End => self.move_to((at.0, line_len), shift, false),
            KeyCode::Esc if self.selection().is_some() => self.move_to(at, false, false),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    /// Columns of the line-number gutter.
    fn gutter(&self) -> u16 {
        if self.line_numbers.get() {
            self.lines.borrow().len().to_string().len() as u16 + 1
        } else {
            0
        }
    }

    /// The text position at a screen position.
    fn position_at(&self, area: Rect, x: u16, y: u16) -> Pos {
        let (top, left) = self.scroll.get();
        let lines = self.lines.borrow();
        let row = (top + usize::from(y.saturating_sub(area.y))).min(lines.len() - 1);
        let column = left + usize::from(x.saturating_sub(area.x + self.gutter()));
        (row, offset_at(&lines[row], column))
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let state = Interaction::of(self).hovered(false);
        buf.set_style(area, theme.well(state));
        let area = super::pad(area);
        let height = usize::from(area.height);
        self.height.set(height);
        let gutter = self.gutter();
        let width = usize::from(area.width.saturating_sub(gutter));
        if width == 0 || height == 0 {
            return;
        }
        let focused = focus::is_focused(self);
        let lines = self.lines.borrow();
        let (row, col) = self.cursor.get();
        let cursor_x = column(&lines[row], col);
        // Keep the cursor visible.
        let (mut top, mut left) = self.scroll.get();
        top = top.min(lines.len() - 1);
        if row < top {
            top = row;
        } else if row >= top + height {
            top = row + 1 - height;
        }
        if cursor_x < width {
            left = 0;
        } else if cursor_x < left {
            left = cursor_x;
        } else if cursor_x >= left + width {
            left = cursor_x + 1 - width;
        }
        self.scroll.set((top, left));

        if lines.len() == 1 && lines[0].is_empty() {
            let text = Rect::new(area.x + gutter, area.y, area.width - gutter, 1);
            Span::styled(self.placeholder.borrow().as_str(), theme.placeholder()).render(text, buf);
        }
        let selection = self.selection().filter(|_| focused);
        for (y, index) in (top..lines.len().min(top + height)).enumerate() {
            let y = area.y + y as u16;
            if gutter > 0 {
                let number = format!("{:>w$} ", index + 1, w = usize::from(gutter - 1));
                let style = if index == row {
                    theme.accent()
                } else {
                    theme.muted()
                };
                Span::styled(number, style).render(Rect::new(area.x, y, gutter, 1), buf);
            }
            let line = &lines[index];
            let mut spans = Vec::new();
            let mut x = 0;
            for (i, g) in line.grapheme_indices(true) {
                let w = g.width();
                if x + w > left + width {
                    break;
                }
                if x >= left {
                    let selected =
                        selection.is_some_and(|(s, e)| (index, i) >= s && (index, i) < e);
                    let style: Style = if selected {
                        theme.text().patch(theme.selection())
                    } else {
                        theme.text()
                    };
                    spans.push(Span::styled(g.to_owned(), style));
                }
                x += w;
            }
            Line::from(spans).render(Rect::new(area.x + gutter, y, area.width - gutter, 1), buf);
        }
        if focused {
            let x = area.x + gutter + (cursor_x - left) as u16;
            let y = area.y + (row - top) as u16;
            cursor::show_at(Position::new(x, y));
        }
    }

    fn on_input(&self, event: &Event) -> Handled {
        if let Event::Paste(text) = event {
            self.insert(&text.replace("\r\n", "\n").replace('\r', "\n"));
            return Handled::Yes;
        }
        if let Event::Mouse(m) = event {
            if let Some(lines) = super::wheel(event) {
                let (top, left) = self.scroll.get();
                let max = self.lines.borrow().len().saturating_sub(1);
                self.scroll
                    .set((top.saturating_add_signed(lines as isize * 3).min(max), left));
                // Keep the view where the wheel put it: move the cursor along.
                let (row, _) = self.cursor.get();
                let (top, _) = self.scroll.get();
                let height = self.height.get().max(1);
                if row < top || row >= top + height {
                    let target = row.clamp(top, top + height - 1).min(max);
                    self.move_to((target, 0), false, false);
                }
                invalidate::invalidate();
                return Handled::Yes;
            }
            let area = super::pad(mouse::area_of(self).unwrap_or_default());
            let at = self.position_at(area, m.column, m.row);
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => self.move_to(at, false, false),
                MouseEventKind::Drag(MouseButton::Left) => self.move_to(at, true, false),
                MouseEventKind::Up(MouseButton::Left) => {}
                _ => return Handled::No,
            }
            return Handled::Yes;
        }
        self.key(event)
    }

    fn focusable(&self) -> bool {
        true
    }
}
