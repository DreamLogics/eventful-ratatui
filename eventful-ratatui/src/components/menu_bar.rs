//! [`MenuBar`].
use super::{
    ActionEventsEmissions, ActionEventsEventSet, ActionEventsSignalsExt,
    choices::{Choices, Row},
    handle,
};
use crate::{
    Component, Handled, focus, invalidate, mouse,
    overlay::{Overlay, Placement, overlay},
    theme::{self, Fill, Interaction},
};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

handle! {
    /// A row of menus along the top: ` File  Edit  View `.
    ///
    /// The bar takes focus as one Tab stop. Left/Right move between titles;
    /// Enter, Space, Down or a click opens one; inside an open menu
    /// Left/Right switch to the neighbouring menu, and Esc closes it with
    /// focus back on the bar. Picking an item closes the menu and emits
    /// [`triggered`](super::ActionEventsSignalsExt::triggered) with its id.
    /// Hints are only shown; bind the shortcuts themselves with a
    /// [`keymap`](crate::keymap).
    ///
    /// Build it with [`menu`](Self::menu), then [`item`](Self::item)s for
    /// that menu. Give the bar one row; open menus are overlays drawn below
    /// it.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, testing::Harness};
    ///
    /// let bar = MenuBar::new()
    ///     .menu("File")
    ///     .item("open", "Open…", "ctrl+o")
    ///     .item("save", "Save", "ctrl+s")
    ///     .separator()
    ///     .item("quit", "Quit", "ctrl+q")
    ///     .menu("Help")
    ///     .item("about", "About", "");
    /// let mut ui = Harness::new(Stack::vertical().fit(bar.clone()), 30, 8);
    /// assert_eq!(ui.lines()[0], " File  Help");
    ///
    /// ui.key("tab").key("enter");
    /// assert!(bar.is_open());
    /// assert!(ui.text().contains("Open…"));
    /// ui.key("esc");
    /// assert!(!bar.is_open() && bar.is_focused());
    /// ```
    MenuBar(State), events = (ActionEventsEventSet, ActionEventsSignalsExt)
}

/// A menu: its title and items (`None` id for separators).
type Entries = Vec<(Option<String>, Row)>;

#[doc(hidden)]
pub struct State {
    menus: RefCell<Vec<(String, Entries)>>,
    highlight: Cell<usize>,
    open: RefCell<Option<(usize, Overlay)>>,
    titles: RefCell<Vec<Rect>>,
    events: ActionEventsEmissions,
    me: Weak<State>,
}

