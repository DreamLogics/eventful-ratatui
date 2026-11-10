//! [`Theme`]: the palette and symbols components draw with.
//!
//! A theme is a small [`Palette`] of colors named for what they are for, not
//! which component uses them: the same `primary` fills a main button, the
//! selected tab and a progress bar; the same `field` is the background of
//! every input, list and track. The colors for focus, hover and disabled are
//! derived from the palette (see [`color`]), so a palette that gets its base
//! colors right gets every state right.
//!
//! Surfaces stack from the back: `background` (the screen), `surface`
//! (dialogs, menus, toasts), then `field`, a "well" one shade further from
//! the background that controls sit in. A well lifts a step further on
//! focus, again on hover, and the cursor row of a list lifts the furthest.
//!
//! Every built-in theme comes as a dark and a light variant. By default the
//! loop asks the terminal for its background color and picks [`Theme::dark`]
//! or [`Theme::light`] to match; [`Tui::theme`](crate::Tui::theme) or
//! [`Tui::themes`](crate::Tui::themes) choose others. Components read
//! [`current`] while rendering, and [`set`] switches at run time.
//!
//! ```
//! use eventful_ratatui::{prelude::*, theme::Theme};
//!
//! let mut theme = Theme::catppuccin_mocha();
//! theme.palette.primary = Color::Rgb(0xf5, 0xc2, 0xe7);
//! let tui = Tui::new().theme(theme);
//! ```
use crate::{focus, mouse};
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use std::{cell::RefCell, rc::Rc};

pub mod color;

use color::{away_from, blend, lift};

/// How far a filled control, like a button, shifts when focused, in percent.
pub const FOCUS_SHIFT: u8 = 16;
/// How far a filled control shifts when hovered; more than focus, so
/// pointing at a focused control still shows.
pub const HOVER_SHIFT: u8 = 24;
/// How far a well lifts from `field` when focused.
pub const WELL_FOCUS_SHIFT: u8 = 5;
/// How far a well lifts from `field` when hovered.
pub const WELL_HOVER_SHIFT: u8 = 9;
/// How far the cursor row of a list lifts past the well it sits in.
pub const ROW_SHIFT: u8 = 14;
/// How far a disabled control blends toward the background.
pub const DISABLED_DIM: u8 = 50;
/// How far everything below a modal blends toward the background.
pub const BACKDROP_DIM: u8 = 60;

/// The base colors of a theme. Every state color is derived from these.
///
/// New fields may be added in minor releases; start from a preset and assign
/// the fields you want to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Palette {
    /// The screen, behind everything.
    pub background: Color,
    /// Raised layers: dialogs, menus, toasts.
    pub surface: Color,
    /// Wells that controls sit in: inputs, lists, tracks.
    pub field: Color,
    /// Body text.
    pub foreground: Color,
    /// Secondary text: hints, placeholders, captions.
    pub muted: Color,
    /// The main accent: main buttons, the selected tab, progress.
    pub primary: Color,
    /// Text on `primary`.
    pub primary_foreground: Color,
    /// Quieter fills: ordinary buttons, chips.
    pub secondary: Color,
    /// Text on `secondary`.
    pub secondary_foreground: Color,
    /// A vivid signature color for occasional emphasis.
    pub accent: Color,
    /// Destructive actions and errors.
    pub destructive: Color,
    /// Text on `destructive`.
    pub destructive_foreground: Color,
    /// Success.
    pub success: Color,
    /// Warnings.
    pub warning: Color,
    /// Neutral information.
    pub info: Color,
    /// Borders and separators.
    pub border: Color,
    /// The border of a container holding the focus.
    pub ring: Color,
    /// The background of selected text.
    pub selection: Color,
}

/// Whether a theme is for a dark or a light terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    /// Light text on a dark background.
    Dark,
    /// Dark text on a light background.
    Light,
}

