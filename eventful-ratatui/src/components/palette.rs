//! [`CommandPalette`].
use super::{
    TextInput,
    choices::{Choices, Row},
};
use crate::{
    Component, Handled,
    overlay::{Closer, Placement, overlay},
    theme,
};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Widget},
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

/// A searchable list of commands at the top of the screen, like the
/// Ctrl+Shift+P palette of editors. Typing filters by fuzzy match;
/// Up/Down move, Enter runs, Esc or a click outside dismisses.
///
/// ```no_run
/// # use eventful_ratatui::prelude::*;
/// # async fn example() {
/// #[derive(Clone)]
/// enum Command { Open, Save, Quit }
///
/// let picked = CommandPalette::new()
///     .command_with_hint(Command::Open, "Open file…", "ctrl+o")
///     .command_with_hint(Command::Save, "Save", "ctrl+s")
///     .command(Command::Quit, "Quit")
///     .show()
///     .await;
/// # }
/// ```
pub struct CommandPalette<T> {
    commands: Vec<(T, String, String)>,
    placeholder: String,
}

impl<T: Clone + 'static> CommandPalette<T> {
    /// An empty palette.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            placeholder: "Type a command…".into(),
        }
    }
    /// Add a command.
    pub fn command(mut self, value: T, label: impl Into<String>) -> Self {
        self.commands.push((value, label.into(), String::new()));
        self
    }
    /// Add a command with right-aligned hint text, like its shortcut.
    pub fn command_with_hint(
        mut self,
        value: T,
        label: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        self.commands.push((value, label.into(), hint.into()));
        self
    }
    /// Text in the empty search field.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Show the palette and wait for a command; `None` if dismissed.
    pub async fn show(self) -> Option<T> {
        let (values, entries): (Vec<T>, Vec<(String, String)>) = self
            .commands
            .into_iter()
            .map(|(v, l, h)| (v, (l, h)))
            .unzip();
        let view = Rc::new(View {
            input: TextInput::new().placeholder(self.placeholder),
            choices: Choices::new(Vec::new(), false, false),
            entries,
            shown: RefCell::default(),
        });
        view.refilter("");
        let picked = Rc::new(Cell::new(None));
        let closer: Rc<Cell<Option<Closer>>> = Rc::default();
        {
            let (view_weak, picked, closer) =
                (Rc::downgrade(&view), picked.clone(), closer.clone());
            view.choices.on_pick(move |row| {
                if let Some(view) = view_weak.upgrade() {
                    picked.set(view.shown.borrow().get(row).copied());
                }
                if let Some(closer) = closer.get() {
                    closer.close();
                }
            });
        }
        let input = view.input.state().clone();
        let weak = Rc::downgrade(&view);
        *input.on_change.borrow_mut() = Some(Rc::new({
            let weak = weak.clone();
            move |text: &str| {
                if let Some(view) = weak.upgrade() {
                    view.refilter(text);
                }
            }
        }));
        *input.intercept.borrow_mut() = Some(Rc::new(move |event: &Event| {
            let Some(view) = weak.upgrade() else {
                return Handled::No;
            };
            let navigation = super::key(event).is_some_and(|key| {
                key.modifiers.is_empty()
                    && matches!(
                        key.code,
                        KeyCode::Up
                            | KeyCode::Down
                            | KeyCode::PageUp
                            | KeyCode::PageDown
                            | KeyCode::Enter
                    )
            });
            if navigation {
                view.choices.navigate(event)
            } else {
                Handled::No
            }
        }));
        // Open first: the overlay remembers the focus to restore.
        let shown = overlay(view.clone())
            .placement(Placement::Top)
            .modal()
            .close_on_click_outside(true)
            .open();
        closer.set(Some(shown.closer()));
        view.input.focus();
        shown.closed().await;
        picked.get().map(|index| values[index].clone())
    }
}

impl<T: Clone + 'static> Default for CommandPalette<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for CommandPalette<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandPalette")
            .field("commands", &self.commands.len())
            .finish()
    }
}

/// The palette on screen.
struct View {
    input: TextInput,
    choices: Rc<Choices>,
    /// Labels and hints of all commands.
    entries: Vec<(String, String)>,
    /// Indices into `entries` of the rows shown, best match first.
    shown: RefCell<Vec<usize>>,
}

impl View {
    fn refilter(&self, query: &str) {
        let mut scored: Vec<(i64, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, (label, _))| fuzzy(query, label).map(|score| (score, i)))
            .collect();
        scored.sort_by_key(|&(score, i)| (std::cmp::Reverse(score), i));
        let rows = scored
            .iter()
            .map(|&(_, i)| {
                let mut row = Row::new(self.entries[i].0.clone());
                row.hint = self.entries[i].1.clone();
                row
            })
            .collect();
        *self.shown.borrow_mut() = scored.into_iter().map(|(_, i)| i).collect();
        self.choices.set_rows(rows);
    }
}

/// Score `text` against `query` as a case-insensitive subsequence; `None` if
/// it does not match. Consecutive letters and word starts score higher.
pub(crate) fn fuzzy(query: &str, text: &str) -> Option<i64> {
    let text: Vec<char> = text.chars().collect();
    let mut score = 0i64;
    let mut at = 0usize;
    let mut last: Option<usize> = None;
    for q in query
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !c.is_whitespace())
    {
        let found = (at..text.len()).find(|&i| text[i].to_lowercase().any(|t| t == q))?;
        score += 1;
        if last.is_some_and(|l| l + 1 == found) {
            score += 5;
        }
        if found == 0 || !text[found - 1].is_alphanumeric() {
            score += 3;
        }
        score -= (found - at) as i64 / 4;
        last = Some(found);
        at = found + 1;
    }
    Some(score)
}

impl Component for View {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let block = Block::bordered()
            .border_type(theme.border_type)
            .border_style(theme.border(true));
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 {
            return;
        }
        let prompt = Rect::new(inner.x, inner.y, 2.min(inner.width), 1);
        Span::styled("›", theme.accent()).render(prompt, buf);
        self.input.render(
            Rect::new(inner.x + 2, inner.y, inner.width.saturating_sub(2), 1),
            buf,
        );
        if inner.height > 1 {
            let rule = Rect::new(inner.x, inner.y + 1, inner.width, 1);
            Line::styled("─".repeat(usize::from(inner.width)), theme.scrollbar()).render(rule, buf);
        }
        if inner.height > 2 {
            let list = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 2);
            if self.choices.len() == 0 {
                Span::styled(" No matching commands", theme.muted()).render(list, buf);
            } else {
                self.choices.render(list, buf);
            }
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        let rows = (self.entries.len() as u16).clamp(1, 10);
        Size::new(60.min(available.width), (rows + 4).min(available.height))
    }
}