impl MenuBar {
    /// An empty bar.
    pub fn new() -> Self {
        Self(Rc::new_cyclic(|me| State {
            menus: RefCell::default(),
            highlight: Cell::new(0),
            open: RefCell::default(),
            titles: RefCell::default(),
            events: ActionEventsEmissions::default(),
            me: me.clone(),
        }))
    }
    /// Start a menu titled `title`, after the others; the
    /// [`item`](Self::item)s and [`separator`](Self::separator)s that follow
    /// go into it.
    pub fn menu(self, title: impl Into<String>) -> Self {
        self.0.menus.borrow_mut().push((title.into(), Vec::new()));
        self
    }
    /// Add an item to the last menu. Picking it emits
    /// [`triggered`](super::ActionEventsSignalsExt::triggered) with `id`.
    /// `hint` is right-aligned text like its shortcut, and may be empty.
    ///
    /// # Panics
    /// Panics if no menu was started.
    pub fn item(
        self,
        id: impl Into<String>,
        label: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        let mut row = Row::new(label.into());
        row.hint = hint.into();
        self.last_menu(|entries| entries.push((Some(id.into()), row)));
        self
    }
    /// Add a separator line to the last menu.
    ///
    /// # Panics
    /// Panics if no menu was started.
    pub fn separator(self) -> Self {
        self.last_menu(|entries| entries.push((None, Row::separator())));
        self
    }
    fn last_menu(&self, f: impl FnOnce(&mut Entries)) {
        let mut menus = self.0.menus.borrow_mut();
        let (_, entries) = menus.last_mut().expect("start a menu with `menu` first");
        f(entries);
    }
    /// Enable or disable every item with `id`. Disabled items are shown
    /// dimmed and cannot be picked. Unknown ids are ignored.
    pub fn set_enabled(&self, id: &str, enabled: bool) {
        for (_, entries) in self.0.menus.borrow_mut().iter_mut() {
            for (item, row) in entries.iter_mut() {
                if item.as_deref() == Some(id) {
                    row.enabled = enabled;
                }
            }
        }
    }
    /// Whether one of its menus is open.
    pub fn is_open(&self) -> bool {
        self.0
            .open
            .borrow()
            .as_ref()
            .is_some_and(|(_, o)| o.is_open())
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    fn open_index(&self) -> Option<usize> {
        self.open
            .borrow()
            .as_ref()
            .filter(|(_, o)| o.is_open())
            .map(|(i, _)| *i)
    }

    fn close(&self) {
        let _closed = self.open.borrow_mut().take();
    }

    fn open(&self, index: usize) {
        let count = self.menus.borrow().len();
        if count == 0 {
            return;
        }
        let index = index % count;
        // Close first, so the new menu returns focus to the bar.
        self.close();
        self.highlight.set(index);
        let anchor = self.titles.borrow().get(index).copied().unwrap_or_default();
        let (ids, rows): (Vec<Option<String>>, Vec<Row>) =
            self.menus.borrow()[index].1.iter().cloned().unzip();
        let choices = Choices::new(rows, true, true);
        let me = self.me.clone();
        choices.on_pick(move |row| {
            if let Some(me) = me.upgrade() {
                me.close();
                if let Some(Some(id)) = ids.get(row) {
                    me.events.triggered().emit(id.clone());
                }
            }
        });
        let me = self.me.clone();
        choices.on_key(move |event| {
            let Some(me) = me.upgrade() else {
                return Handled::No;
            };
            let count = me.menus.borrow().len();
            if super::pressed(event, KeyCode::Left) {
                me.open((index + count - 1) % count);
            } else if super::pressed(event, KeyCode::Right) {
                me.open((index + 1) % count);
            } else {
                return Handled::No;
            }
            Handled::Yes
        });
        let shown = overlay(choices).placement(Placement::Below(anchor)).open();
        *self.open.borrow_mut() = Some((index, shown));
        invalidate::invalidate();
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme::current();
        let focused = focus::is_focused(self);
        let pointer = mouse::hover_position(self);
        let open = self.open_index();
        let mut x = area.x;
        let mut titles = Vec::new();
        let mut line = Line::default();
        for (i, (title, _)) in self.menus.borrow().iter().enumerate() {
            let text = format!(" {title} ");
            let width = super::width(&text);
            let rect = Rect::new(x, area.y, width, 1);
            // Ghost buttons; the open menu's title stays lit.
            let state = Interaction {
                focused: focused && self.highlight.get() == i,
                hovered: open == Some(i) || pointer.is_some_and(|p| rect.contains(p)),
                disabled: false,
            };
            titles.push(rect);
            x = x.saturating_add(width);
            line.push_span(Span::styled(text, theme.fill(Fill::Ghost, state)));
        }
        line.render(area, buf);
        *self.titles.borrow_mut() = titles;
    }

    fn on_input(&self, event: &Event) -> Handled {
        let count = self.menus.borrow().len();
        if count == 0 {
            return Handled::No;
        }
        if let Some(position) = super::left_press(event) {
            let hit = self
                .titles
                .borrow()
                .iter()
                .position(|r| r.contains(position));
            return match hit {
                Some(index) => {
                    self.open(index);
                    Handled::Yes
                }
                None => Handled::No,
            };
        }
        let current = self.highlight.get();
        if super::pressed(event, KeyCode::Left) {
            self.highlight.set((current + count - 1) % count);
        } else if super::pressed(event, KeyCode::Right) {
            self.highlight.set((current + 1) % count);
        } else if super::activates(event) || super::pressed(event, KeyCode::Down) {
            self.open(current);
        } else {
            return Handled::No;
        }
        invalidate::invalidate();
        Handled::Yes
    }

    fn focusable(&self) -> bool {
        !self.menus.borrow().is_empty()
    }

    fn preferred_size(&self, available: Size) -> Size {
        let width: u16 = self
            .menus
            .borrow()
            .iter()
            .map(|(t, _)| super::width(t) + 2)
            .sum();
        Size::new(width.min(available.width), 1)
    }
}