impl Appearance {
    /// Ask the terminal for its background color and tell which appearance
    /// matches it. Falls back to [`Dark`](Self::Dark) when the terminal does
    /// not answer, or when output is not a terminal.
    ///
    /// Call it before the terminal is in raw mode and reading input, as
    /// [`Tui::run`](crate::Tui::run) does.
    pub fn detect() -> Self {
        use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};
        match theme_mode(QueryOptions::default()) {
            Ok(ThemeMode::Light) => Self::Light,
            _ => Self::Dark,
        }
    }
}

/// The interaction state a control is drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interaction {
    /// It has the keyboard focus.
    pub focused: bool,
    /// The mouse pointer is over it.
    pub hovered: bool,
    /// It cannot be used.
    pub disabled: bool,
}

impl Interaction {
    /// The state of `component` in the last frame: focused and hovered as
    /// [`focus::is_focused`] and [`mouse::is_hovered`] report. Pass the
    /// component itself; inside `render`, `self`.
    pub fn of<C: ?Sized>(component: &C) -> Self {
        Self {
            focused: focus::is_focused(component),
            hovered: mouse::is_hovered(component),
            disabled: false,
        }
    }
    /// The same state, disabled when `disabled` is true.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// The same state, focused when `focused` is true.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
    /// The same state, hovered when `hovered` is true.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }
    /// Focused or hovered: the control is active.
    pub fn is_active(self) -> bool {
        !self.disabled && (self.focused || self.hovered)
    }
}

/// The fill of a control, from loudest to quietest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fill {
    /// The main action: filled with `primary`.
    Primary,
    /// An ordinary action: filled with `secondary`.
    #[default]
    Secondary,
    /// A destructive action: filled with `destructive`.
    Destructive,
    /// No fill until focused or hovered.
    Ghost,
}

/// A color role for status: badges, toasts, progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tone {
    /// No particular meaning.
    #[default]
    Neutral,
    /// The main accent.
    Primary,
    /// Something worked.
    Success,
    /// Something needs attention.
    Warning,
    /// Something failed.
    Error,
    /// Neutral information.
    Info,
}

/// A palette plus the symbols and border shape components draw with.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Theme {
    /// A display name, for a theme picker.
    pub name: &'static str,
    /// The base colors.
    pub palette: Palette,
    /// The border shape of containers and overlays.
    pub border_type: BorderType,
    /// Symbols for checkboxes, radios and the like.
    pub symbols: Symbols,
}

