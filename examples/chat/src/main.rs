//! A chat client: a Tokio network worker, a message log and an input box,
//! built from library components. Tab or a click moves the focus, the wheel
//! and arrows scroll the log; Esc or Ctrl+C quits.
//!
//! Run with `cargo run -p chat` to talk to a local bot, or
//! `cargo run -p chat -- HOST:PORT` for any line-based server (`nc -l 7000`).
use eventful_ratatui::prelude::*;
use eventful_rs::*;
mod bot;
mod client;
use client::*;

declare_shard!(Ui, runtime = main);

/// The whole screen: the log, the input and a status line.
#[eventful(shard = Ui)]
struct App {
    messages: LogView,
    log_panel: Panel,
    input: TextInput,
    input_panel: Panel,
    status: Label,
    hints: KeyHints,
    client: ShardRcHandle<Client>,
    keys: Keymap<App>,
}

impl App {
    fn new(client: ShardRcHandle<Client>) -> Self {
        let messages = LogView::new().wrap();
        let input = TextInput::new().placeholder("Say something; /help for commands");
        Self {
            log_panel: Panel::new("Messages").child(messages.clone()),
            input_panel: Panel::new("Message").child(input.clone()),
            messages,
            input,
            status: Label::new(Line::from("connecting…").yellow()),
            hints: KeyHints::new().hint("tab", "focus").hint("esc", "quit"),
            client,
            keys: Keymap::new()
                .bind("esc", |_| quit())
                .bind("ctrl+c", |_| quit()),
            events: Default::default(),
        }
    }

    fn push(&self, line: Line<'static>) {
        self.messages.push(line);
        self.log_panel
            .set_title(format!("Messages ({})", self.messages.len()));
    }

    /// Enter in the input box.
    fn submit(&self, line: String) {
        if !line.is_empty() {
            self.client.send(line);
            self.input.clear();
        }
    }
}

impl ChatEvents for App {
    fn connected(&self, peer: String) {
        self.status
            .set_text(Line::from(format!("connected to {peer}")).green());
    }
    fn received(&self, line: String) {
        self.push(Line::from(line));
    }
    fn sent(&self, line: String) {
        self.push(Line::from(format!("me: {line}")).cyan());
    }
    fn disconnected(&self, reason: String) {
        self.status
            .set_text(Line::from(format!("disconnected: {reason}")).red());
    }
}

impl Component for App {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [log, input, status] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(area);
        self.log_panel.render(log, buf);
        self.input_panel.render(input, buf);
        let width = self.hints.preferred_size(status.as_size()).width;
        let [status, hints] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(status);
        self.status.render(status, buf);
        self.hints.render(hints, buf);
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
    let app = App::bind_local(App::new(client.clone()))?;
    let _network = client.events().role::<()>().connect(&app).scoped();
    let _submit = app
        .input
        .submitted()
        .with_receiver(&app)
        .connect(App::submit)
        .scoped();
    app.input.focus();
    client.open(addr);
    Tui::new().mouse(true).run(&app).await
}
