//! Every component on one screen. Run with `cargo run -p gallery`.
//!
//! Tab moves the focus, Ctrl+PageUp/PageDown switch pages, Ctrl+P opens the
//! command palette, the mouse works everywhere, and Ctrl+C quits. What you do
//! is logged on the Lists page.
use eventful_ratatui::prelude::*;
use eventful_rs::*;
use std::future::Future;

declare_shard!(Ui, runtime = main);

#[eventful(shard = Ui)]
struct Gallery {
    tabs: Tabs,
    hints: KeyHints,
    log: LogView,
    name: TextInput,
    save: Button,
    fruits: List,
    bar: ProgressBar,
    less: Button,
    more: Button,
    overlay_buttons: [Button; 6],
    keys: Keymap<Gallery>,
}

/// What the overlay page's buttons and the palette can do.
#[derive(Clone, Copy, Debug)]
enum Demo {
    Alert,
    Confirm,
    Prompt,
    Menu,
    Toast,
    Palette,
}

const DEMOS: [(Demo, &str); 6] = [
    (Demo::Alert, "Alert"),
    (Demo::Confirm, "Confirm"),
    (Demo::Prompt, "Prompt"),
    (Demo::Menu, "Menu"),
    (Demo::Toast, "Toast"),
    (Demo::Palette, "Palette"),
];

impl Gallery {
    fn new() -> Self {
        let log = LogView::new();
        let name = TextInput::new().placeholder("Ada Lovelace");
        let save = Button::new("Save").primary();
        let fruits = List::new([
            "Apple",
            "Apricot",
            "Banana",
            "Blueberry",
            "Cherry",
            "Date",
            "Fig",
            "Grape",
            "Kiwi",
            "Lemon",
            "Mango",
            "Orange",
            "Peach",
            "Pear",
            "Plum",
        ])
        .multi_select();
        let bar = ProgressBar::new();
        bar.set_value(Some(0.3));
        let (less, more) = (Button::new("-10%"), Button::new("+10%"));

        let form = Stack::vertical()
            .spacing(1)
            .child(Constraint::Length(1), labelled("Name", name.clone()))
            .child(
                Constraint::Length(1),
                labelled(
                    "Password",
                    TextInput::new().password().placeholder("secret"),
                ),
            )
            .child(
                Constraint::Length(1),
                labelled(
                    "Port",
                    TextInput::new()
                        .value("8080")
                        .validator(|text| match text.parse::<u16>() {
                            Ok(_) => Ok(()),
                            Err(_) => Err("not a port".into()),
                        }),
                ),
            )
            .child(
                Constraint::Length(1),
                Checkbox::new("Subscribe to the newsletter"),
            )
            .child(
                Constraint::Length(1),
                Switch::new("Dark mode").checked(true),
            )
            .child(
                Constraint::Length(1),
                RadioGroup::new(["Small", "Medium", "Large"])
                    .horizontal()
                    .selected(1),
            )
            .child(
                Constraint::Length(1),
                Stack::horizontal()
                    .spacing(2)
                    .fit(save.clone())
                    .fit(Button::new("Delete").danger())
                    .fit({
                        let disabled = Button::new("Disabled");
                        disabled.set_enabled(false);
                        disabled
                    }),
            );
        let controls = Panel::new("Controls")
            .padding(Padding::horizontal(1))
            .child(form);

        let lists = Stack::horizontal()
            .spacing(1)
            .child(
                Constraint::Percentage(40),
                Panel::new("Fruits")
                    .footer("space marks, enter picks")
                    .child(fruits.clone()),
            )
            .child(Constraint::Fill(1), Panel::new("Log").child(log.clone()));

        let progress = Panel::new("Progress").padding(Padding::uniform(1)).child(
            Stack::vertical()
                .spacing(1)
                .fit(Spinner::new("Working…"))
                .child(Constraint::Length(1), bar.clone())
                .child(Constraint::Length(1), ProgressBar::new())
                .fit(
                    Stack::horizontal()
                        .spacing(2)
                        .fit(less.clone())
                        .fit(more.clone()),
                ),
        );

        let mut long_form = Stack::vertical();
        for i in 1..=40 {
            long_form = long_form.child(
                Constraint::Length(1),
                labelled(&format!("Field {i:02}"), TextInput::new().placeholder("…")),
            );
        }
        let scroll = Panel::new("Scroll").child(ScrollView::new(long_form));

        let overlay_buttons = DEMOS.map(|(_, label)| Button::new(label));
        let mut row = Stack::horizontal().spacing(2);
        for button in &overlay_buttons {
            row = row.fit(button.clone());
        }
        let overlays = Panel::new("Overlays").padding(Padding::uniform(1)).child(
            Stack::vertical()
                .spacing(1)
                .fit(Label::new("Dialogs, menus, toasts and the command palette (ctrl+p).").muted())
                .fit(row),
        );

        let tabs = Tabs::new()
            .page("Controls", controls)
            .page("Lists", lists)
            .page("Progress", progress)
            .page("Overlays", overlays)
            .page("Scroll", scroll);
        let hints = KeyHints::new()
            .hint("tab", "focus")
            .hint("ctrl+pgup/pgdn", "pages")
            .hint("ctrl+p", "commands")
            .hint("ctrl+c", "quit");
        Self {
            tabs,
            hints,
            log,
            name,
            save,
            fruits,
            bar,
            less,
            more,
            overlay_buttons,
            keys: Keymap::new()
                .bind("ctrl+c", |_| quit())
                .bind("ctrl+p", |g: &Gallery| g.demo(Demo::Palette)),
            events: Default::default(),
        }
    }

