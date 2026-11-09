//! Tier-2 components: dialogs, menus, selects, combo boxes, toasts and the
//! command palette.
use eventful_ratatui::{prelude::*, testing::Harness};
use futures::FutureExt;
use std::{future::Future, pin::Pin};

/// A screen with one button, to have something below the overlays.
fn screen() -> (Button, Harness<Stack>) {
    let button = Button::new("Below");
    let ui = Harness::new(Stack::vertical().fit(button.clone()), 40, 12);
    (button, ui)
}

/// Poll a pending UI future once.
fn poll<T>(future: &mut Pin<Box<dyn Future<Output = T>>>) -> Option<T> {
    future.now_or_never()
}

#[test]
fn confirm_defaults_to_ok_and_esc_cancels() {
    let (below, mut ui) = screen();
    below.focus();
    ui.render();
    let mut answer: Pin<Box<dyn Future<Output = bool>>> =
        Box::pin(confirm("Delete", "Delete 3 files?"));
    assert!(poll(&mut answer).is_none());
    ui.render();
    let text = ui.text();
    assert!(
        text.contains("Delete 3 files?") && text.contains("[ OK ]"),
        "{text}"
    );
    ui.key("enter");
    assert_eq!(poll(&mut answer), Some(true));
    assert!(below.is_focused(), "focus came back");

    let mut answer: Pin<Box<dyn Future<Output = bool>>> = Box::pin(confirm("Delete", "Sure?"));
    poll(&mut answer);
    ui.render().key("esc");
    assert_eq!(poll(&mut answer), Some(false));

    let mut answer: Pin<Box<dyn Future<Output = bool>>> = Box::pin(confirm("Delete", "Sure?"));
    poll(&mut answer);
    ui.render().key("shift+tab").key("enter");
    assert_eq!(poll(&mut answer), Some(false), "Cancel was pressed");
}

#[test]
fn dialogs_block_the_screen_below() {
    let (below, mut ui) = screen();
    let mut choice: Pin<Box<dyn Future<Output = Option<usize>>>> = Box::pin(
        Dialog::new("Hi")
            .message("Text")
            .button("A")
            .primary_button("B")
            .show(),
    );
    poll(&mut choice);
    ui.render().click(2, 0);
    assert!(!below.is_focused());
    assert!(poll(&mut choice).is_none(), "still open");
    // Click the first button, A.
    let row = ui.lines().iter().position(|l| l.contains("[ A ]")).unwrap() as u16;
    let column = ui.lines()[row as usize].find("[ A ]").unwrap() as u16;
    ui.click(column + 1, row);
    assert_eq!(poll(&mut choice), Some(Some(0)));
}

#[test]
fn prompt_returns_the_entered_text() {
    let (_, mut ui) = screen();
    let mut name: Pin<Box<dyn Future<Output = Option<String>>>> =
        Box::pin(prompt("Rename", "New name:", "draft"));
    poll(&mut name);
    ui.render().key("ctrl+w").type_text("final").key("enter");
    assert_eq!(poll(&mut name), Some(Some("final".into())));
}

#[derive(Clone, Debug, PartialEq)]
enum Edit {
    Cut,
    Copy,
    Paste,
}

fn edit_menu() -> Menu<Edit> {
    Menu::new()
        .item_with_hint(Edit::Cut, "Cut", "ctrl+x")
        .disabled_item(Edit::Copy, "Copy")
        .separator()
        .item(Edit::Paste, "Paste")
}

#[test]
fn menus_pick_by_key_skip_disabled_items_and_dismiss() {
    let (_, mut ui) = screen();
    let mut picked: Pin<Box<dyn Future<Output = Option<Edit>>>> =
        Box::pin(edit_menu().show(Placement::Below(Rect::new(2, 1, 1, 0))));
    poll(&mut picked);
    ui.render();
    assert!(
        ui.lines()[2].contains("Cut") && ui.lines()[2].contains("ctrl+x"),
        "{}",
        ui.text()
    );
    ui.key("down").key("enter");
    assert_eq!(
        poll(&mut picked),
        Some(Some(Edit::Paste)),
        "Copy and the separator are skipped"
    );

    let mut picked: Pin<Box<dyn Future<Output = Option<Edit>>>> =
        Box::pin(edit_menu().show_at(Position::new(5, 5)));
    poll(&mut picked);
    ui.render().click(39, 11);
    assert_eq!(poll(&mut picked), Some(None), "a click outside dismisses");

    let mut picked: Pin<Box<dyn Future<Output = Option<Edit>>>> =
        Box::pin(edit_menu().show_at(Position::new(5, 5)));
    poll(&mut picked);
    ui.render().type_text("p").key("enter");
    assert_eq!(poll(&mut picked), Some(Some(Edit::Paste)), "type-ahead");
}

