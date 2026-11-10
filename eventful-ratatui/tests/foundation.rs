//! Focus, key bubbling, overlays, cursor and timers, through the harness.
use eventful_ratatui::{
    overlay::{Placement, overlay},
    prelude::*,
    testing::Harness,
    theme::{Fill, Interaction},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

type Log = Rc<RefCell<Vec<String>>>;

/// A one-line focusable field that shows `*` while focused and logs keys.
/// Handles the chord `handles`, if set.
struct Field {
    name: &'static str,
    handles: Option<&'static str>,
    log: Log,
}

fn field(name: &'static str, log: &Log) -> Rc<Field> {
    Rc::new(Field {
        name,
        handles: None,
        log: log.clone(),
    })
}

impl Component for Field {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mark = if focus::is_focused(self) { "*" } else { " " };
        format!("{mark}{}", self.name).render(area, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        let InputEvent::Key(key) = event else {
            return Handled::No;
        };
        let chord = KeyChord::from_event(key);
        self.log.borrow_mut().push(format!("{} {chord}", self.name));
        self.handles
            .is_some_and(|h| h.parse::<KeyChord>().unwrap() == chord)
            .into()
    }
    fn focusable(&self) -> bool {
        true
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(8, 1)
    }
}

/// Renders its children one per row and logs keys that bubble to it. Shows
/// `+` while focus is within it.
struct Group {
    name: &'static str,
    children: Vec<Rc<dyn Component>>,
    log: Log,
}

impl Component for Group {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mark = if focus::within(self) { "+" } else { " " };
        format!("{mark}{}", self.name).render(area, buf);
        for (i, child) in self.children.iter().enumerate() {
            let row = Rect::new(area.x + 1, area.y + 1 + i as u16, area.width - 1, 1);
            child.render(row, buf);
        }
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        if let InputEvent::Key(key) = event {
            let chord = KeyChord::from_event(key);
            self.log.borrow_mut().push(format!("{} {chord}", self.name));
        }
        Handled::No
    }
    fn preferred_size(&self, _: Size) -> Size {
        Size::new(10, 1 + self.children.len() as u16)
    }
}

fn group(name: &'static str, children: Vec<Rc<dyn Component>>, log: &Log) -> Rc<Group> {
    Rc::new(Group {
        name,
        children,
        log: log.clone(),
    })
}

/// Fields a and b in group g, and c on its own.
fn form(log: &Log) -> (Rc<Group>, [Rc<Field>; 3]) {
    let (a, b, c) = (field("a", log), field("b", log), field("c", log));
    let inner = group("g", vec![a.clone(), b.clone()], log);
    (inner, [a, b, c])
}

/// The root of the form: g at rows 0–2, c at row 3.
struct Form {
    inner: Rc<Group>,
    c: Rc<Field>,
    log: Log,
}

impl Component for Form {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.inner.render(Rect::new(0, 0, area.width, 3), buf);
        self.c.render(Rect::new(0, 3, area.width, 1), buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        if let InputEvent::Key(key) = event {
            let chord = KeyChord::from_event(key);
            self.log.borrow_mut().push(format!("root {chord}"));
        }
        Handled::No
    }
}

fn harness() -> (Harness<Form>, Log, [Rc<Field>; 3]) {
    let log = Log::default();
    let (inner, fields) = form(&log);
    let root = Form {
        inner,
        c: fields[2].clone(),
        log: log.clone(),
    };
    (Harness::new(root, 12, 6), log, fields)
}

#[test]
fn tab_moves_focus_in_render_order_and_wraps() {
    let (mut ui, _, _) = harness();
    assert_eq!(ui.lines()[..4], [" g", "  a", "  b", " c"]);
    ui.key("tab");
    assert_eq!(ui.lines()[..4], ["+g", " *a", "  b", " c"]);
    ui.key("tab").key("tab");
    assert_eq!(ui.lines()[..4], [" g", "  a", "  b", "*c"]);
    ui.key("tab");
    assert_eq!(ui.lines()[1], " *a", "Tab wraps");
    ui.key("shift+tab");
    assert_eq!(ui.lines()[3], "*c", "BackTab wraps backwards");
}

