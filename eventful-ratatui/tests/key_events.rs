//! `#[key_events]`: generated chords, dispatch and delivery.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use eventful_ratatui::{Component, Handled, KeyChord, Tui, key_events, quit};
use eventful_rs::*;
use futures::{StreamExt, stream};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};
use std::{cell::RefCell, io};

/// Every chord form, so the macro's parser is checked against the runtime's.
const CHORDS: [&str; 15] = [
    "j",
    "J",
    "shift+k",
    "ctrl+s",
    "alt+enter",
    "shift+tab",
    "ctrl+shift+x",
    "+",
    "ctrl++",
    "space",
    "alt+plus",
    "f5",
    "PageDown",
    "meta+super+hyper+home",
    "ESC",
];

#[key_events]
trait Every {
    #[key(
        "j",
        "J",
        "shift+k",
        "ctrl+s",
        "alt+enter",
        "shift+tab",
        "ctrl+shift+x"
    )]
    fn first(&self);
    #[key("+", "ctrl++")]
    #[key("space")]
    fn second(&self);
    #[key("alt+plus")]
    #[key("f5", "PageDown", "meta+super+hyper+home", "ESC")]
    fn third(&self);
}

#[test]
fn chords_match_the_runtime_parser() {
    let generated: Vec<KeyChord> = EveryBridge::BINDINGS.iter().map(|(c, _)| *c).collect();
    let parsed: Vec<KeyChord> = CHORDS.iter().map(|c| c.parse().unwrap()).collect();
    assert_eq!(generated, parsed);
    let names: Vec<&str> = EveryBridge::BINDINGS.iter().map(|(_, n)| *n).collect();
    let expected = [["first"; 7].as_slice(), &["second"; 3], &["third"; 5]].concat();
    assert_eq!(names, expected);
}

#[key_events]
pub trait ListKeys {
    #[key("j", "down")]
    fn next(&self);
    #[key("k", "up")]
    fn previous(&self);
    #[key("ctrl+s")]
    fn save(&self);
    #[key("q")]
    fn quit(&self) {
        quit();
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::from(code))
}

#[test]
fn dispatch_matches_presses_and_repeats() {
    let bridge = ListKeysBridge::new();
    let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_eq!(bridge.dispatch(&key(KeyCode::Down)), Handled::Yes);
    assert_eq!(bridge.dispatch(&Event::Key(ctrl_s)), Handled::Yes);
    assert_eq!(bridge.dispatch(&key(KeyCode::Char('s'))), Handled::No);
    let mut repeat = KeyEvent::from(KeyCode::Char('j'));
    repeat.kind = KeyEventKind::Repeat;
    assert_eq!(bridge.dispatch(&Event::Key(repeat)), Handled::Yes);
    let mut release = repeat;
    release.kind = KeyEventKind::Release;
    assert_eq!(bridge.dispatch(&Event::Key(release)), Handled::No);
    assert_eq!(bridge.dispatch(&Event::FocusGained), Handled::No);
}

declare_shard!(Ui, runtime = main);

#[eventful(shard = Ui)]
struct List {
    log: RefCell<Vec<&'static str>>,
    keys: ListKeysBridge,
}

impl ListKeys for List {
    fn next(&self) {
        self.log.borrow_mut().push("next");
    }
    fn previous(&self) {
        self.log.borrow_mut().push("previous");
    }
    fn save(&self) {
        self.log.borrow_mut().push("save");
    }
}

impl Component for List {
    fn render(&self, _: Rect, _: &mut Buffer) {}
    fn on_input(&self, event: &Event) -> Handled {
        self.keys.dispatch(event)
    }
}

#[test]
fn bound_events_reach_the_connected_receiver() {
    Ui::shard().run_main(|| async {
        let list = List::bind_local(List {
            log: RefCell::default(),
            keys: ListKeysBridge::new(),
            events: Default::default(),
        })
        .unwrap();
        let _keys = list.keys.connect_to(&list).scoped();
        let input = stream::iter(
            [
                key(KeyCode::Char('j')),
                key(KeyCode::Up),
                key(KeyCode::Char('x')),
                Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
                key(KeyCode::Char('q')),
            ]
            .map(io::Result::Ok),
        )
        // Keep input open: the queued `quit` event ends the loop.
        .chain(stream::pending());
        let mut terminal = Terminal::new(TestBackend::new(4, 1)).unwrap();
        Tui::new()
            .run_with(&mut terminal, input, &list)
            .await
            .unwrap();
        assert_eq!(*list.log.borrow(), ["next", "previous", "save"]);
    });
}
