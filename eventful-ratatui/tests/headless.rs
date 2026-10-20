//! Run components against `TestBackend` with scripted input.
use crossterm::event::{Event, KeyCode, KeyEvent};
use eventful_ratatui::{Component, Handled, Invalidate, Keymap, Tui, quit};
use eventful_rs::*;
use futures::{StreamExt, channel::mpsc, executor::block_on, stream};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, widgets::Widget};
use std::{
    cell::{Cell, RefCell},
    io,
    sync::Mutex,
};

/// `quit()` targets the current loop process-wide; keep loops from overlapping.
static SERIAL: Mutex<()> = Mutex::new(());

fn key(c: char) -> io::Result<Event> {
    Ok(Event::Key(KeyEvent::from(KeyCode::Char(c))))
}

fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(12, 2)).unwrap()
}

struct Counter {
    count: Cell<u32>,
    renders: Cell<u32>,
    ui: Invalidate,
    keys: Keymap<Counter>,
}

impl Counter {
    fn new() -> Self {
        Self {
            count: Cell::new(0),
            renders: Cell::new(0),
            ui: Invalidate::new(),
            keys: Keymap::new()
                .bind("j", |c: &Counter| {
                    c.count.set(c.count.get() + 1);
                    c.ui.invalidate();
                })
                .bind("q", |_| quit()),
        }
    }
}

impl Component for Counter {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.renders.set(self.renders.get() + 1);
        format!("count: {}", self.count.get()).render(area, buf);
    }
    fn on_input(&self, event: &Event) -> Handled {
        self.keys.dispatch(self, event)
    }
}

#[test]
fn input_updates_the_final_frame_and_redraws_coalesce() {
    let _serial = SERIAL.lock().unwrap();
    let counter = Counter::new();
    let mut terminal = terminal();
    let input = stream::iter((0..50).map(|_| key('j')));
    // A slow frame rate makes coalescing deterministic: one initial frame,
    // then one pending frame drawn when input ends.
    block_on(
        Tui::new()
            .frame_rate(1)
            .run_with(&mut terminal, input, &counter),
    )
    .unwrap();
    terminal
        .backend()
        .assert_buffer_lines(["count: 50   ", "            "]);
    assert_eq!(counter.renders.get(), 2);
}

#[test]
fn quit_returns_without_waiting_for_input() {
    let _serial = SERIAL.lock().unwrap();
    let counter = Counter::new();
    let mut terminal = terminal();
    let input = stream::iter([key('j'), key('q'), key('j')]).chain(stream::pending());
    block_on(Tui::new().run_with(&mut terminal, input, &counter)).unwrap();
    assert_eq!(counter.count.get(), 1, "input after quit is not handled");
    assert!(eventful_ratatui::Quit::current().is_none());
}

#[test]
fn input_errors_end_the_loop() {
    let _serial = SERIAL.lock().unwrap();
    let input = stream::iter([Err(io::Error::other("tty gone"))]);
    let err = block_on(Tui::new().run_with(&mut terminal(), input, &Counter::new())).unwrap_err();
    assert!(matches!(err, eventful_ratatui::Error::Io(_)), "{err:?}");
}

#[test]
fn resize_redraws() {
    let _serial = SERIAL.lock().unwrap();
    let counter = Counter::new();
    let input = stream::iter([Ok(Event::Resize(12, 2))]);
    block_on(
        Tui::new()
            .frame_rate(1)
            .run_with(&mut terminal(), input, &counter),
    )
    .unwrap();
    assert_eq!(counter.renders.get(), 2);
}

mod feed {
    use eventful_rs::*;

    declare_shard!(pub Worker, runtime = std);

    #[events]
    pub trait FeedEvents {
        fn line(&self, text: String);
    }

    #[eventful(FeedEvents, shard = Worker)]
    pub struct Feed {}

    impl Feed {
        pub async fn new() -> Result<ShardRcHandle<Self>, InvokeError> {
            Self::spawn(|| Self {
                events: Default::default(),
            })
            .await
        }
    }

    #[asynchronize(pub)]
    impl Feed {
        /// Emit each line and wait until listeners handled it.
        #[asynced]
        pub async fn send(&self, lines: Vec<String>) -> Result<(), DeliveryError> {
            for line in lines {
                self.events.line().tracked().emit(line).await?;
            }
            Ok(())
        }
    }
}
use feed::*;

declare_shard!(Ui, runtime = main);

#[eventful(shard = Ui)]
struct Log {
    lines: RefCell<Vec<String>>,
    ui: Invalidate,
}

impl FeedEvents for Log {
    fn line(&self, text: String) {
        self.lines.borrow_mut().push(text);
        self.ui.invalidate();
    }
}

impl Component for Log {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.lines.borrow().join(",").render(area, buf);
    }
}

#[test]
fn worker_events_redraw_main_shard_components() {
    let _serial = SERIAL.lock().unwrap();
    Ui::shard().run_main(|| async {
        let log = Log::bind_local(Log {
            lines: RefCell::default(),
            ui: Invalidate::new(),
            events: Default::default(),
        })
        .unwrap();
        let feed = Feed::new().await.unwrap();
        let _subscription = feed.line().connect(&log).scoped();
        let (tx, rx) = mpsc::unbounded::<io::Result<Event>>();
        let mut terminal = terminal();
        let script = async move {
            feed.send(vec!["a".into(), "b".into()]).await.unwrap();
            drop(tx); // End input; the loop draws the pending frame.
        };
        let (result, ()) = futures::join!(Tui::new().run_with(&mut terminal, rx, &log), script);
        result.unwrap();
        terminal
            .backend()
            .assert_buffer_lines(["a,b         ", "            "]);
    });
}
