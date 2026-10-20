//! A chat client: a Tokio network worker, a message list and an input box.
//! Tab switches focus; Esc or Ctrl+C quits.
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
    focused: Cell<bool>,
    ui: Invalidate,
    keys: Keymap<Messages>,
}

impl Messages {
    fn new() -> Self {
        let up = |m: &Messages, n: usize| {
            let max = m.lines.borrow().len().saturating_sub(m.height.get());
            m.scroll.set((m.scroll.get() + n).min(max));
            m.ui.invalidate();
        };
        let down = |m: &Messages, n: usize| {
            m.scroll.set(m.scroll.get().saturating_sub(n));
            m.ui.invalidate();
        };
        Self {
            lines: RefCell::default(),
            scroll: Cell::new(0),
            height: Cell::new(0),
            focused: Cell::new(false),
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
        let block = frame(&title, self.focused.get());
        let inner = block.inner(area);
        block.render(area, buf);
        let height = usize::from(inner.height);
        self.height.set(height);
        let end = lines.len().saturating_sub(self.scroll.get());
        let start = end.saturating_sub(height);
        Paragraph::new(lines[start..end].to_vec()).render(inner, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        self.keys.dispatch(self, event)
    }
}

/// The line being typed. Enter sends it.
#[eventful(shard = Ui)]
struct Input {
    text: RefCell<String>,
    focused: Cell<bool>,
    client: ShardRcHandle<Client>,
    ui: Invalidate,
}

impl Component for Input {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let text = self.text.borrow();
        let cursor = if self.focused.get() { "▏" } else { "" };
        Paragraph::new(format!("{text}{cursor}"))
            .block(frame("Message", self.focused.get()))
            .render(area, buf);
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
}

/// The root: lays out the children, owns focus and handles what they don't.
#[eventful(shard = Ui)]
struct App {
    messages: ShardRc<Messages>,
    input: ShardRc<Input>,
    status: RefCell<Line<'static>>,
    focus: FocusChain,
    keys: Keymap<App>,
    ui: Invalidate,
}

impl App {
    fn new(messages: ShardRc<Messages>, input: ShardRc<Input>) -> Self {
        let app = Self {
            messages,
            input,
            status: RefCell::new(Line::from("connecting…").yellow()),
            focus: FocusChain::new(),
            keys: Keymap::new()
                .bind("tab", App::focus_next)
                .bind("backtab", App::focus_next)
                .bind("esc", |_| quit())
                .bind("ctrl+c", |_| quit()),
            ui: Invalidate::new(),
            events: Default::default(),
        };
        app.focus.focus([&app.input]);
        app.input.focused.set(true);
        app
    }

    /// Toggle focus between the input box and the message list.
    fn focus_next(&self) {
        let to_input = !self.focus.is_focused(&*self.input);
        if to_input {
            self.focus.focus([&self.input]);
        } else {
            self.focus.focus([&self.messages]);
        }
        self.input.focused.set(to_input);
        self.messages.focused.set(!to_input);
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
        let help = Span::from("  tab: switch focus  esc: quit").dim();
        let mut line = self.status.borrow().clone();
        line.push_span(help);
        line.render(status, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        match self.focus.dispatch(event) {
            Handled::No => self.keys.dispatch(self, event),
            handled => handled,
        }
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
        focused: Cell::new(false),
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
    run_tui(&app).await
}
