//! Mouse routing against `TestBackend` with scripted input.
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use eventful_ratatui::{Component, FocusChain, Handled, Tui, mouse};
use futures::{executor::block_on, stream};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::{cell::RefCell, io, rc::Rc, sync::Mutex};

/// `quit()` targets the current loop process-wide; keep loops from overlapping.
static SERIAL: Mutex<()> = Mutex::new(());

type Log = Rc<RefCell<Vec<String>>>;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> io::Result<Event> {
    Ok(Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }))
}

const DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
const DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
const UP: MouseEventKind = MouseEventKind::Up(MouseButton::Left);

fn short(kind: MouseEventKind) -> &'static str {
    match kind {
        MouseEventKind::Down(_) => "down",
        MouseEventKind::Drag(_) => "drag",
        MouseEventKind::Up(_) => "up",
        MouseEventKind::Moved => "move",
        _ => "scroll",
    }
}

/// Logs the mouse events it is offered, with their position relative to its
/// own area, and handles them if `handles` is set. Renders `child`, if any,
/// in its lower half.
struct Pane {
    name: &'static str,
    handles: bool,
    child: Option<Rc<Pane>>,
    log: Log,
}

impl Pane {
    fn new(name: &'static str, handles: bool, log: &Log) -> Rc<Self> {
        Self::with_child(name, handles, None, log)
    }
    fn with_child(
        name: &'static str,
        handles: bool,
        child: Option<Rc<Pane>>,
        log: &Log,
    ) -> Rc<Self> {
        Rc::new(Self {
            name,
            handles,
            child,
            log: log.clone(),
        })
    }
}

impl Component for Pane {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if let Some(child) = &self.child {
            let half = area.height / 2;
            let lower = Rect::new(area.x, area.y + half, area.width, area.height - half);
            child.render(lower, buf);
        }
    }
    fn on_input(&self, event: &Event) -> Handled {
        let Event::Mouse(event) = event else {
            self.log.borrow_mut().push(format!("{} key", self.name));
            return Handled::No;
        };
        let area = mouse::area_of(self).expect("hit-tested components have an area");
        self.log.borrow_mut().push(format!(
            "{} {} {},{}",
            self.name,
            short(event.kind),
            i32::from(event.column) - i32::from(area.x),
            i32::from(event.row) - i32::from(area.y),
        ));
        self.handles.into()
    }
}

/// A plain root: two panes side by side on a 20x4 screen, and an optional
/// popup over the middle of the top row.
struct Split {
    left: Rc<Pane>,
    right: Rc<Pane>,
    popup: Option<Rc<Pane>>,
    focus: FocusChain,
    log: Log,
}

impl Split {
    fn new(log: &Log, left: Rc<Pane>, right: Rc<Pane>) -> Self {
        Self {
            left,
            right,
            popup: None,
            focus: FocusChain::new(),
            log: log.clone(),
        }
    }
}

impl Component for Split {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let half = area.width / 2;
        self.left
            .render(Rect::new(area.x, area.y, half, area.height), buf);
        self.right
            .render(Rect::new(area.x + half, area.y, half, area.height), buf);
        if let Some(popup) = &self.popup {
            popup.render(Rect::new(5, 0, 10, 1), buf);
        }
    }
    fn on_input(&self, event: &Event) -> Handled {
        if self.focus.dispatch(event) == Handled::Yes {
            return Handled::Yes;
        }
        if let Event::Mouse(event) = event {
            self.log
                .borrow_mut()
                .push(format!("root {}", short(event.kind)));
        }
        Handled::No
    }
}

fn run<C: Component + 'static>(root: &C, input: Vec<io::Result<Event>>, mouse: bool) {
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    block_on(
        Tui::new()
            .mouse(mouse)
            .run_with(&mut terminal, stream::iter(input), root),
    )
    .unwrap();
}

fn take(log: &Log) -> Vec<String> {
    log.take()
}

#[test]
fn events_go_to_the_component_under_the_pointer() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let split = Split::new(&log, Pane::new("l", true, &log), Pane::new("r", true, &log));
    run(
        &split,
        vec![
            mouse(DOWN, 1, 1),
            mouse(UP, 1, 1),
            mouse(MouseEventKind::Moved, 12, 3),
        ],
        true,
    );
    assert_eq!(take(&log), ["l down 1,1", "l up 1,1", "r move 2,3"]);
}

#[test]
fn the_topmost_component_comes_first() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let mut split = Split::new(&log, Pane::new("l", true, &log), Pane::new("r", true, &log));
    split.popup = Some(Pane::new("p", false, &log));
    run(&split, vec![mouse(DOWN, 6, 0), mouse(DOWN, 6, 1)], true);
    // The popup ignores the press. Overlapped siblings are not its parents,
    // so the plain root gets it next.
    assert_eq!(take(&log), ["p down 1,0", "root down", "l down 6,1"]);
}

#[test]
fn unhandled_events_bubble_to_parents_then_the_plain_root() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let inner = Pane::new("inner", false, &log);
    let left = Pane::with_child("l", false, Some(inner), &log);
    let split = Split::new(&log, left, Pane::new("r", true, &log));
    run(&split, vec![mouse(DOWN, 3, 3), mouse(DOWN, 3, 0)], true);
    assert_eq!(
        take(&log),
        [
            "inner down 3,1",
            "l down 3,3",
            "root down",
            "l down 3,0",
            "root down"
        ]
    );
}

#[test]
fn a_hit_tested_root_is_offered_events_once() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let root = Pane::with_child("root", false, Some(Pane::new("child", false, &log)), &log);
    run(&root, vec![mouse(DOWN, 0, 3)], true);
    assert_eq!(take(&log), ["child down 0,1", "root down 0,3"]);
}

#[test]
fn a_press_captures_drags_and_the_release() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let inner = Pane::new("inner", false, &log);
    let left = Pane::with_child("l", true, Some(inner), &log);
    let split = Split::new(&log, left, Pane::new("r", true, &log));
    run(
        &split,
        vec![
            mouse(DOWN, 1, 3),
            mouse(DRAG, 15, 0),
            mouse(UP, 15, 0),
            mouse(DRAG, 15, 0),
        ],
        true,
    );
    // The handler and its parents hold the capture; the component below it
    // that declined the press does not.
    assert_eq!(
        take(&log),
        [
            "inner down 1,1",
            "l down 1,3",
            "l drag 15,0",
            "l up 15,0",
            "r drag 5,0"
        ]
    );
}

#[test]
fn focus_chains_do_not_route_mouse_events() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let split = Split::new(
        &log,
        Pane::new("l", false, &log),
        Pane::new("r", false, &log),
    );
    split.focus.focus([&split.right]);
    run(
        &split,
        vec![
            mouse(DOWN, 1, 1),
            Ok(Event::Key(KeyEvent::from(KeyCode::Char('x')))),
        ],
        true,
    );
    assert_eq!(take(&log), ["l down 1,1", "root down", "r key"]);
}

#[test]
fn without_routing_the_root_gets_mouse_events() {
    let _serial = SERIAL.lock().unwrap();
    let log = Log::default();
    let split = Split::new(&log, Pane::new("l", true, &log), Pane::new("r", true, &log));
    run(&split, vec![mouse(DOWN, 1, 1)], false);
    assert_eq!(take(&log), ["root down"]);
    assert_eq!(mouse::area_of(&*split.left), None);
}
