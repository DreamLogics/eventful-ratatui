//! Tier-3 components: data, editing and layout.
use eventful_ratatui::{prelude::*, testing::Harness};

#[test]
fn sliders_step_clamp_and_follow_the_mouse() {
    let volume = Slider::new(0.0, 100.0, 5.0).with_value(50.0);
    let mut ui = Harness::new(volume.clone(), 24, 1);
    assert!(ui.lines()[0].ends_with(" 50"), "{:?}", ui.lines());
    ui.key("tab").key("right").key("right");
    assert_eq!(volume.value(), 60.0);
    ui.key("end").key("right");
    assert_eq!(volume.value(), 100.0, "clamped");
    ui.key("pagedown");
    assert_eq!(volume.value(), 50.0);
    ui.click(0, 0);
    assert_eq!(volume.value(), 0.0);
    ui.drag((0, 0), (19, 0));
    assert_eq!(volume.value(), 100.0, "the track ends before the label");
    volume.set_value(33.0);
    assert_eq!(volume.value(), 35.0, "snapped to the step");
}

#[test]
fn spin_boxes_step_by_key_wheel_and_arrows() {
    let count = SpinBox::new(0.0, 10.0, 1.0).with_value(3.0);
    let mut ui = Harness::new(count.clone(), 12, 1);
    assert_eq!(ui.lines(), ["‹  3 ›"]);
    ui.key("tab").key("up").key("up");
    assert_eq!(count.value(), 5.0);
    ui.click(0, 0);
    assert_eq!(count.value(), 4.0);
    ui.scroll(2, 0, -2);
    assert_eq!(count.value(), 6.0);
    let price = SpinBox::new(0.0, 1.0, 0.05).with_value(0.25);
    let ui = Harness::new(price, 12, 1);
    assert_eq!(ui.lines(), ["‹ 0.25 ›"], "decimals follow the step");
}

#[test]
fn fields_show_help_or_the_inputs_error() {
    let port = TextInput::new().value("80").validator(|t| {
        t.parse::<u16>()
            .map(|_| ())
            .map_err(|_| "not a port".into())
    });
    let field = Field::input("Port", port.clone()).help("1–65535");
    let mut ui = Harness::new(field.clone(), 16, 3);
    assert_eq!(ui.lines(), ["Port", "80", "1–65535"]);
    ui.key("tab").type_text("x");
    assert_eq!(ui.lines()[2], "not a port");
    assert_eq!(field.preferred_size(Size::new(20, 20)).height, 3);
}

#[test]
fn badges_chips_and_breadcrumbs() {
    let ui = Harness::new(Badge::new("3").kind(ToastKind::Error), 5, 1);
    assert_eq!(ui.lines(), [" 3"]);
    let chip = Chip::new("rust");
    let mut ui = Harness::new(chip.clone(), 10, 1);
    ui.click(1, 0);
    assert!(chip.is_selected());
    let crumbs = Breadcrumb::new()
        .item("home", "Home")
        .item("docs", "Docs")
        .item("api", "API");
    let mut ui = Harness::new(crumbs.clone(), 30, 1);
    assert_eq!(ui.lines(), ["Home › Docs › API"]);
    ui.key("tab").key("left").key("left");
    // Keys move a highlight over the clickable items; no shard runs here to
    // observe `triggered`, so check the rendering.
    assert_eq!(ui.buffer()[(0, 0)].bg, theme::current().focused.bg.unwrap());
}

#[test]
fn splits_drag_the_divider() {
    let split = Split::horizontal(Label::new("left"), Label::new("right")).ratio(0.5);
    let mut ui = Harness::new(split.clone(), 21, 2);
    assert_eq!(ui.lines()[0], "left      │right");
    ui.drag((10, 0), (5, 1));
    assert_eq!(ui.lines()[0], "left │right");
    assert!((split.get_ratio() - 0.25).abs() < 0.01);
    ui.drag((5, 0), (0, 0));
    assert_eq!(ui.lines()[0], "lef│right", "panes keep a minimum size");
}

#[test]
fn accordions_toggle_sections() {
    let accordion = Accordion::new()
        .section("General", Label::new("general settings"))
        .section("Advanced", Label::new("advanced settings"))
        .exclusive();
    let mut ui = Harness::new(accordion.clone(), 24, 4);
    assert_eq!(ui.lines()[..2], ["▸ General", "▸ Advanced"]);
    ui.key("tab").key("enter");
    assert_eq!(
        ui.lines(),
        ["▾ General", "  general settings", "", "▸ Advanced"],
        "the open section fills the spare rows"
    );
    ui.key("down").key("space");
    assert!(!accordion.is_open(0), "exclusive");
    assert!(accordion.is_open(1));
    ui.click(3, 0);
    assert!(accordion.is_open(0));
}

fn people() -> Table {
    Table::new([
        Column::new("Name", Constraint::Fill(1)),
        Column::new("Age", Constraint::Length(4)).right(),
    ])
    .sortable()
    .row(["Ada", "36"])
    .row(["Grace", "85"])
    .row(["Alan", "9"])
}

#[test]
fn tables_render_select_and_sort_naturally() {
    let table = people();
    let mut ui = Harness::new(table.clone(), 16, 4);
    assert_eq!(
        ui.lines(),
        [
            " Name        Age",
            " Ada          36",
            " Grace        85",
            " Alan          9"
        ]
    );
    ui.click(12, 0);
    assert_eq!(table.sorting(), Some((1, true)));
    assert_eq!(table.display_order(), [2, 0, 1], "9 < 36 < 85, not by text");
    assert!(ui.lines()[0].ends_with("Age▲"));
    ui.click(12, 0);
    assert_eq!(table.display_order(), [1, 0, 2]);
    ui.key("tab").key("down").key("down");
    assert_eq!(
        table.selection(),
        Some(0),
        "the second row in display order"
    );
    ui.click(2, 3);
    assert_eq!(table.selection(), Some(2));
}