/// The symbols components draw with.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Symbols {
    /// Checkbox off and on.
    pub checkbox: [&'static str; 2],
    /// Checkbox in the indeterminate state.
    pub checkbox_mixed: &'static str,
    /// Radio button off and on.
    pub radio: [&'static str; 2],
    /// A switch's knob.
    pub knob: &'static str,
    /// Collapsed and expanded tree nodes and accordions.
    pub disclosure: [&'static str; 2],
    /// Marks the cursor row of a focused list, in the primary color.
    pub highlight: &'static str,
    /// Spinner frames.
    pub spinner: &'static [&'static str],
    /// Scrollbar track and thumb.
    pub scrollbar: [&'static str; 2],
    /// Marks a submenu.
    pub submenu: &'static str,
    /// Separates breadcrumb items.
    pub separator: &'static str,
    /// Masks password characters.
    pub mask: char,
    /// Ascending and descending sort markers.
    pub sort: [&'static str; 2],
    /// The top and bottom rows of a three-row button.
    pub caps: [&'static str; 2],
    /// The left and right ends of a pill, like a badge or a switch.
    pub ends: [&'static str; 2],
}

impl Default for Symbols {
    fn default() -> Self {
        Self {
            checkbox: ["□", "■"],
            checkbox_mixed: "▣",
            radio: ["○", "●"],
            knob: "●",
            disclosure: ["▸", "▾"],
            highlight: "▌",
            spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            scrollbar: ["│", "┃"],
            submenu: "▸",
            separator: " › ",
            mask: '•',
            sort: ["▲", "▼"],
            caps: ["▄", "▀"],
            ends: ["▐", "▌"],
        }
    }
}

impl Symbols {
    /// ASCII only, for terminals and fonts without box drawing or braille.
    pub fn ascii() -> Self {
        Self {
            checkbox: ["[ ]", "[x]"],
            checkbox_mixed: "[-]",
            radio: ["( )", "(*)"],
            knob: "o",
            disclosure: [">", "v"],
            highlight: ">",
            spinner: &["|", "/", "-", "\\"],
            scrollbar: ["|", "#"],
            submenu: ">",
            separator: " > ",
            mask: '*',
            sort: ["^", "v"],
            caps: [" ", " "],
            ends: [" ", " "],
        }
    }
}

impl Default for Theme {
    /// [`Theme::dark`].
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// A theme named `name` over `palette`, with the default symbols.
    pub fn new(name: &'static str, palette: Palette) -> Self {
        Self {
            name,
            palette,
            border_type: BorderType::Rounded,
            symbols: Symbols::default(),
        }
    }

    /// Whether this theme is dark or light, read off its background.
    pub fn appearance(&self) -> Appearance {
        if color::is_light(self.palette.background) {
            Appearance::Light
        } else {
            Appearance::Dark
        }
    }

    /// Every built-in theme, for a theme picker.
    pub fn presets() -> Vec<Theme> {
        vec![
            Self::dark(),
            Self::light(),
            Self::catppuccin_mocha(),
            Self::catppuccin_latte(),
            Self::gruvbox_dark(),
            Self::gruvbox_light(),
            Self::nord(),
            Self::tokyo_night(),
        ]
    }

    /// Neutral grays with a white primary, after shadcn/ui.
    pub fn dark() -> Self {
        let background = rgb(0x0a0a0a);
        let primary = rgb(0xe5e5e5);
        Self::new(
            "Dark",
            Palette {
                background,
                surface: rgb(0x161616),
                field: rgb(0x212121),
                foreground: rgb(0xfafafa),
                muted: rgb(0xa1a1a1),
                primary,
                primary_foreground: rgb(0x171717),
                secondary: rgb(0x2a2a2a),
                secondary_foreground: rgb(0xfafafa),
                accent: rgb(0x8b5cf6),
                destructive: rgb(0xff6467),
                destructive_foreground: rgb(0x171717),
                success: rgb(0x4ade80),
                warning: rgb(0xfacc15),
                info: rgb(0x60a5fa),
                border: rgb(0x5e5e5e),
                ring: blend(primary, background, 15),
                selection: rgb(0x264f78),
            },
        )
    }

    /// Neutral grays with a black primary, after shadcn/ui.
    pub fn light() -> Self {
        let background = rgb(0xffffff);
        let primary = rgb(0x171717);
        Self::new(
            "Light",
            Palette {
                background,
                surface: rgb(0xf7f7f7),
                field: rgb(0xefefef),
                foreground: rgb(0x0a0a0a),
                muted: rgb(0x666666),
                primary,
                primary_foreground: rgb(0xfafafa),
                secondary: rgb(0xe6e6e6),
                secondary_foreground: rgb(0x171717),
                accent: rgb(0x7c3aed),
                destructive: rgb(0xdc2626),
                destructive_foreground: rgb(0xfafafa),
                success: rgb(0x15803d),
                warning: rgb(0xa16207),
                info: rgb(0x1d4ed8),
                border: rgb(0x8f8f8f),
                ring: blend(primary, background, 15),
                selection: rgb(0xbfd7f5),
            },
        )
    }

    /// Catppuccin Mocha: soft pastels on deep indigo.
    pub fn catppuccin_mocha() -> Self {
        let background = rgb(0x11111b); // crust
        let primary = rgb(0x89b4fa); // blue
        let field = rgb(0x1e1e2e); // base
        Self::new(
            "Catppuccin Mocha",
            Palette {
                background,
                surface: blend(background, field, 40),
                field,
                foreground: rgb(0xcdd6f4),           // text
                muted: rgb(0xa6adc8),                // subtext0
                primary,                             //
                primary_foreground: background,      //
                secondary: rgb(0x313244),            // surface0
                secondary_foreground: rgb(0xcdd6f4), // text
                accent: rgb(0xcba6f7),               // mauve
                destructive: rgb(0xf38ba8),          // red
                destructive_foreground: background,  //
                success: rgb(0xa6e3a1),              // green
                warning: rgb(0xf9e2af),              // yellow
                info: rgb(0x74c7ec),                 // sapphire
                border: rgb(0x6c7086),               // overlay0
                ring: blend(primary, background, 15),
                selection: rgb(0x45475a), // surface1
            },
        )
    }

    /// Catppuccin Latte: the light flavor.
    pub fn catppuccin_latte() -> Self {
        let background = rgb(0xeff1f5); // base
        let primary = rgb(0x1e66f5); // blue
        Self::new(
            "Catppuccin Latte",
            Palette {
                background,
                surface: rgb(0xe6e9ef),    // mantle
                field: rgb(0xdce0e8),      // crust
                foreground: rgb(0x4c4f69), // text
                muted: rgb(0x5c5f77),      // subtext1: subtext0 is under 4.5:1 on base
                primary,
                // White: base is under 4.5:1 on Latte blue.
                primary_foreground: rgb(0xffffff),
                secondary: rgb(0xccd0da), // surface0
                secondary_foreground: rgb(0x4c4f69),
                accent: rgb(0x8839ef),      // mauve
                destructive: rgb(0xd20f39), // red
                destructive_foreground: rgb(0xffffff),
                success: rgb(0x40a02b), // green
                warning: rgb(0xfe640b), // peach: yellow is too faint on base
                info: rgb(0x209fb5),    // sapphire
                border: rgb(0x8c8fa1),  // overlay1
                ring: blend(primary, background, 15),
                selection: rgb(0xbcc0cc), // surface1
            },
        )
    }

    /// Gruvbox dark: warm retro tones.
    pub fn gruvbox_dark() -> Self {
        let background = rgb(0x282828); // bg0
        let primary = rgb(0xfabd2f); // bright yellow
        Self::new(
            "Gruvbox Dark",
            Palette {
                background,
                surface: rgb(0x32302f),    // bg0_s
                field: rgb(0x3c3836),      // bg1
                foreground: rgb(0xebdbb2), // fg1
                muted: rgb(0xbdae93),      // fg3
                primary,
                primary_foreground: background,
                secondary: rgb(0x504945), // bg2
                secondary_foreground: rgb(0xebdbb2),
                accent: rgb(0xfe8019),                 // bright orange
                destructive: rgb(0xfb4934),            // bright red
                destructive_foreground: rgb(0x1d2021), // bg0_h
                success: rgb(0xb8bb26),                // bright green
                warning: rgb(0xfabd2f),                // bright yellow
                info: rgb(0x83a598),                   // bright blue
                border: rgb(0x7c6f64),                 // bg4
                ring: blend(primary, background, 15),
                selection: rgb(0x665c54), // bg3
            },
        )
    }

    /// Gruvbox light: the faded accents on cream.
    pub fn gruvbox_light() -> Self {
        let background = rgb(0xfbf1c7); // bg0
        let primary = rgb(0x076678); // faded blue
        Self::new(
            "Gruvbox Light",
            Palette {
                background,
                surface: rgb(0xf2e5bc),    // bg0_s
                field: rgb(0xebdbb2),      // bg1
                foreground: rgb(0x3c3836), // fg1
                muted: rgb(0x665c54),      // fg3
                primary,
                primary_foreground: background,
                secondary: rgb(0xd5c4a1), // bg2
                secondary_foreground: rgb(0x3c3836),
                accent: rgb(0xaf3a03),      // faded orange
                destructive: rgb(0x9d0006), // faded red
                destructive_foreground: background,
                success: rgb(0x79740e), // faded green
                warning: rgb(0xb57614), // faded yellow
                info: rgb(0x427b58),    // faded aqua
                border: rgb(0x928374),  // gray
                ring: blend(primary, background, 15),
                selection: rgb(0xd5c4a1), // bg2
            },
        )
    }

    /// Nord: cool, desaturated blues.
    pub fn nord() -> Self {
        let background = rgb(0x2e3440); // nord0
        let primary = rgb(0x88c0d0); // nord8
        let field = rgb(0x3b4252); // nord1
        Self::new(
            "Nord",
            Palette {
                background,
                surface: blend(background, field, 50),
                field,
                foreground: rgb(0xeceff4), // nord6
                muted: rgb(0xd8dee9),      // nord4
                primary,
                primary_foreground: background,
                secondary: rgb(0x4c566a), // nord3
                secondary_foreground: rgb(0xeceff4),
                accent: rgb(0xb48ead),      // nord15
                destructive: rgb(0xbf616a), // nord11
                destructive_foreground: rgb(0xeceff4),
                success: rgb(0xa3be8c), // nord14
                warning: rgb(0xebcb8b), // nord13
                info: rgb(0x81a1c1),    // nord9
                border: rgb(0x707e9b),
                ring: blend(primary, background, 15),
                selection: rgb(0x434c5e), // nord2
            },
        )
    }

    /// Tokyo Night: saturated blues and purples on near-black.
    pub fn tokyo_night() -> Self {
        let background = rgb(0x1a1b26); // bg
        let primary = rgb(0x7aa2f7); // blue
        let field = rgb(0x292e42); // bg_highlight
        Self::new(
            "Tokyo Night",
            Palette {
                background,
                surface: blend(background, field, 50),
                field,
                foreground: rgb(0xc0caf5), // fg
                muted: rgb(0xa9b1d6),      // fg_dark
                primary,
                primary_foreground: background,
                secondary: rgb(0x283457), // bg_visual
                secondary_foreground: rgb(0xc0caf5),
                accent: rgb(0xbb9af7),      // magenta
                destructive: rgb(0xf7768e), // red
                destructive_foreground: background,
                success: rgb(0x9ece6a), // green
                warning: rgb(0xe0af68), // yellow
                info: rgb(0x7dcfff),    // cyan
                border: rgb(0x737aa2),  // dark5
                ring: blend(primary, background, 15),
                selection: rgb(0x33467c),
            },
        )
    }

    /// The light or dark default for `appearance`.
    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }

    // ---- Text ------------------------------------------------------------

    /// The screen: body text on the background.
    pub fn screen(&self) -> Style {
        Style::new()
            .fg(self.palette.foreground)
            .bg(self.palette.background)
    }
    /// Body text; keeps the background it is drawn on.
    pub fn text(&self) -> Style {
        Style::new().fg(self.palette.foreground)
    }
    /// Secondary text; keeps the background it is drawn on.
    pub fn muted(&self) -> Style {
        Style::new().fg(self.palette.muted)
    }
    /// Container and section titles.
    pub fn title(&self) -> Style {
        Style::new()
            .fg(self.palette.foreground)
            .add_modifier(Modifier::BOLD)
    }
    /// Text in the primary color, for emphasis.
    pub fn accent(&self) -> Style {
        Style::new().fg(self.palette.primary)
    }
    /// Placeholder text in empty inputs.
    pub fn placeholder(&self) -> Style {
        Style::new().fg(self.palette.muted)
    }
    /// Text of a disabled control; keeps the background.
    pub fn disabled(&self) -> Style {
        Style::new().fg(blend(
            self.palette.muted,
            self.palette.background,
            DISABLED_DIM,
        ))
    }
    /// The color of `tone`.
    pub fn tone(&self, tone: Tone) -> Color {
        let p = &self.palette;
        match tone {
            Tone::Neutral => p.muted,
            Tone::Primary => p.primary,
            Tone::Success => p.success,
            Tone::Warning => p.warning,
            Tone::Error => p.destructive,
            Tone::Info => p.info,
        }
    }
    /// Error text.
    pub fn error(&self) -> Style {
        Style::new().fg(self.palette.destructive)
    }
    /// Warning text.
    pub fn warning(&self) -> Style {
        Style::new().fg(self.palette.warning)
    }
    /// Success text.
    pub fn success(&self) -> Style {
        Style::new().fg(self.palette.success)
    }
    /// Information text.
    pub fn info(&self) -> Style {
        Style::new().fg(self.palette.info)
    }

    // ---- Lines -----------------------------------------------------------

    /// A container border: the ring color while it holds the focus.
    pub fn border(&self, focused: bool) -> Style {
        Style::new().fg(if focused {
            self.palette.ring
        } else {
            self.palette.border
        })
    }

    // ---- Surfaces --------------------------------------------------------

    /// A raised layer: dialogs, menus, toasts.
    pub fn surface(&self) -> Style {
        Style::new()
            .fg(self.palette.foreground)
            .bg(self.palette.surface)
    }

    /// The background of a well in `state`: `field`, lifted on focus and
    /// further on hover.
    pub fn well_color(&self, state: Interaction) -> Color {
        let p = &self.palette;
        let away = away_from(p.background);
        if state.disabled {
            blend(p.field, p.background, DISABLED_DIM)
        } else if state.hovered {
            blend(p.field, away, WELL_HOVER_SHIFT)
        } else if state.focused {
            blend(p.field, away, WELL_FOCUS_SHIFT)
        } else {
            p.field
        }
    }

    /// A well in `state`: inputs, lists, tracks.
    pub fn well(&self, state: Interaction) -> Style {
        let fg = if state.disabled {
            blend(self.palette.muted, self.palette.background, DISABLED_DIM)
        } else {
            self.palette.foreground
        };
        Style::new().fg(fg).bg(self.well_color(state))
    }

    /// The cursor row of a list in a well in state `well`. The row lifts
    /// past the well; it lifts less while the list has neither focus nor the
    /// pointer, so the selection stays visible.
    pub fn row(&self, well: Interaction) -> Style {
        let base = self.well_color(well);
        let away = away_from(self.palette.background);
        let mut amount = if well.is_active() {
            ROW_SHIFT
        } else {
            ROW_SHIFT / 2
        };
        // Lift less where the palette's text would lose too much contrast.
        let fg = self.palette.foreground;
        while amount > 2 && color::contrast(fg, blend(base, away, amount)).is_some_and(|c| c < 4.5)
        {
            amount -= 1;
        }
        Style::new().fg(fg).bg(blend(base, away, amount))
    }

    /// A row under the pointer in a list in state `well`.
    pub fn row_hover(&self, well: Interaction) -> Style {
        let base = self.well_color(well);
        let away = away_from(self.palette.background);
        Style::new().bg(blend(base, away, ROW_SHIFT / 2))
    }

    /// Selected text in an input.
    pub fn selection(&self) -> Style {
        Style::new()
            .fg(self.palette.foreground)
            .bg(self.palette.selection)
    }

    // ---- Controls --------------------------------------------------------

    /// A filled control, like a button, in `state`. A ghost fill is the
    /// background it is drawn on until focused or hovered; check
    /// [`Style::bg`] for `None`.
    pub fn fill(&self, fill: Fill, state: Interaction) -> Style {
        let p = &self.palette;
        let (bg, fg) = match fill {
            Fill::Primary => (p.primary, p.primary_foreground),
            Fill::Secondary => (p.secondary, p.secondary_foreground),
            Fill::Destructive => (p.destructive, p.destructive_foreground),
            Fill::Ghost => (p.secondary, p.foreground),
        };
        if state.disabled {
            let style = Style::new().fg(blend(fg, bg, DISABLED_DIM));
            return match fill {
                Fill::Ghost => style.fg(blend(p.muted, p.background, DISABLED_DIM)),
                _ => style.bg(blend(bg, p.background, DISABLED_DIM)),
            };
        }
        let shift = match (state.hovered, state.focused) {
            (true, true) => HOVER_SHIFT + FOCUS_SHIFT / 2,
            (true, false) => HOVER_SHIFT,
            (false, true) => FOCUS_SHIFT,
            (false, false) => 0,
        };
        let style = if fill == Fill::Ghost && !state.is_active() {
            Style::new().fg(p.foreground)
        } else {
            Style::new().fg(fg).bg(shifted(bg, fg, shift))
        };
        if state.focused {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    /// A scrollbar track.
    pub fn scrollbar(&self) -> Style {
        Style::new().fg(blend(self.palette.border, self.palette.background, 40))
    }
    /// A scrollbar thumb.
    pub fn scrollbar_thumb(&self) -> Style {
        Style::new().fg(self.palette.muted)
    }
}

/// `fill` shifted `percent` to show a state, keeping `label` on it readable:
/// away from the fill's own lightness where the label stays readable,
/// otherwise away from the label.
fn shifted(fill: Color, label: Color, percent: u8) -> Color {
    use color::contrast;
    if percent == 0 {
        return fill;
    }
    let floor = contrast(label, fill).map_or(4.5, |base| base.min(4.5));
    let lifted = lift(fill, percent);
    if contrast(label, lifted).is_none_or(|c| c >= floor) {
        return lifted;
    }
    blend(fill, away_from(label), percent)
}

/// An RGB color from `0xRRGGBB`.
const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

thread_local! {
    static CURRENT: RefCell<Rc<Theme>> = RefCell::new(Rc::new(Theme::default()));
}

/// The theme of this thread's loop.
pub fn current() -> Rc<Theme> {
    CURRENT.with_borrow(Rc::clone)
}

/// Replace the theme of this thread's loop, and redraw.
pub fn set(theme: Theme) {
    let _old = CURRENT.replace(Rc::new(theme));
    crate::invalidate::invalidate();
}

#[cfg(test)]
mod tests {
    use super::{color::contrast, *};

    /// Every preset keeps text readable on every rung it is drawn on.
    #[test]
    fn presets_keep_text_readable() {
        for theme in Theme::presets() {
            let p = theme.palette;
            let name = theme.name;
            let at_least = |what: &str, fg: Color, bg: Color, floor: f64| {
                let ratio = contrast(fg, bg).expect("rgb");
                assert!(
                    ratio >= floor,
                    "{name}: {what} contrast {ratio:.2} below {floor}"
                );
            };
            let hot = Interaction {
                focused: true,
                hovered: true,
                disabled: false,
            };
            at_least("text", p.foreground, p.background, 7.0);
            at_least("text on field", p.foreground, p.field, 4.5);
            // Large wells lift on focus only; the cursor row lifts past that.
            let focused = Interaction::default().focused(true);
            at_least(
                "text on cursor row",
                p.foreground,
                theme.row(focused).bg.unwrap(),
                4.5,
            );
            at_least("muted", p.muted, p.background, 4.5);
            at_least("muted on field", p.muted, p.field, 3.0);
            at_least("primary label", p.primary_foreground, p.primary, 4.5);
            for state in [Interaction::default(), hot] {
                for fill in [
                    Fill::Primary,
                    Fill::Secondary,
                    Fill::Destructive,
                    Fill::Ghost,
                ] {
                    let style = theme.fill(fill, state);
                    let bg = style.bg.unwrap_or(p.background);
                    at_least("button label", style.fg.unwrap(), bg, 3.0);
                }
            }
            at_least("border", p.border, p.background, 2.0);
            at_least("ring", p.ring, p.background, 3.0);
            for tone in [
                Tone::Success,
                Tone::Warning,
                Tone::Error,
                Tone::Info,
                Tone::Primary,
            ] {
                at_least("tone", theme.tone(tone), p.background, 2.5);
            }
        }
    }

    #[test]
    fn appearance_matches_the_background() {
        for theme in Theme::presets() {
            let light = theme.name.contains("Light") || theme.name.contains("Latte");
            assert_eq!(
                theme.appearance() == Appearance::Light,
                light,
                "{}",
                theme.name
            );
        }
    }

    #[test]
    fn wells_lift_away_from_the_background() {
        for theme in [Theme::dark(), Theme::light()] {
            let p = theme.palette;
            let rest = theme.well_color(Interaction::default());
            let hover = theme.well_color(Interaction::default().hovered(true));
            let distance = |c| contrast(c, p.background).unwrap();
            assert!(distance(hover) > distance(rest), "{}", theme.name);
        }
    }
}
