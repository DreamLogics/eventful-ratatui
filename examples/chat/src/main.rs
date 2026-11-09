//! A chat client: a Tokio network worker, a message list and an input box.
//! Tab or a click switches focus, the wheel scrolls; Esc or Ctrl+C quits.
//!
//! Run with `cargo run -p chat` to talk to a local bot, or
//! `cargo run -p chat -- HOST:PORT` for any line-based server (`nc -l 7000`).
use eventful_ratatui::prelude::*;
use eventful_rs::*;
use std::cell::{Cell, RefCell};
mod bot;
mod client;
use client::*;

declare_shard!(Ui, runtime = main);

/// Draw a component's border, highlighted while it has focus.
fn frame(title: &str, focused: bool) -> Block<'_> {
    let block = Block::bordered().title(format!(" {title} "));
    if focused {
        block.border_style(Style::new().cyan().bold())
    } else {
        block.border_style(Style::new().dark_gray())
    }
}

/// Scrollback of everything said. j/k scroll while focused.
#[eventful(shard = Ui)]
struct Messages {
    lines: RefCell<Vec<Line<'static>>>,
    /// Lines scrolled up from the bottom.
    scroll: Cell<usize>,
    /// Rendered height, for paging.
    height: Cell<usize>,
    ui: Invalidate,
    keys: Keymap<Messages>,
}

impl Messages {
    fn new() -> Self {
        let up = Messages::scroll_up;
        let down = Messages::scroll_down;
        Self {
            lines: RefCell::default(),
            scroll: Cell::new(0),
            height: Cell::new(0),
            ui: Invalidate::new(),
            keys: Keymap::new()
                .bind("k", move |m: &Messages| up(m, 1))
                .bind("up", move |m: &Messages| up(m, 1))
                .bind("pageup", move |m: &Messages| up(m, m.height.get()))
                .bind("g", move |m: &Messages| up(m, usize::MAX / 2))
                .bind("j", move |m: &Messages| down(m, 1))
                .bind("down", move |m: &Messages| down(m, 1))
                .bind("pagedown", move |m: &Messages| down(m, m.height.get()))
                .bind("G", move |m: &Messages| down(m, usize::MAX)),
            events: Default::default(),
        }
    }

    fn scroll_up(&self, n: usize) {
        let max = self.lines.borrow().len().saturating_sub(self.height.get());
        self.scroll.set((self.scroll.get() + n).min(max));
        self.ui.invalidate();
    }
    fn scroll_down(&self, n: usize) {
        self.scroll.set(self.scroll.get().saturating_sub(n));
        self.ui.invalidate();
    }

    fn push(&self, line: Line<'static>) {
        self.lines.borrow_mut().push(line);
        // Keep the view still while scrolled up.
        if self.scroll.get() > 0 {
            self.scroll.set(self.scroll.get() + 1);
        }
        self.ui.invalidate();
    }
}

impl ChatEvents for Messages {
    fn received(&self, line: String) {
        self.push(Line::from(line));
    }
    fn sent(&self, line: String) {
        self.push(Line::from(format!("me: {line}")).cyan());
    }
}

impl Component for Messages {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.lines.borrow();
        let title = match self.scroll.get() {
            0 => format!("Messages ({})", lines.len()),
            n => format!("Messages ({}, {n} up)", lines.len()),
        };
        let block = frame(&title, focus::is_focused(self));
        let inner = block.inner(area);
        block.render(area, buf);
        let height = usize::from(inner.height);
        self.height.set(height);
        let end = lines.len().saturating_sub(self.scroll.get());
        let start = end.saturating_sub(height);
        Paragraph::new(lines[start..end].to_vec()).render(inner, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        match event {
            InputEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_up(3),
                MouseEventKind::ScrollDown => self.scroll_down(3),
                _ => return Handled::No,
            },
            _ => return self.keys.dispatch(self, event),
        }
        Handled::Yes
    }
    fn focusable(&self) -> bool {
        true
    }
}

/// The line being typed. Enter sends it.
#[eventful(shard = Ui)]
struct Input {
    text: RefCell<String>,
    client: ShardRcHandle<Client>,
    ui: Invalidate,
}

impl Component for Input {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let text = self.text.borrow();
        let focused = focus::is_focused(self);
        let block = frame("Message", focused);
        let inner = block.inner(area);
        Paragraph::new(text.as_str()).block(block).render(area, buf);
        if focused {
            let column = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            cursor::show_at(Position::new(
                inner.x + column.min(inner.width.saturating_sub(1)),
                inner.y,
            ));
        }
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        let InputEvent::Key(key) = event else {
            return Handled::No;
        };
        if key.kind == KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Handled::No;
        }
        match key.code {
            KeyCode::Char(c) => self.text.borrow_mut().push(c),
            KeyCode::Backspace => {
                self.text.borrow_mut().pop();
            }
            KeyCode::Enter => {
                let line = self.text.take();
                if !line.is_empty() {
                    self.client.send(line);
                }
            }
            _ => return Handled::No,
        }
        self.ui.invalidate();
        Handled::Yes
    }
    fn focusable(&self) -> bool {
        true
    }
}

/// The root: lays out the children and handles what they don't. Tab and
/// clicks move the focus between them.
#[eventful(shard = Ui)]
struct App {
    messages: ShardRc<Messages>,
    input: ShardRc<Input>,
    status: RefCell<Line<'static>>,
    keys: Keymap<App>,
    ui: Invalidate,
}

impl App {
    fn new(messages: ShardRc<Messages>, input: ShardRc<Input>) -> Self {
        focus::set(&*input);
        Self {
            messages,
            input,
            status: RefCell::new(Line::from("connecting…").yellow()),
            keys: Keymap::new()
                .bind("esc", |_| quit())
                .bind("ctrl+c", |_| quit()),
            ui: Invalidate::new(),
            events: Default::default(),
        }
    }

    fn set_status(&self, status: Line<'static>) {
        *self.status.borrow_mut() = status;
        self.ui.invalidate();
    }
}

impl ChatEvents for App {
    fn connected(&self, peer: String) {
        self.set_status(Line::from(format!("connected to {peer}")).green());
    }
    fn disconnected(&self, reason: String) {
        self.set_status(Line::from(format!("disconnected: {reason}")).red());
    }
}

impl Component for App {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [messages, input, status] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(area);
        self.messages.render(messages, buf);
        self.input.render(input, buf);
        let help = Span::from("  tab, click: switch focus  esc: quit").dim();
        let mut line = self.status.borrow().clone();
        line.push_span(help);
        line.render(status, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        self.keys.dispatch(self, event)
    }
}

#[sharded_main(Ui)]
async fn main() -> eventful_ratatui::Result<()> {
    let addr = match std::env::args().nth(1) {
        Some(addr) => addr,
        None => bot::start()?.to_string(),
    };
    let client = Client::new().await?;
    let messages = Messages::bind_local(Messages::new())?;
    let input = Input::bind_local(Input {
        text: RefCell::default(),
        client: client.clone(),
        ui: Invalidate::new(),
        events: Default::default(),
    })?;
    let app = App::bind_local(App::new(messages.clone(), input))?;
    let _subscriptions = [
        client.events().role::<()>().connect(&messages).scoped(),
        client.events().role::<()>().connect(&app).scoped(),
    ];
    client.open(addr);
    Tui::new().mouse(true).run(&app).await
}
