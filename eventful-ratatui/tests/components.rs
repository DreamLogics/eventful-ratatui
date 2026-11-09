//! Library components, driven through the harness.
use eventful_ratatui::{prelude::*, testing::Harness};
use std::rc::Rc;

/// Renders its children one per row, at their preferred width.
struct Column(Vec<Rc<dyn Component>>);

impl Component for Column {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (i, child) in self.0.iter().enumerate() {
            let size = child.preferred_size(area.as_size());
            child.render(Rect::new(area.x, area.y + i as u16, size.width, 1), buf);
        }
    }
}

#[test]
fn buttons_render_focus_and_click() {
    let save = Button::new("Save").primary();
    let quit = Button::new("Quit");
    let mut ui = Harness::new(
        Column(vec![Rc::new(save.clone()), Rc::new(quit.clone())]),
        12,
        2,
    );
    assert_eq!(ui.lines(), ["[ Save ]", "[ Quit ]"]);
    ui.key("tab");
    assert!(save.is_focused());
    let focused = theme::current().focused;
    assert_eq!(ui.buffer()[(0, 0)].bg, focused.bg.unwrap());
    ui.click(2, 1);
    assert!(quit.is_focused(), "a click focuses");
    quit.set_enabled(false);
    ui.render().key("tab");
    assert!(save.is_focused(), "disabled buttons are skipped");
}

#[test]
fn checkboxes_and_switches_toggle() {
    let check = Checkbox::new("Bold");
    let switch = Switch::new("Wifi").checked(true);
    let mut ui = Harness::new(
        Column(vec![Rc::new(check.clone()), Rc::new(switch.clone())]),
        12,
        2,
    );
    assert_eq!(ui.lines(), ["[ ] Bold", "──● Wifi"]);
    ui.key("tab").key("space");
    assert!(check.is_checked());
    ui.click(0, 1);
    assert!(!switch.is_checked());
    assert_eq!(ui.lines(), ["[x] Bold", "○── Wifi"]);
}

#[test]
fn radio_groups_select_with_arrows_and_clicks() {
    let size = RadioGroup::new(["Small", "Medium", "Large"]).selected(0);
    let mut ui = Harness::new(size.clone(), 12, 3);
    assert_eq!(ui.lines(), ["(•) Small", "( ) Medium", "( ) Large"]);
    ui.key("tab").key("down").key("down").key("down");
    assert_eq!(size.selection(), Some(2), "stops at the end");
    ui.click(1, 1);
    assert_eq!(size.selection(), Some(1));
    let row = RadioGroup::new(["a", "b"]).horizontal();
    let mut ui = Harness::new(row.clone(), 12, 1);
    assert_eq!(ui.lines(), ["( ) a  ( ) b"]);
    ui.click(8, 0);
    assert_eq!(row.selection(), Some(1));
}

#[test]
fn panels_frame_a_child_and_light_up_with_focus() {
    let name = TextInput::new().placeholder("name");
    let panel = Panel::new("User").child(name.clone());
    let mut ui = Harness::new(panel, 14, 3);
    assert_eq!(
        ui.lines(),
        ["╭ User ──────╮", "│name        │", "╰────────────╯"]
    );
    let border = theme::current().border;
    assert_eq!(ui.buffer()[(0, 0)].fg, border.fg.unwrap());
    ui.key("tab");
    assert!(name.is_focused());
    assert_eq!(
        ui.buffer()[(0, 0)].fg,
        theme::current().border_focused.fg.unwrap()
    );
}

fn input(text: &str, width: u16) -> (TextInput, Harness<TextInput>) {
    let input = TextInput::new().value(text);
    let mut ui = Harness::new(input.clone(), width, 1);
    ui.key("tab");
    (input, ui)
}

