//! Every component on one screen. Run with `cargo run -p gallery`.
//!
//! Tab moves the focus, Ctrl+PageUp/PageDown switch pages, Ctrl+P opens the
//! command palette, the list at the bottom right picks the theme, the mouse works everywhere, and
//! Ctrl+C quits. What you do is logged on the Lists page.
use eventful_ratatui::prelude::*;
use eventful_ratatui::theme::{Theme, ThemeFile};
use eventful_rs::*;
use std::{cell::RefCell, future::Future, path::PathBuf};

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
    menu_bar: MenuBar,
    editor: TextArea,
    /// Picks the theme: the built-in one, then each theme file.
    theme_picker: Select,
    /// The theme files, in the order the picker lists them after the
    /// built-in theme.
    theme_files: Vec<PathBuf>,
    /// The theme the loop started with, once something else was picked.
    start_theme: RefCell<Option<Theme>>,
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
        bar.set_label(Some("Uploading notes.md".into()));
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
                    .with_selection(1),
            )
            .child(
                Constraint::Length(1),
                Stack::horizontal()
                    .spacing(1)
                    .fit(save.clone())
                    .fit(Button::new("Cancel"))
                    .fit(Button::new("Delete").destructive())
                    .fit(Button::new("Details").ghost())
                    .fit({
                        let disabled = Button::new("Disabled");
                        disabled.set_enabled(false);
                        disabled
                    }),
            )
            .child(
                Constraint::Length(3),
                Stack::horizontal()
                    .spacing(1)
                    .fit(Button::new("Deploy").primary().large())
                    .fit(Button::new("Preview").large())
                    .fit(Button::new("Destroy").destructive().large()),
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
                .child(Constraint::Length(2), bar.clone())
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

        let table = Table::new([
            Column::new("Package", Constraint::Fill(1)),
            Column::new("Version", Constraint::Length(9)),
            Column::new("Downloads", Constraint::Length(12)).right(),
        ])
        .sortable()
        .row(["ratatui", "0.30.2", "12,400,000"])
        .row(["crossterm", "0.29.0", "48,100,000"])
        .row(["eventful-rs", "0.2.2", "3,200"])
        .row(["tokio", "1.53.0", "310,000,000"])
        .row(["futures", "0.3.31", "250,000,000"])
        .row(["unicode-width", "0.2.1", "190,000,000"]);
        let tree = Tree::new([
            TreeNode::new("src", "src").expanded().children([
                TreeNode::new("src/lib.rs", "lib.rs"),
                TreeNode::new("src/components", "components").children([
                    TreeNode::new("src/components/button.rs", "button.rs"),
                    TreeNode::new("src/components/list.rs", "list.rs"),
                    TreeNode::new("src/components/tree.rs", "tree.rs"),
                ]),
            ]),
            TreeNode::new("examples", "examples")
                .children([TreeNode::new("examples/gallery", "gallery")]),
            TreeNode::new("Cargo.toml", "Cargo.toml"),
        ]);
        let data = Split::horizontal(
            Panel::new("Files").child(tree),
            Panel::new("Crates")
                .footer("click a header to sort")
                .child(table),
        )
        .ratio(0.35);

        let menu_bar = MenuBar::new()
            .menu("File")
            .item("new", "New", "ctrl+n")
            .item("open", "Open…", "ctrl+o")
            .separator()
            .item("save", "Save", "ctrl+s")
            .menu("Edit")
            .item("undo", "Undo", "ctrl+z")
            .item("redo", "Redo", "ctrl+y")
            .menu("Help")
            .item("about", "About", "");
        let editor = TextArea::new()
            .line_numbers()
            .tab_inserts()
            .value("fn main() {\n    println!(\"Hello from the gallery!\");\n}\n");
        let editor_page = Stack::vertical().fit(menu_bar.clone()).child(
            Constraint::Fill(1),
            Panel::new("main.rs").child(editor.clone()),
        );

        let volume = Slider::new(0.0, 100.0, 5.0).with_value(40.0);
        let port = TextInput::new().value("8080").validator(|t| {
            t.parse::<u16>()
                .map(|_| ())
                .map_err(|_| "must be 0–65535".into())
        });
        let more_page = Panel::new("More controls")
            .padding(Padding::horizontal(1))
            .child(
                Stack::vertical()
                    .spacing(1)
                    .fit(
                        Breadcrumb::new()
                            .item("home", "Home")
                            .item("settings", "Settings")
                            .item("audio", "Audio"),
                    )
                    .child(Constraint::Length(1), labelled("Volume", volume))
                    .child(
                        Constraint::Length(1),
                        labelled("Retries", SpinBox::new(0.0, 10.0, 1.0).with_value(3.0)),
                    )
                    .fit(Field::input("Port", port).help("The server listens here"))
                    .fit(
                        Stack::horizontal()
                            .spacing(1)
                            .fit(Chip::new("rust").selected(true))
                            .fit(Chip::new("tui"))
                            .fit(Chip::new("async"))
                            .fit(Badge::new("NEW"))
                            .fit(Badge::new("3 errors").tone(Tone::Error)),
                    )
                    .child(
                        Constraint::Fill(1),
                        Accordion::new()
                            .section(
                                "What is this?",
                                Label::new("A gallery of every component.").wrap(),
                            )
                            .section(
                                "Keyboard",
                                Label::new("Tab moves focus; arrows work inside.").wrap(),
                            )
                            .exclusive(),
                    ),
            );

        let tabs = Tabs::new()
            .page("Controls", controls)
            .page("More", more_page)
            .page("Lists", lists)
            .page("Data", data)
            .page("Editor", editor_page)
            .page("Progress", progress)
            .page("Overlays", overlays)
            .page("Scroll", scroll);
        let hints = KeyHints::new()
            .hint("tab", "focus")
            .hint("ctrl+pgup/pgdn", "pages")
            .hint("ctrl+p", "commands")
            .hint("ctrl+c", "quit");
        let theme_files = theme_files();
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
            menu_bar,
            editor,
            theme_picker: Select::new(
                std::iter::once("Terminal".to_owned()).chain(theme_files.iter().map(|p| label(p))),
            )
            .with_selection(0),
            theme_files,
            start_theme: RefCell::default(),
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

    fn menu(&self, id: String) {
        match id.as_str() {
            "new" => self.editor.set_text(""),
            "about" => self.demo(Demo::Alert),
            _ => {}
        }
        self.log.push(format!("menu bar: {id}"));
    }

    /// The picker's choice: the built-in theme, or a theme file in the
    /// appearance of the terminal.
    fn pick_theme(&self, index: usize) {
        // The loop started with the built-in theme, matching the terminal.
        let start = self
            .start_theme
            .borrow_mut()
            .get_or_insert_with(|| (*theme::current()).clone())
            .clone();
        let theme = match index.checked_sub(1).map(|i| &self.theme_files[i]) {
            None => Ok(start),
            Some(path) => ThemeFile::open(path)
                .map_err(|err| format!("{}: {err}", path.display()))
                .and_then(|file| file.load(start.appearance()).map_err(|err| err.to_string())),
        };
        match theme {
            Ok(theme) => {
                self.log.push(format!("theme: {}", theme.name));
                theme::set(theme);
            }
            Err(err) => {
                self.log.push(Line::from(err.clone()).red());
                Toast::error(err).show();
            }
        }
    }

    fn less(&self) {
        self.nudge(-0.1);
    }
    fn more(&self) {
        self.nudge(0.1);
    }
}

/// The theme files shipped with the crate, sorted.
fn theme_files() -> Vec<PathBuf> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../eventful-ratatui/themes");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "crod"))
        .collect();
    paths.sort();
    paths
}

/// A theme file's name for the picker: `tokyo-night.crod` is "Tokyo Night".
fn label(path: &std::path::Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    stem.split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
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
        // The key hints, then the theme picker at the right.
        let [hints, picker] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(20)]).areas(hints);
        self.hints.render(hints, buf);
        self.theme_picker.render(picker, buf);
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
    let _theme = gallery
        .theme_picker
        .selected()
        .with_receiver(&gallery)
        .connect(Gallery::pick_theme)
        .scoped();
    let _menu = gallery
        .menu_bar
        .triggered()
        .with_receiver(&gallery)
        .connect(Gallery::menu)
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
