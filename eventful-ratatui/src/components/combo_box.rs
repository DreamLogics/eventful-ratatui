//! [`ComboBox`].
use super::{
    TextEventsEmissions, TextEventsEventSet, TextEventsSignalsExt, TextInput,
    choices::{Choices, Row},
    handle,
};
use crate::{
    Component, Handled, focus, invalidate, mouse,
    overlay::{Overlay, Placement, overlay},
};
use crossterm::event::{Event, KeyCode};
use ratatui_core::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use std::{cell::RefCell, rc::Rc};

handle! {
    /// A text input with suggestions: typing filters the options in a list
    /// below it, like an autocomplete.
    ///
    /// Up/Down move through the suggestions (and open them), Enter accepts
    /// the highlighted one (the first, until moved), Esc closes the list, and a click picks one.
    /// Emits [`changed`](super::TextEventsSignalsExt::changed) while typing
    /// and [`submitted`](super::TextEventsSignalsExt::submitted) with the
    /// accepted text.
    ///
    /// Matching is case-insensitive: options starting with the text come
    /// first, then options containing it elsewhere, each in their original
    /// order. The list is an [overlay](crate::overlay) below the input and
    /// closes when focus leaves. The text is free: Enter without a
    /// highlighted suggestion submits whatever was typed.
    ///
    /// ```
    /// use eventful_ratatui::{components::ComboBox, testing::Harness};
    ///
    /// let fruit = ComboBox::new(["Apple", "Banana", "Mango", "Orange"]);
    /// let mut ui = Harness::new(fruit.clone(), 20, 6);
    ///
    /// ui.key("tab").type_text("o");
    /// assert!(fruit.is_open());
    /// assert_eq!(fruit.suggestions(), ["Orange", "Mango"]);
    ///
    /// // The best match is highlighted: Enter accepts it.
    /// ui.key("enter");
    /// assert_eq!(fruit.text(), "Orange");
    /// assert!(!fruit.is_open());
    /// ```
    ComboBox(State), events = (TextEventsEventSet, TextEventsSignalsExt)
}

#[doc(hidden)]
pub struct State {
    input: TextInput,
    options: RefCell<Vec<String>>,
    /// Indices into `options` of the suggestions shown.
    shown: RefCell<Vec<usize>>,
    choices: Rc<Choices>,
    popup: RefCell<Option<Overlay>>,
    events: TextEventsEmissions,
}

impl ComboBox {
    /// An empty combo box suggesting `options`.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(options: I) -> Self {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        let state = Rc::new(State {
            input: TextInput::new(),
            options: RefCell::new(options),
            shown: RefCell::default(),
            choices: Choices::new(Vec::new(), true, false),
            popup: RefCell::default(),
            events: TextEventsEmissions::default(),
        });
        let input = state.input.state().clone();
        let me = Rc::downgrade(&state);
        *input.intercept.borrow_mut() = Some(Rc::new({
            let me = me.clone();
            move |event: &Event| me.upgrade().map_or(Handled::No, |me| me.intercept(event))
        }));
        *input.on_change.borrow_mut() = Some(Rc::new({
            let me = me.clone();
            move |text: &str| {
                if let Some(me) = me.upgrade() {
                    me.events.changed().emit(text.to_owned());
                    me.refilter(text);
                    me.open();
                }
            }
        }));
        *input.on_submit.borrow_mut() = Some(Rc::new({
            let me = me.clone();
            move |text: &str| {
                if let Some(me) = me.upgrade() {
                    me.events.submitted().emit(text.to_owned());
                }
            }
        }));
        state.choices.on_pick(move |row| {
            if let Some(me) = me.upgrade() {
                me.accept(row);
            }
        });
        state.refilter("");
        Self(state)
    }
    /// Text shown while the input is empty.
    pub fn placeholder(self, placeholder: impl Into<String>) -> Self {
        // The builder shares the input's state; the returned handle is a
        // copy of ours.
        drop(self.0.input.clone().placeholder(placeholder));
        self
    }
    /// The current text.
    pub fn text(&self) -> String {
        self.0.input.text()
    }
    /// Replace the text without emitting events or opening the list.
    pub fn set_text(&self, text: impl Into<String>) {
        self.0.input.set_text(text);
    }
    /// Replace the options and refilter the suggestions for the current
    /// text.
    pub fn set_options<I: IntoIterator<Item = S>, S: Into<String>>(&self, options: I) {
        *self.0.options.borrow_mut() = options.into_iter().map(Into::into).collect();
        self.0.refilter(&self.0.input.text());
    }
    /// The options matching the current text, in the order they are
    /// listed; every option while the text is empty.
    pub fn suggestions(&self) -> Vec<String> {
        let options = self.0.options.borrow();
        self.0
            .shown
            .borrow()
            .iter()
            .map(|&i| options[i].clone())
            .collect()
    }
    /// Whether the suggestion list is open.
    pub fn is_open(&self) -> bool {
        self.0.is_open()
    }
}

impl State {
    fn is_open(&self) -> bool {
        self.popup.borrow().as_ref().is_some_and(Overlay::is_open)
    }

    /// Show the options matching `text`: prefix matches first, then other
    /// substring matches, case-insensitively.
    fn refilter(&self, text: &str) {
        let needle = text.to_lowercase();
        let options = self.options.borrow();
        let mut prefix = Vec::new();
        let mut inner = Vec::new();
        for (i, option) in options.iter().enumerate() {
            let lower = option.to_lowercase();
            if lower.starts_with(&needle) {
                prefix.push(i);
            } else if lower.contains(&needle) {
                inner.push(i);
            }
        }
        prefix.extend(inner);
        let rows = prefix
            .iter()
            .map(|&i| Row::new(options[i].clone()))
            .collect();
        drop(options);
        self.choices.set_rows(rows);
        let empty = prefix.is_empty();
        *self.shown.borrow_mut() = prefix;
        if empty {
            self.close();
        }
    }

    fn open(&self) {
        if self.is_open() || self.shown.borrow().is_empty() {
            return;
        }
        let Some(anchor) = mouse::area_of(self) else {
            return;
        };
        let popup = overlay(self.choices.clone())
            .placement(Placement::Below(anchor))
            .passive()
            .open();
        *self.popup.borrow_mut() = Some(popup);
    }

    fn close(&self) {
        let _closed = self.popup.borrow_mut().take();
    }

    fn accept(&self, row: usize) {
        let Some(&index) = self.shown.borrow().get(row) else {
            return;
        };
        let text = self.options.borrow()[index].clone();
        self.input.set_text(text.clone());
        self.close();
        self.events.changed().emit(text.clone());
        self.events.submitted().emit(text);
    }

    /// Keys the input sees first.
    fn intercept(&self, event: &Event) -> Handled {
        let Some(key) = super::key(event) else {
            return Handled::No;
        };
        if !key.modifiers.is_empty() {
            return Handled::No;
        }
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
                if !self.is_open() =>
            {
                self.refilter(&self.input.text());
                self.open();
                Handled::Yes
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                self.choices.navigate(event)
            }
            KeyCode::Enter if self.is_open() && self.choices.highlight().is_some() => {
                self.choices.pick_highlighted();
                Handled::Yes
            }
            KeyCode::Esc if self.is_open() => {
                self.close();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

impl Component for State {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // The list belongs to the focused input; close it when focus leaves.
        if self.is_open() && !focus::within(self) {
            self.close();
            invalidate::invalidate();
        }
        self.input.render(area, buf);
    }
    fn preferred_size(&self, available: Size) -> Size {
        Size::new(available.width, 1)
    }
}