#[test]
fn text_inputs_edit_by_grapheme_and_word() {
    let (input, mut ui) = input("", 20);
    ui.type_text("hello wörld");
    assert_eq!(input.text(), "hello wörld");
    ui.key("ctrl+w");
    assert_eq!(input.text(), "hello ");
    ui.type_text("👩‍👩‍👧 x").key("left").key("backspace");
    assert_eq!(
        input.text(),
        "hello 👩‍👩‍👧x",
        "the family emoji is one grapheme"
    );
    ui.key("backspace");
    assert_eq!(input.text(), "hello x");
    ui.key("home").key("ctrl+right").key("ctrl+delete");
    assert_eq!(input.text(), "hello");
    ui.key("ctrl+u");
    assert_eq!(input.text(), "");
}

#[test]
fn text_inputs_select_and_replace() {
    let (input, mut ui) = input("one two", 20);
    ui.key("ctrl+left").key("shift+end");
    assert_eq!(input.selection(), Some(4..7));
    ui.type_text("2");
    assert_eq!(input.text(), "one 2");
    ui.key("ctrl+a").key("delete");
    assert_eq!(input.text(), "");
    ui.paste("a\nb");
    assert_eq!(input.text(), "a b");
}

#[test]
fn text_inputs_scroll_and_place_the_cursor() {
    let (input, mut ui) = input("abcdefghij", 5);
    assert_eq!(ui.lines(), ["ghij"], "the cursor at the end stays visible");
    assert_eq!(ui.cursor(), Some(Position::new(4, 0)));
    ui.key("home");
    assert_eq!(ui.lines(), ["abcde"]);
    assert_eq!(ui.cursor(), Some(Position::new(0, 0)));
    ui.click(3, 0);
    assert_eq!(input.cursor(), 3);
    ui.drag((1, 0), (4, 0));
    assert_eq!(input.selection(), Some(1..4));
    let wide = TextInput::new().value("日本語");
    let mut ui = Harness::new(wide.clone(), 10, 1);
    ui.key("tab").key("left");
    assert_eq!(
        ui.cursor(),
        Some(Position::new(4, 0)),
        "wide characters take two cells"
    );
}

#[test]
fn text_inputs_mask_limit_and_validate() {
    let pin = TextInput::new().password().max_chars(4).validator(|text| {
        if text.len() == 4 {
            Ok(())
        } else {
            Err("4 digits".into())
        }
    });
    let mut ui = Harness::new(pin.clone(), 10, 1);
    assert_eq!(pin.error().as_deref(), Some("4 digits"));
    ui.key("tab").type_text("123456");
    assert_eq!(pin.text(), "1234");
    assert_eq!(ui.lines(), ["••••"]);
    assert!(pin.is_valid());
}

#[test]
fn spinners_and_indeterminate_bars_animate() {
    let spinner = Spinner::new("Loading");
    let mut ui = Harness::new(spinner.clone(), 12, 1);
    assert!(ui.lines()[0].ends_with(" Loading"));
    let before = ui.renders();
    ui.fire_timers();
    assert_eq!(ui.renders(), before + 1, "it asked for another frame");
    spinner.set_spinning(false);
    ui.render();
    assert_eq!(ui.lines(), ["Loading"]);

    let bar = ProgressBar::new();
    bar.set_value(Some(0.5));
    let ui = Harness::new(bar, 14, 1);
    assert_eq!(ui.lines(), ["━━━━━────  50%"]);
}

#[test]
fn key_hints_render_and_derive_from_bindings() {
    let hints = KeyHints::new().hint("tab", "focus").hint("esc", "quit");
    let ui = Harness::new(hints, 20, 1);
    assert_eq!(ui.lines(), ["tab focus  esc quit"]);
    let bindings = [
        ("j".parse().unwrap(), "next_item"),
        ("down".parse().unwrap(), "next_item"),
    ];
    let ui = Harness::new(KeyHints::from_bindings(&bindings), 20, 1);
    assert_eq!(ui.lines(), ["j/down next item"]);
}