fn files() -> Tree {
    Tree::new([
        TreeNode::new("src", "src")
            .expanded()
            .child(TreeNode::new("src/main.rs", "main.rs"))
            .child(TreeNode::new("src/ui", "ui").child(TreeNode::new("src/ui/mod.rs", "mod.rs"))),
        TreeNode::new("README.md", "README.md"),
    ])
}

#[test]
fn trees_expand_collapse_and_navigate() {
    let tree = files();
    let mut ui = Harness::new(tree.clone(), 20, 6);
    assert_eq!(
        ui.lines()[..4],
        ["▾ src", "    main.rs", "  ▸ ui", "  README.md"]
    );
    ui.key("tab").keys(&["down", "down", "down"]);
    assert_eq!(tree.selection().as_deref(), Some("src/ui"));
    ui.key("right");
    assert!(tree.is_expanded("src/ui"));
    ui.key("right");
    assert_eq!(tree.selection().as_deref(), Some("src/ui/mod.rs"));
    ui.key("left");
    assert_eq!(
        tree.selection().as_deref(),
        Some("src/ui"),
        "Left goes to the parent"
    );
    ui.key("left");
    assert!(!tree.is_expanded("src/ui"));
    ui.click(0, 0);
    assert!(!tree.is_expanded("src"), "a click on the arrow toggles");
    assert_eq!(ui.lines()[..2], ["▸ src", "  README.md"]);
    tree.select("src/ui/mod.rs");
    ui.render();
    assert!(
        tree.is_expanded("src") && tree.is_expanded("src/ui"),
        "select reveals"
    );
}

#[test]
fn menu_bars_open_switch_and_close() {
    let bar = MenuBar::new()
        .menu("File")
        .item("open", "Open", "ctrl+o")
        .item("quit", "Quit", "")
        .menu("Edit")
        .item("undo", "Undo", "ctrl+z");
    let mut ui = Harness::new(Stack::vertical().fit(bar.clone()), 30, 8);
    assert_eq!(ui.lines()[0], " File  Edit");
    ui.key("tab").key("enter");
    assert!(bar.is_open());
    assert!(ui.text().contains("Open"));
    ui.key("right");
    assert!(
        ui.text().contains("Undo") && !ui.text().contains("Open"),
        "{}",
        ui.text()
    );
    ui.key("esc");
    assert!(!bar.is_open());
    assert!(bar.is_focused(), "focus returns to the bar");
    ui.click(8, 0);
    assert!(ui.text().contains("Undo"));
    ui.key("enter");
    assert!(!bar.is_open(), "picking closes");
}

fn editor(text: &str) -> (TextArea, Harness<TextArea>) {
    let area = TextArea::new().value(text);
    let mut ui = Harness::new(area.clone(), 12, 3);
    ui.key("tab");
    (area, ui)
}

#[test]
fn text_areas_edit_across_lines() {
    let (area, mut ui) = editor("one\ntwo");
    ui.key("end").key("enter").type_text("1.5");
    assert_eq!(area.text(), "one\n1.5\ntwo");
    ui.key("down").key("home").key("backspace");
    assert_eq!(
        area.text(),
        "one\n1.5two",
        "Backspace at a line start joins lines"
    );
    ui.key("ctrl+home")
        .key("shift+down")
        .key("shift+right")
        .key("delete");
    assert_eq!(area.text(), ".5two");
    ui.paste("a\nb\n");
    assert_eq!(area.text(), "a\nb\n.5two");
    assert_eq!(area.cursor(), (2, 0));
}

#[test]
fn text_areas_keep_the_goal_column_and_scroll() {
    let (area, mut ui) = editor("long line here\nab\nanother long line\n4\n5\n6");
    ui.keys(&["end", "down", "down"]);
    assert_eq!(
        area.cursor(),
        (2, 14),
        "the goal column survives a short line"
    );
    assert_eq!(ui.cursor(), Some(Position::new(11, 2)));
    ui.keys(&["down", "down", "down"]);
    assert_eq!(ui.lines()[2], "6", "scrolled down: {:?}", ui.lines());
    ui.click(0, 0);
    assert_eq!(area.cursor(), (3, 0));
}

#[test]
fn text_areas_undo_and_redo() {
    let (area, mut ui) = editor("");
    ui.type_text("hello");
    ui.key("enter");
    ui.type_text("x");
    ui.key("ctrl+z");
    assert_eq!(area.text(), "hello", "the line break was its own step");
    ui.key("ctrl+z");
    assert_eq!(area.text(), "");
    ui.key("ctrl+y");
    assert_eq!(area.text(), "hello");
}

#[test]
fn text_areas_leave_tab_to_focus_unless_asked() {
    let first = TextArea::new();
    let second = TextInput::new();
    let mut ui = Harness::new(
        Stack::vertical()
            .child(Constraint::Length(2), first.clone())
            .fit(second.clone()),
        12,
        3,
    );
    ui.key("tab").key("tab");
    assert!(second.is_focused());
    let code = TextArea::new().tab_inserts();
    let mut ui = Harness::new(code.clone(), 12, 2);
    code.focus();
    ui.render().key("tab");
    assert_eq!(code.text(), "    ");
}

#[test]
fn line_numbers_take_a_gutter() {
    let area = TextArea::new().line_numbers().value("a\nb");
    let ui = Harness::new(area, 8, 2);
    assert_eq!(ui.lines(), ["1 a", "2 b"]);
}
