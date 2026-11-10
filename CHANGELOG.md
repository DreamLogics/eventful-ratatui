# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `THIRD-PARTY-NOTICES.md`, crediting [ratcn](https://github.com/kristoferlund/ratcn),
  whose visual design the components follow and whose theme colors and tuning
  several themes use, and the color schemes the themes are built on. The
  crate packages now also contain the license texts.
- Doctest examples for every component and core entry point, and fuller
  documentation throughout.

### Changed

- **Key chords**: with a modifier other than shift, a letter's case is
  ignored, so `"Ctrl+S"` is ctrl+s rather than ctrl+shift+s; write
  `"ctrl+shift+s"` for that. A capital letter on its own still means shift
  (`"G"`). `KeyChord`'s `Display` writes `ctrl+shift+s` instead of `ctrl+S`,
  and `#[key_events]` parses chords the same way.
- **Focus**: a focused component that stops being focusable, like a button
  that is disabled, passes focus to the next focusable component in its
  layer, or clears focus if there is none.

### Fixed

- `Tree::set_roots` clears the selection when the selected node is gone, as
  documented, instead of keeping a stale id.

## [0.1.0] - 2026-11-10

The first release.

### Added

- **The loop**: `run_tui` and the `Tui` builder run a root component on the
  main eventful-rs shard, taking turns with queued calls. Redraw requests from
  any shard (`Invalidate`, `invalidate::invalidate`, `invalidate::after`) are
  coalesced into at most one frame per interval; `Tui::tick` redraws on a
  timer. `quit` ends the loop, and the terminal is restored on return and on
  panic.
- **Components** (default `components` feature): `Button`, `Checkbox`,
  `Switch`, `RadioGroup`, `TextInput`, `TextArea` (with undo), `List`,
  `Table` (sortable), `Tree`, `Tabs`, `Panel`, `ScrollView`, `Split`,
  `Accordion`, `Stack`, `Label`, `Field`, `ProgressBar`, `Spinner`,
  `LogView`, `KeyHints`, `Slider`, `SpinBox`, `Select`, `ComboBox`, `Menu`,
  `MenuBar`, `CommandPalette` (fuzzy search), `Dialog` with `alert`,
  `confirm` and `prompt`, `Toast`, `Badge`, `Chip` and `Breadcrumb`. They
  report through eventful-rs events.
- **Focus**: Tab order follows render order; keys go to the focused component
  and bubble through its parents to the root.
- **Mouse** (`Tui::mouse`): events go to the component under the pointer and
  bubble; a press captures the mouse until release, and focuses what it
  lands on. Hover is tracked and redraws by itself (`mouse::is_hovered`,
  `mouse::hover_position`, `is_hovered()` on component handles).
- **Overlays**: popups, modals and passive overlays with placement, light
  dismiss, focus restore and an awaitable `Closed`.
- **Key bindings**: `Keymap` with chords like `"ctrl+s"`, and the
  `#[key_events]` attribute, which turns a trait into eventful-rs events
  bound to keys.
- **Themes**: a `Palette` of purpose colors from which focus, hover and
  disabled styles are derived (`Theme::fill`, `well`, `row`, …). The built-in
  theme is generated from the terminal's 16 colors, queried at startup
  (`TerminalColors::query`, `Theme::from_terminal`). Other themes load from
  CROL files with `<dark>` and `<light>` variants (`ThemeFile`,
  `Tui::theme_file`); Neutral, Catppuccin, Gruvbox, Nord and Tokyo Night ship
  in `themes/`, with a schema for `crol check` and editors.
- **Testing**: `testing::Harness` drives a component without a terminal:
  keys, text, paste, clicks, drags, scrolling, hover and timers, with the
  screen as text or cells.
- The `cursor` module, to place the terminal cursor from a component.

[Unreleased]: https://github.com/DreamLogics/eventful-ratatui/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/DreamLogics/eventful-ratatui/releases/tag/v0.1.0