#[test]
fn menus_pick_by_mouse() {
    let (_, mut ui) = screen();
    let mut picked: Pin<Box<dyn Future<Output = Option<Edit>>>> =
        Box::pin(edit_menu().show_at(Position::new(5, 5)));
    poll(&mut picked);
    ui.render();
    let paste = ui.lines().iter().position(|l| l.contains("Paste")).unwrap() as u16;
    ui.mouse(MouseEventKind::Moved, 8, paste);
    ui.click(8, paste);
    assert_eq!(poll(&mut picked), Some(Some(Edit::Paste)));
}

#[test]
fn selects_open_a_list_and_change_value() {
    let size = Select::new(["Small", "Medium", "Large"]).selected(0);
    let mut ui = Harness::new(
        Stack::vertical().child(Constraint::Length(1), size.clone()),
        20,
        8,
    );
    assert!(ui.lines()[0].starts_with(" Small"));
    ui.key("tab").key("down");
    assert_eq!(
        size.value().as_deref(),
        Some("Medium"),
        "Down changes the value while closed"
    );
    ui.key("enter");
    assert!(size.is_open());
    ui.key("down").key("enter");
    assert!(!size.is_open());
    assert_eq!(size.value().as_deref(), Some("Large"));
    assert!(size.is_focused(), "focus returns to the select");
    ui.click(3, 0);
    assert!(size.is_open());
    ui.key("esc");
    assert!(!size.is_open());
}

#[test]
fn combo_boxes_suggest_and_accept() {
    let fruit = ComboBox::new(["Apple", "Banana", "Blueberry", "Cherry"]);
    let mut ui = Harness::new(
        Stack::vertical().child(Constraint::Length(1), fruit.clone()),
        20,
        8,
    );
    ui.key("tab").type_text("b");
    assert!(fruit.is_open());
    assert_eq!(fruit.suggestions(), ["Banana", "Blueberry"]);
    ui.key("down").key("enter");
    assert_eq!(
        fruit.text(),
        "Blueberry",
        "Down moved past the first suggestion"
    );
    assert!(!fruit.is_open());
    ui.key("ctrl+u").type_text("x");
    assert!(!fruit.is_open(), "no matches, no list");
    ui.key("backspace").key("down");
    assert!(fruit.is_open(), "Down opens the full list");
    ui.key("esc");
    assert!(!fruit.is_open());
}

#[test]
fn combo_boxes_list_prefix_matches_first() {
    let fruit = ComboBox::new(["Pineapple", "Apple"]);
    let mut ui = Harness::new(
        Stack::vertical().child(Constraint::Length(1), fruit.clone()),
        20,
        8,
    );
    ui.key("tab").type_text("ap");
    assert_eq!(fruit.suggestions(), ["Apple", "Pineapple"]);
}

#[test]
fn toasts_stack_and_close_on_click() {
    let (_, mut ui) = screen();
    Toast::info("first").show();
    Toast::error("second").show();
    ui.render();
    let text = ui.text();
    let (first, second) = (text.find("first").unwrap(), text.find("second").unwrap());
    assert!(first < second, "newest at the bottom:\n{text}");
    let row = ui
        .lines()
        .iter()
        .position(|l| l.contains("second"))
        .unwrap() as u16;
    ui.click(38, row);
    assert!(!ui.text().contains("second"));
    assert!(ui.text().contains("first"));
}

#[test]
fn the_command_palette_filters_fuzzily() {
    let (below, mut ui) = screen();
    below.focus();
    ui.render();
    let mut picked: Pin<Box<dyn Future<Output = Option<&str>>>> = Box::pin(
        CommandPalette::new()
            .command("open", "Open file")
            .command_with_hint("save", "Save", "ctrl+s")
            .command("save-as", "Save as…")
            .command("quit", "Quit")
            .show(),
    );
    poll(&mut picked);
    ui.render().type_text("sa");
    assert!(ui.text().contains("Save"), "{}", ui.text());
    assert!(!ui.text().contains("Quit"));
    ui.key("down").key("enter");
    assert_eq!(poll(&mut picked), Some(Some("save-as")));

    let mut picked: Pin<Box<dyn Future<Output = Option<&str>>>> =
        Box::pin(CommandPalette::new().command("open", "Open file").show());
    poll(&mut picked);
    ui.render().type_text("ofl").key("enter");
    assert_eq!(
        poll(&mut picked),
        Some(Some("open")),
        "a subsequence matches"
    );
    assert!(below.is_focused(), "focus came back");
}