mod events {
    use super::*;
    use eventful_rs::*;
    use std::cell::RefCell;

    declare_shard!(Ui, runtime = main);

    #[eventful(shard = Ui)]
    struct Form {
        name: TextInput,
        ok: Button,
        log: RefCell<Vec<String>>,
    }

    impl Form {
        fn renamed(&self, name: String) {
            self.log.borrow_mut().push(format!("name {name}"));
        }
        fn submit(&self) {
            self.log.borrow_mut().push("ok".into());
        }
    }

    impl Component for Form {
        fn render(&self, area: Rect, buf: &mut Buffer) {
            self.name
                .render(Rect::new(area.x, area.y, area.width, 1), buf);
            self.ok.render(Rect::new(area.x, area.y + 1, 6, 1), buf);
        }
    }

    #[test]
    fn component_events_reach_connected_methods() {
        Ui::shard().run_main(|| async {
            let form = Form::bind_local(Form {
                name: TextInput::new(),
                ok: Button::new("OK"),
                log: RefCell::default(),
                events: Default::default(),
            })
            .unwrap();
            let _renamed = form
                .name
                .changed()
                .with_receiver(&form)
                .connect(Form::renamed)
                .scoped();
            let _submit = form
                .ok
                .clicked()
                .with_receiver(&form)
                .connect(Form::submit)
                .scoped();
            let mut ui = Harness::new(form.clone(), 10, 2);
            ui.key("tab").type_text("ab").key("tab").key("enter");
            // Delivery is queued; let the shard run it.
            futures_timer::Delay::new(std::time::Duration::from_millis(20)).await;
            assert_eq!(*form.log.borrow(), ["name a", "name ab", "ok"]);
        });
    }
}

fn items(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("item {i}")).collect()
}

#[test]
fn lists_select_scroll_and_reveal_the_selection() {
    let list = List::new(items(10));
    let mut ui = Harness::new(list.clone(), 12, 3);
    assert_eq!(ui.lines()[0], "  item 0   │".trim_end().replace('│', "┃"));
    ui.key("tab").key("down");
    assert_eq!(
        list.selection(),
        Some(0),
        "the first Down selects the first item"
    );
    ui.keys(&["down", "down", "down"]);
    assert_eq!(list.selection(), Some(3));
    assert!(ui.lines()[2].starts_with("› item 3"), "{:?}", ui.lines());
    ui.key("end");
    assert!(ui.lines()[2].starts_with("› item 9"));
    ui.scroll(1, 1, -1);
    assert!(
        ui.lines()[0].starts_with("  item 4"),
        "the wheel scrolls the view only"
    );
    assert_eq!(list.selection(), Some(9));
    ui.click(1, 1);
    assert_eq!(list.selection(), Some(5));
}

#[test]
fn lists_search_as_you_type_and_mark_items() {
    let list = List::new(["apple", "banana", "blueberry", "cherry"]).multi_select();
    let mut ui = Harness::new(list.clone(), 20, 4);
    ui.key("tab").type_text("b");
    assert_eq!(list.selection(), Some(1));
    ui.type_text("b");
    assert_eq!(list.selection(), Some(2), "a repeated letter cycles");
    ui.key("space").key("up").key("space");
    assert_eq!(list.marked(), [1, 2]);
    assert!(ui.lines()[1].contains("[x] banana"), "{:?}", ui.lines());
    let vim = List::new(items(3)).vim_keys();
    let mut ui = Harness::new(vim.clone(), 12, 3);
    ui.key("tab").key("j").key("j").key("k");
    assert_eq!(vim.selection(), Some(0));
    ui.key("G");
    assert_eq!(vim.selection(), Some(2));
}