    fn saved(&self) {
        self.log.push(format!("saved {:?}", self.name.text()));
    }
    fn picked(&self, index: usize) {
        let name = self
            .fruits
            .item(index)
            .map(|l| l.to_string())
            .unwrap_or_default();
        self.log.push(format!("picked {name}"));
    }
    fn page(&self, index: usize) {
        self.log
            .push(Line::from(format!("page {}", self.tabs.titles()[index])).dim());
    }
    fn nudge(&self, delta: f64) {
        let value = self.bar.value().unwrap_or(0.0) + delta;
        self.bar.set_value(Some(value));
    }
    /// Run `task` on the UI shard, owned by the gallery.
    fn spawn<F: Future<Output = ()> + 'static>(&self, task: impl FnOnce(ShardWeak<Self>) -> F) {
        if let Ok(me) = ShardRc::try_from_ref(self) {
            let _ = ShardRc::spawn_owned(&me, task);
        }
    }

    fn demo(&self, demo: Demo) {
        let log = self.log.clone();
        let anchor = self.overlay_buttons[3].area().unwrap_or_default();
        self.spawn(move |_| async move {
            let outcome = match demo {
                Demo::Alert => {
                    alert("About", "eventful-ratatui component gallery.").await;
                    "alert closed".to_owned()
                }
                Demo::Confirm => {
                    let ok = confirm("Delete", "Delete all 3 files? This cannot be undone.").await;
                    format!("confirmed: {ok}")
                }
                Demo::Prompt => match prompt("Rename", "New name:", "untitled").await {
                    Some(name) => format!("renamed to {name:?}"),
                    None => "rename cancelled".to_owned(),
                },
                Demo::Menu => {
                    let picked = Menu::new()
                        .item_with_hint("cut", "Cut", "ctrl+x")
                        .item_with_hint("copy", "Copy", "ctrl+c")
                        .item_with_hint("paste", "Paste", "ctrl+v")
                        .separator()
                        .disabled_item("undo", "Undo")
                        .item("select all", "Select all")
                        .show(Placement::Below(anchor))
                        .await;
                    format!("menu: {picked:?}")
                }
                Demo::Toast => {
                    Toast::success("Saved!").show();
                    Toast::warning("Disk 91% full").show();
                    "toasts shown".to_owned()
                }
                Demo::Palette => {
                    let mut palette = CommandPalette::new();
                    for (demo, label) in DEMOS {
                        palette = palette.command(demo, format!("Show {label}"));
                    }
                    format!("palette: {:?}", palette.show().await)
                }
            };
            log.push(outcome);
        });
    }

    fn less(&self) {
        self.nudge(-0.1);
    }
    fn more(&self) {
        self.nudge(0.1);
    }
}

/// `label` in a fixed-width column, then `input`.
fn labelled(label: &str, input: impl Component + 'static) -> Stack {
    Stack::horizontal()
        .child(Constraint::Length(10), Label::new(label.to_owned()).muted())
        .child(Constraint::Fill(1), input)
}

impl Component for Gallery {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [body, hints] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        self.tabs.render(body, buf);
        self.hints.render(hints, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        self.keys.dispatch(self, event)
    }
}

#[sharded_main(Ui)]
async fn main() -> eventful_ratatui::Result<()> {
    let gallery = Gallery::bind_local(Gallery::new())?;
    let _saved = gallery
        .save
        .clicked()
        .with_receiver(&gallery)
        .connect(Gallery::saved)
        .scoped();
    let _picked = gallery
        .fruits
        .activated()
        .with_receiver(&gallery)
        .connect(Gallery::picked)
        .scoped();
    let _page = gallery
        .tabs
        .selected()
        .with_receiver(&gallery)
        .connect(Gallery::page)
        .scoped();
    let _less = gallery
        .less
        .clicked()
        .with_receiver(&gallery)
        .connect(Gallery::less)
        .scoped();
    let _more = gallery
        .more
        .clicked()
        .with_receiver(&gallery)
        .connect(Gallery::more)
        .scoped();
    let _demos: Vec<_> = DEMOS
        .iter()
        .zip(&gallery.overlay_buttons)
        .map(|(&(demo, _), button)| {
            button
                .clicked()
                .with_receiver(&gallery)
                .connect(move |g: &Gallery| g.demo(demo))
                .scoped()
        })
        .collect();
    gallery.name.focus();
    Tui::new().mouse(true).run(&gallery).await
}