#[test]
fn keys_bubble_from_the_focused_component_to_the_root() {
    let (mut ui, log, [_, b, _]) = harness();
    focus::set(&*b);
    ui.render().key("x");
    assert_eq!(*log.borrow(), ["b x", "g x", "root x"]);
}

#[test]
fn a_handled_key_stops_bubbling_and_unhandled_tab_moves_focus() {
    let log = Log::default();
    let a = Rc::new(Field {
        name: "a",
        handles: Some("tab"),
        log: log.clone(),
    });
    let b = field("b", &log);
    let root = group("root", vec![a.clone(), b.clone()], &log);
    let mut ui = Harness::new(root, 10, 3);
    focus::set(&*a);
    ui.render().key("tab");
    assert!(focus::is_focused(&*a), "a handled Tab itself");
    focus::set(&*b);
    ui.render().key("tab");
    assert!(focus::is_focused(&*a), "unhandled Tab moved on");
    assert_eq!(*log.borrow(), ["a tab", "b tab", "root tab"]);
}

#[test]
fn without_focus_the_root_gets_keys() {
    let (mut ui, log, _) = harness();
    ui.key("x");
    assert_eq!(*log.borrow(), ["root x"]);
}

#[test]
fn a_click_focuses_the_component_under_the_pointer() {
    let (mut ui, _, [a, b, c]) = harness();
    ui.click(2, 2);
    assert!(focus::is_focused(&*b));
    ui.click(5, 3);
    assert!(focus::is_focused(&*c));
    ui.click(0, 0); // The group itself is not focusable.
    assert!(focus::is_focused(&*c));
    let _ = a;
}

#[test]
fn a_popup_takes_focus_traps_tab_and_restores_focus_on_esc() {
    let (mut ui, log, [a, _, _]) = harness();
    focus::set(&*a);
    ui.render();
    let (x, y) = (field("x", &log), field("y", &log));
    let menu = group("m", vec![x.clone(), y.clone()], &log);
    let popup = overlay(menu)
        .placement(Placement::At(Rect::new(4, 0, 8, 3)))
        .open();
    ui.render();
    assert!(focus::is_focused(&*x), "the first focusable component");
    ui.key("tab").key("tab");
    assert!(focus::is_focused(&*x), "Tab stays inside");
    log.borrow_mut().clear();
    ui.key("z");
    assert_eq!(
        *log.borrow(),
        ["x z", "m z"],
        "keys don't reach the app root"
    );
    ui.key("esc");
    assert!(!popup.is_open());
    assert!(focus::is_focused(&*a), "focus is back");
}

#[test]
fn a_click_outside_a_popup_closes_it_and_is_consumed() {
    let (mut ui, _, [_, _, c]) = harness();
    let popup = overlay(field("p", &Log::default()))
        .placement(Placement::At(Rect::new(6, 0, 6, 1)))
        .open();
    ui.render();
    ui.click(1, 3);
    assert!(!popup.is_open());
    assert!(!focus::is_focused(&*c), "the click only closed the popup");
}

#[test]
fn a_modal_dims_and_blocks_what_is_below() {
    let (mut ui, _, [_, _, c]) = harness();
    let dialog = overlay(field("d", &Log::default()))
        .placement(Placement::Center)
        .modal()
        .open();
    let screen = theme::current().palette.foreground;
    ui.render();
    // What is below blends toward the background.
    let below = &ui.buffer()[(1, 3)];
    assert_ne!(below.fg, screen, "{below:?}");
    ui.click(1, 3);
    assert!(dialog.is_open(), "a click outside does not close it");
    assert!(!focus::is_focused(&*c), "nor reach what is below");
    drop(dialog);
    ui.render();
    assert_eq!(ui.buffer()[(1, 3)].fg, screen);
}