#[test]
fn tabs_switch_pages_by_key_and_click() {
    let name = TextInput::new().value("ada");
    let tabs = Tabs::new()
        .page("User", name.clone())
        .page("About", KeyHints::new().hint("v", "0.1"));
    let mut ui = Harness::new(tabs.clone(), 20, 2);
    assert_eq!(ui.lines(), [" User │ About", "ada"]);
    ui.key("tab");
    assert!(tabs.is_focused());
    ui.key("tab");
    assert!(name.is_focused(), "Tab goes into the page");
    ui.key("ctrl+pagedown");
    assert_eq!(tabs.selection(), 1);
    assert!(
        tabs.is_focused(),
        "focus left the hidden page for the tab row"
    );
    assert_eq!(ui.lines()[1], "v 0.1");
    ui.key("left");
    assert_eq!(tabs.selection(), 0);
    ui.click(9, 0);
    assert_eq!(tabs.selection(), 1);
}

/// `n` text inputs, one per row, as a tall form.
struct Form(Vec<TextInput>);

impl Component for Form {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (i, input) in self.0.iter().enumerate() {
            input.render(Rect::new(area.x, area.y + i as u16, area.width, 1), buf);
        }
    }
    fn preferred_size(&self, available: Size) -> Size {
        Size::new(available.width, self.0.len() as u16)
    }
}

#[test]
fn scroll_views_scroll_the_focus_into_view_and_map_input() {
    let inputs: Vec<TextInput> = (0..10)
        .map(|i| TextInput::new().value(format!("field {i}")))
        .collect();
    let view = ScrollView::new(Form(inputs.clone()));
    let mut ui = Harness::new(view.clone(), 12, 3);
    assert_eq!(
        ui.lines()[0].trim_end_matches(['┃', '│']).trim_end(),
        "field 0"
    );
    for _ in 0..6 {
        ui.key("tab");
    }
    assert!(inputs[5].is_focused());
    assert_eq!(
        view.offset(),
        3,
        "the focused field is the last visible row"
    );
    assert_eq!(
        ui.cursor(),
        Some(Position::new(7, 2)),
        "the cursor maps to the screen"
    );
    ui.click(2, 0);
    assert!(inputs[3].is_focused(), "clicks map to the scrolled content");
    ui.scroll(1, 1, 10);
    assert_eq!(view.offset(), 7, "clamped at the end");
}

#[test]
fn log_views_follow_and_hold_still_when_scrolled_back() {
    let log = LogView::new().max_lines(100);
    let mut ui = Harness::new(log.clone(), 10, 2);
    for i in 0..5 {
        log.push(format!("line {i}"));
    }
    ui.render();
    assert!(ui.lines()[1].starts_with("line 4"));
    ui.key("tab").key("up");
    assert_eq!(log.scrolled_back(), 1);
    log.push("line 5");
    ui.render();
    assert!(
        ui.lines()[1].starts_with("line 3"),
        "held still: {:?}",
        ui.lines()
    );
    ui.key("end");
    assert!(ui.lines()[1].starts_with("line 5"));
}

#[test]
fn stacks_lay_out_children_and_report_their_size() {
    let form = Stack::vertical()
        .spacing(1)
        .child(Constraint::Length(1), Label::new("Name"))
        .fit(Button::new("OK"));
    assert_eq!(form.preferred_size(Size::new(20, 20)), Size::new(6, 3));
    let row = Stack::horizontal()
        .spacing(1)
        .fit(Label::new("a"))
        .fit(Checkbox::new("b"));
    let ui = Harness::new(Stack::vertical().fit(form).fit(row), 12, 4);
    assert_eq!(ui.lines(), ["Name", "", "[ OK ]", "a [ ] b"]);
}

#[test]
fn labels_wrap_and_align() {
    let ui = Harness::new(Label::new("one two three").wrap(), 7, 3);
    assert_eq!(ui.lines(), ["one two", "three", ""]);
    let ui = Harness::new(Label::new("hi").alignment(Alignment::Right), 5, 1);
    assert_eq!(ui.lines(), ["   hi"]);
}
