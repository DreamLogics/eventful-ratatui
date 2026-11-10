# eventful-ratatui

Terminal applications with [ratatui](https://ratatui.rs) on
[eventful-rs](https://github.com/DreamLogics/eventful-rs).

Workers on background shards do I/O and computation. UI components live on
the main shard as ordinary `#[eventful]` values. Events flow from workers to
components, components ask for a redraw, and one loop handles input and
drawing.

- **Components**: buttons, inputs, text areas, lists, tables, trees, tabs,
  selects, menus, dialogs, toasts, a command palette and more, all reporting
  what the user did through eventful-rs events.
- **Focus and mouse, without wiring**: every component records where it was
  drawn, so Tab order, click-to-focus, hover and mouse routing come from the
  frame itself.
- **Overlays**: popups, modal dialogs that dim what is below, and toasts.
- **Themes**: the built-in theme is generated from the terminal's own 16
  colors, so apps follow the user's color scheme. Other themes are
  [CROL](https://crates.io/crates/crol) files with a `<dark>` and a
  `<light>` variant.
- **Testing**: `testing::Harness` drives components without a terminal,
  through the same input routing as the real loop.

## Example

```toml
[dependencies]
eventful-ratatui = "0.1"
eventful-rs = "0.2.2"
```

```rust,no_run
use eventful_ratatui::prelude::*;
use eventful_rs::*;
use std::cell::Cell;

declare_shard!(Ui, runtime = main);

#[eventful(shard = Ui)]
struct Counter {
    count: Cell<u32>,
    label: Label,
    add: Button,
    keys: Keymap<Counter>,
}

impl Counter {
    fn add(&self) {
        self.count.set(self.count.get() + 1);
        self.label.set_text(format!("Clicked {} times", self.count.get()));
    }
}

impl Component for Counter {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [label, button] =
            Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).areas(area);
        self.label.render(label, buf);
        self.add.render(button, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        self.keys.dispatch(self, event)
    }
}

#[sharded_main(Ui)]
async fn main() -> eventful_ratatui::Result<()> {
    let counter = Counter::bind_local(Counter {
        count: Cell::new(0),
        label: Label::new("Not clicked yet; q quits"),
        add: Button::new("Add one").primary(),
        keys: Keymap::new().bind("q", |_| quit()),
        events: Default::default(),
    })?;
    // Components report through events; connect them to your methods.
    let _add = counter
        .add
        .clicked()
        .with_receiver(&counter)
        .connect(Counter::add)
        .scoped();
    counter.add.focus();
    Tui::new().mouse(true).run(&counter).await
}
```

The repository has three larger examples:

- `cargo run -p gallery`: every component, with a theme picker.
- `cargo run -p chat`: a chat client with a Tokio network worker.
- `cargo run -p dashboard`: a background import with progress, using plain
  ratatui widgets.

## Themes

At startup the loop asks the terminal for its foreground, background and 16
ANSI colors, and generates the built-in theme from them: surfaces and input
wells are shades between background and foreground, and accents come from the
ANSI colors, adjusted where they would be hard to read. When the terminal does
not answer (and on Windows), a common dark palette is used.

Other themes are CROL data files. Six are bundled: Catppuccin, Gruvbox,
Neutral, Nord, One (One Dark and One Light) and Tokyo Night. The loop loads
the variant that matches the terminal's background:

```rust,no_run
use eventful_ratatui::{prelude::*, theme::ThemeFile};

fn main() -> std::io::Result<()> {
    // A bundled theme; `ThemeFile::bundled()` lists them all.
    let tui = Tui::new().theme_file(ThemeFile::one());
    // Or a theme file of your own:
    let tui = Tui::new().theme_file(ThemeFile::open("my-theme.crod")?);
    Ok(())
}
```

The bundled themes are also files in
[`eventful-ratatui/themes/`](eventful-ratatui/themes); copy one to start
your own.

A theme file names the schema published with this crate, so `crol check` and
the CROL language server check it while you edit:

```text
#![schema("crate:eventful-ratatui@0.1")]

name: "Mine"
palette: { background: "#101014", foreground: "#e8e8ee", /* … */ }

<light> {
    palette: { background: "#fafafa", foreground: "#1a1a1e", /* … */ },
}
```

## Compatibility

| eventful-ratatui | eventful-rs | ratatui | crossterm | Rust |
| ---------------- | ----------- | ------- | --------- | ---- |
| 0.1              | 0.2.2+      | 0.30    | 0.29      | 1.99 |

Depend on `eventful-rs` yourself as well: its macros resolve the runtime
through your own dependency. Both must resolve to the same semver-compatible
version, or types like `ShardRc` from the two copies won't match.
`eventful_ratatui::eventful_rs` re-exports the copy this crate is built
against, as do `eventful_ratatui::ratatui` and `eventful_ratatui::crossterm`.

## Acknowledgements

The look of the components follows [ratcn](https://github.com/kristoferlund/ratcn)
by Kristofer Lund: fills instead of borders, input wells that lift on focus and
hover, and a palette of purpose-named colors. Several themes take their colors
and tuning from ratcn's. The color schemes are by their authors: shadcn/ui,
Catppuccin, Gruvbox, Nord, Atom's One (GitHub) and Tokyo Night. See
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for their licenses.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
