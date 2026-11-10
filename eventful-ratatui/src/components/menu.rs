//! [`Menu`].
use super::choices::{Choices, Row};
use crate::overlay::{Closer, Placement, overlay};
use ratatui::text::Line;
use ratatui_core::layout::{Position, Rect};
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

/// A popup menu of actions, each carrying a value of your choice.
/// [`show`](Self::show) it and await the picked value.
///
/// Up/Down move, Enter picks, a letter jumps to the next item starting with
/// it, the mouse highlights on hover and picks on click, and Esc or a click
/// outside dismisses. Separators and disabled items are skipped. Use
/// [`show_at`](Self::show_at) for a context menu at the pointer.
///
/// The menu opens as an [`overlay`](crate::overlay) and takes focus while
/// open; focus goes back where it was when it closes. Showing needs an async
/// context on the UI shard, like a task spawned with `ShardRc::spawn_owned`:
///
/// ```no_run
/// # use eventful_ratatui::prelude::*;
/// # async fn example(anchor: Rect) {
/// #[derive(Clone)]
/// enum Edit { Cut, Copy, Paste }
///
/// let picked = Menu::new()
///     .item_with_hint(Edit::Cut, "Cut", "ctrl+x")
///     .item_with_hint(Edit::Copy, "Copy", "ctrl+c")
///     .separator()
///     .item(Edit::Paste, "Paste")
///     .show(Placement::Below(anchor))
///     .await;
/// # }
/// ```
///
/// In tests, poll the future once to open the menu, then drive it with a
/// [`Harness`](crate::testing::Harness):
///
/// ```
/// use eventful_ratatui::{prelude::*, testing::Harness};
/// use futures::FutureExt;
///
/// let mut ui = Harness::new(Label::new(""), 30, 8);
/// let mut picked = Box::pin(
///     Menu::new()
///         .item("cut", "Cut")
///         .disabled_item("copy", "Copy")
///         .item("paste", "Paste")
///         .show_at(Position::new(2, 1)),
/// );
/// assert!((&mut picked).now_or_never().is_none(), "open, waiting for a pick");
/// ui.render().key("down").key("enter");
/// assert_eq!(picked.now_or_never(), Some(Some("paste")), "Copy is skipped");
/// ```
pub struct Menu<T> {
    items: Vec<(Option<T>, Row)>,
}

impl<T: Clone + 'static> Menu<T> {
    /// An empty menu.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    /// Add an item labelled `label` that picks `value`.
    pub fn item(mut self, value: T, label: impl Into<Line<'static>>) -> Self {
        self.items.push((Some(value), Row::new(label)));
        self
    }
    /// Add an item with right-aligned hint text, like its shortcut.
    pub fn item_with_hint(
        mut self,
        value: T,
        label: impl Into<Line<'static>>,
        hint: impl Into<String>,
    ) -> Self {
        let mut row = Row::new(label);
        row.hint = hint.into();
        self.items.push((Some(value), row));
        self
    }
    /// Add an item that is shown dimmed and cannot be picked or highlighted.
    pub fn disabled_item(mut self, value: T, label: impl Into<Line<'static>>) -> Self {
        let mut row = Row::new(label);
        row.enabled = false;
        self.items.push((Some(value), row));
        self
    }
    /// Add a separator line.
    pub fn separator(mut self) -> Self {
        self.items.push((None, Row::separator()));
        self
    }
    /// Show the menu at `placement` and wait for a pick: the value of the
    /// picked item, or `None` if it was dismissed. Dropping the future
    /// closes the menu.
    ///
    /// With [`Placement::Below`], the menu is at least as wide as the
    /// anchor, like a dropdown under a button.
    pub async fn show(self, placement: Placement) -> Option<T> {
        let (values, rows): (Vec<Option<T>>, Vec<Row>) = self.items.into_iter().unzip();
        let choices = Choices::new(rows, true, true);
        let picked = Rc::new(Cell::new(None));
        let closer: Rc<RefCell<Option<Closer>>> = Rc::default();
        {
            let (picked, closer) = (picked.clone(), closer.clone());
            choices.on_pick(move |index| {
                picked.set(Some(index));
                if let Some(closer) = *closer.borrow() {
                    closer.close();
                }
            });
        }
        let shown = overlay(choices).placement(placement).open();
        *closer.borrow_mut() = Some(shown.closer());
        shown.closed().await;
        picked.get().and_then(|index| values[index].clone())
    }
    /// Show the menu with its top-left corner at `position`, like a context
    /// menu at the pointer.
    pub async fn show_at(self, position: Position) -> Option<T> {
        self.show(Placement::Below(Rect::new(position.x, position.y, 1, 0)))
            .await
    }
}

impl<T: Clone + 'static> Default for Menu<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Menu<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Menu")
            .field("items", &self.items.len())
            .finish()
    }
}