#[test]
fn a_passive_overlay_leaves_focus_and_keys_alone() {
    let (mut ui, log, [a, _, _]) = harness();
    focus::set(&*a);
    ui.render();
    let _toast = overlay(field("t", &log))
        .placement(Placement::BottomRight)
        .passive()
        .open();
    ui.render().key("x");
    assert!(focus::is_focused(&*a));
    assert_eq!(log.borrow()[0], "a x");
    assert!(ui.lines()[4].ends_with(" t"), "{:?}", ui.lines());
}

#[test]
fn closed_resolves_when_the_overlay_closes() {
    let (mut ui, _, _) = harness();
    let popup = overlay(field("p", &Log::default())).open();
    let closed = popup.closed();
    ui.render().key("esc");
    futures::executor::block_on(closed);
    assert!(!popup.is_open());
}

#[test]
fn components_place_the_cursor() {
    struct Caret;
    impl Component for Caret {
        fn render(&self, area: Rect, _: &mut Buffer) {
            cursor::show_at(Position::new(area.x + 2, area.y));
        }
    }
    let ui = Harness::new(Caret, 5, 1);
    assert_eq!(ui.cursor(), Some(Position::new(2, 0)));
}

#[test]
fn timers_request_later_frames() {
    struct Clock(std::cell::Cell<u32>);
    impl Component for Clock {
        fn render(&self, area: Rect, buf: &mut Buffer) {
            self.0.set(self.0.get() + 1);
            invalidate::after(Duration::from_secs(3600));
            self.0.get().to_string().render(area, buf);
        }
    }
    let mut ui = Harness::new(Clock(std::cell::Cell::new(0)), 3, 1);
    assert_eq!(ui.lines(), ["1"]);
    ui.fire_timers();
    assert_eq!(ui.lines(), ["2"]);
}

#[test]
fn timers_wake_the_loop() {
    struct Once(std::cell::Cell<u32>);
    impl Component for Once {
        fn render(&self, _: Rect, _: &mut Buffer) {
            self.0.set(self.0.get() + 1);
            match self.0.get() {
                1 => invalidate::after(Duration::from_millis(20)),
                _ => quit(),
            }
        }
    }
    let root = Once(std::cell::Cell::new(0));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(3, 1)).unwrap();
    let input = futures::stream::pending();
    futures::executor::block_on(Tui::new().run_with(&mut terminal, input, &root)).unwrap();
    assert_eq!(root.0.get(), 2);
}

#[test]
fn hover_follows_the_pointer_and_redraws() {
    let ok = Button::new("OK");
    let cancel = Button::new("Cancel");
    let mut ui = Harness::new(
        Stack::horizontal()
            .spacing(1)
            .fit(ok.clone())
            .fit(cancel.clone()),
        20,
        1,
    );
    let theme = theme::current();
    let rest = theme.fill(Fill::Secondary, Interaction::default());
    let hot = theme.fill(Fill::Secondary, Interaction::default().hovered(true));
    assert_eq!(ui.buffer()[(0, 0)].bg, rest.bg.unwrap());
    let before = ui.renders();
    ui.hover(1, 0);
    assert!(ui.renders() > before, "a hover change redraws");
    assert!(ok.is_hovered(), "the button under the pointer");
    assert_eq!(ui.buffer()[(0, 0)].bg, hot.bg.unwrap());
    let before = ui.renders();
    ui.hover(2, 0);
    assert_eq!(ui.renders(), before, "moving within it changes nothing");
    ui.hover(8, 0);
    assert_eq!(ui.buffer()[(0, 0)].bg, rest.bg.unwrap());
    assert_eq!(ui.buffer()[(8, 0)].bg, hot.bg.unwrap());
}

#[test]
fn modals_hide_what_is_below_from_the_pointer() {
    let below = Button::new("Below");
    let mut ui = Harness::new(Stack::vertical().fit(below.clone()), 30, 8);
    ui.hover(1, 0);
    assert!(below.is_hovered());
    let _dialog = overlay(Label::new("modal"))
        .placement(Placement::Center)
        .modal()
        .open();
    ui.render();
    assert!(!below.is_hovered(), "the modal covers it");
}
