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
//! # The built-in theme
//!
//! There is one built-in theme, generated from the terminal's own colors: at
//! startup the loop asks the terminal for its 16 ANSI colors, foreground and
//! background ([`TerminalColors::query`]) and derives every palette color
//! from them ([`Theme::from_terminal`]). The app follows the user's color
//! scheme, light or dark, while getting real shades for surfaces and wells.
//!
//! # Theme files
//!
//! Other themes are [CROL](https://crates.io/crates/crol) data files
//! (`.crod`) with a `<dark>` and a `<light>` variant; the loop loads the one
//! that matches the terminal's background. This crate ships a few in its
//! `themes/` directory: Neutral, Catppuccin, Gruvbox, Nord and Tokyo Night.
//!
//! ```text
//! name: "Mine"
//! <dark> {
//!     palette: { background: "#101014", foreground: "#e8e8ee", /* … */ },
//! }
//! <light> {
//!     palette: { background: "#fafafa", foreground: "#1a1a1e", /* … */ },
//! }
//! ```
//!
//! Every [`Palette`] field must be given, as `"#rrggbb"` or a color name.
//! Binds (`blue := "#89b4fa"`) and references work as everywhere in CROL;
//! `&self` is the enclosing map, so inside `palette`, `ring: &self.primary`
//! reuses the primary color.
//!
//! ```no_run
//! use eventful_ratatui::{prelude::*, theme::ThemeFile};
//!
//! # fn main() -> std::io::Result<()> {
//! let tui = Tui::new().theme_file(ThemeFile::open("themes/catppuccin.crod")?);
//! # Ok(())
//! # }
//! ```
//!
//! [`Tui::theme`](crate::Tui::theme) sets a fixed [`Theme`] instead.
//! Components read [`current`] while rendering, and [`set`] switches at run
//! time.
//!
//! # Drawing with the theme
//!
//! Custom components take their styles from the theme rather than picking
//! colors, so they match the library's components and follow theme changes:
//!
//! ```
//! use eventful_ratatui::{prelude::*, testing::Harness, theme::{Fill, Interaction}};
//! use std::rc::Rc;
//!
//! /// A ghost button: no fill until focused or hovered.
//! struct Action(&'static str);
//! impl Component for Action {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let style = theme::current().fill(Fill::Ghost, Interaction::of(self));
//!         Line::styled(self.0, style).render(area, buf);
//!     }
//!     fn focusable(&self) -> bool {
//!         true
//!     }
//! }
//!
//! let mut ui = Harness::new(Rc::new(Action("Undo")), 4, 1);
//! // Unfocused, it shows the screen background.
//! assert_eq!(ui.buffer()[(0, 0)].bg, theme::current().palette.background);
//! ui.key("tab");
//! let focused = theme::current().fill(Fill::Ghost, Interaction::default().focused(true));
//! assert_eq!(Some(ui.buffer()[(0, 0)].bg), focused.bg);
//! ```
use crate::{focus, mouse};
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use std::{cell::RefCell, rc::Rc};

pub mod color;
mod file;
mod terminal;

pub use file::{ThemeError, ThemeFile};
pub use terminal::TerminalColors;

use color::{away_from, blend, lift};

/// How far a filled control, like a button, shifts when focused, in percent.
pub(crate) const FOCUS_SHIFT: u8 = 16;
/// How far a filled control shifts when hovered; more than focus, so
/// pointing at a focused control still shows.
pub(crate) const HOVER_SHIFT: u8 = 24;
/// How far a well lifts from `field` when focused.
pub(crate) const WELL_FOCUS_SHIFT: u8 = 5;
/// How far a well lifts from `field` when hovered.
pub(crate) const WELL_HOVER_SHIFT: u8 = 9;
/// How far the cursor row of a list lifts past the well it sits in.
pub(crate) const ROW_SHIFT: u8 = 14;
/// How far a disabled control blends toward the background.
pub(crate) const DISABLED_DIM: u8 = 50;
/// How far everything below a modal blends toward the background.
pub(crate) const BACKDROP_DIM: u8 = 60;

/// The base colors of a theme. Every state color is derived from these.
///
/// New fields may be added in minor releases; start from an existing theme's
/// palette and assign the fields you want to change:
///
/// ```
/// use eventful_ratatui::{prelude::*, theme::Theme};
///
/// let mut palette = Theme::default().palette;
/// palette.primary = Color::Rgb(0xd9, 0x46, 0xef);
/// palette.ring = palette.primary;
/// let theme = Theme::new("Orchid", palette);
/// assert_eq!(theme.palette.primary, Color::Rgb(0xd9, 0x46, 0xef));
/// ```
///
/// Keep `*_foreground` colors readable on their fill, and `foreground` and
/// `muted` readable on `background`, `surface` and `field`: aim for a
/// contrast of 4.5:1 ([`color::contrast`]).
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
    /// The appearance whose background `background` is: light when its
    /// relative luminance is above 0.4 ([`color::is_light`]). Colors without
    /// channels, like `Reset`, count as dark.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::Appearance};
    ///
    /// assert_eq!(Appearance::of(Color::Rgb(0xfa, 0xfa, 0xfa)), Appearance::Light);
    /// assert_eq!(Appearance::of(Color::Rgb(0x1e, 0x1e, 0x1e)), Appearance::Dark);
    /// assert_eq!(Appearance::of(Color::Reset), Appearance::Dark);
    /// ```
    pub fn of(background: Color) -> Self {
        if color::is_light(background) {
            Self::Light
        } else {
            Self::Dark
        }
    }

    /// The name of this appearance's variant in theme files: `dark` or
    /// `light`.
    pub fn variant(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// The interaction state a control is drawn in, for the theme's state
/// styles like [`Theme::fill`] and [`Theme::well`].
///
/// [`of`](Self::of) reads focus and hover from the last frame; the builder
/// methods override them, for example to disable a control:
///
/// ```
/// use eventful_ratatui::theme::Interaction;
///
/// let state = Interaction::default().focused(true);
/// assert!(state.is_active());
/// assert!(!state.disabled(true).is_active());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Focused or hovered, and not disabled: the control is active.
    pub fn is_active(self) -> bool {
        !self.disabled && (self.focused || self.hovered)
    }
}

/// The fill of a control, from loudest to quietest. See [`Theme::fill`].
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

/// A color role for status: badges, toasts, progress. [`Theme::tone`] maps
/// it to a palette color.
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
///
/// The default is the built-in theme over [`TerminalColors::default`]. Make
/// one with [`Theme::new`], [`Theme::from_terminal`] or
/// [`ThemeFile::load`], adjust its public fields, and install it with
/// [`set`] or [`Tui::theme`](crate::Tui::theme):
///
/// ```
/// use eventful_ratatui::{prelude::*, theme::{Symbols, Theme}};
///
/// // For terminals or fonts without box drawing.
/// let mut theme = Theme::default();
/// theme.symbols = Symbols::ascii();
/// theme.border_type = BorderType::Plain;
/// let tui = Tui::new().theme(theme);
/// ```
///
/// The style methods give the style for each role; components draw with
/// them, and custom components should too (see the
/// [module docs](self#drawing-with-the-theme)).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Theme {
    /// A display name, for a theme picker.
    pub name: String,
    /// The base colors.
    pub palette: Palette,
    /// The border shape of containers and overlays. Rounded by default.
    pub border_type: BorderType,
    /// Symbols for checkboxes, radios and the like.
    pub symbols: Symbols,
}

/// The symbols components draw with. The default uses Unicode box drawing,
/// geometric shapes and braille; [`ascii`](Self::ascii) is the fallback.
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
    /// The built-in theme over [`TerminalColors::default`], a common dark
    /// palette; what draws when the terminal is not asked, as in tests.
    fn default() -> Self {
        Self::from_terminal(&TerminalColors::default())
    }
}

impl Theme {
    /// A theme named `name` over `palette`, with the default symbols and
    /// rounded borders. See [`Palette`] for an example.
    pub fn new(name: impl Into<String>, palette: Palette) -> Self {
        Self {
            name: name.into(),
            palette,
            border_type: BorderType::Rounded,
            symbols: Symbols::default(),
        }
    }

    /// Whether this theme is dark or light, read off its background.
    pub fn appearance(&self) -> Appearance {
        Appearance::of(self.palette.background)
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
    /// The color of `tone`: `muted` for neutral, `destructive` for errors,
    /// and the palette color of the same name otherwise.
    ///
    /// ```
    /// use eventful_ratatui::theme::{Theme, Tone};
    ///
    /// let theme = Theme::default();
    /// assert_eq!(theme.tone(Tone::Error), theme.palette.destructive);
    /// assert_eq!(theme.tone(Tone::Neutral), theme.palette.muted);
    /// ```
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

    /// A container border: the ring color while it holds the focus. Pass
    /// [`focus::within`] of the container for `focused`.
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
    /// further on hover, and blended toward the background when disabled.
    /// Lifts are reduced where they would cost the text too much contrast.
    pub fn well_color(&self, state: Interaction) -> Color {
        let p = &self.palette;
        if state.disabled {
            blend(p.field, p.background, DISABLED_DIM)
        } else if state.hovered {
            self.lift_well(p.field, WELL_HOVER_SHIFT)
        } else if state.focused {
            self.lift_well(p.field, WELL_FOCUS_SHIFT)
        } else {
            p.field
        }
    }

    /// `base` lifted `amount` away from the background, less where the text
    /// would lose too much contrast: no more than 5% of what it has on
    /// `base`, and not below 4.5:1 from above it.
    fn lift_well(&self, base: Color, mut amount: u8) -> Color {
        let away = away_from(self.palette.background);
        let fg = self.palette.foreground;
        let contrast = |c| color::contrast(fg, c).unwrap_or(21.0);
        let floor = contrast(base).min(4.5 / 0.95) * 0.95;
        while amount > 1 && contrast(blend(base, away, amount)) < floor {
            amount -= 1;
        }
        blend(base, away, amount)
    }

    /// A well in `state`: inputs, lists, tracks. The background is
    /// [`well_color`](Self::well_color), with the foreground dimmed when
    /// disabled.
    ///
    /// ```
    /// use eventful_ratatui::theme::{Interaction, Theme};
    ///
    /// let theme = Theme::default();
    /// let rest = theme.well(Interaction::default());
    /// let hovered = theme.well(Interaction::default().hovered(true));
    /// assert_eq!(rest.bg, Some(theme.palette.field));
    /// assert_ne!(hovered.bg, rest.bg);
    /// ```
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
        let amount = if well.is_active() {
            ROW_SHIFT
        } else {
            ROW_SHIFT / 2
        };
        Style::new()
            .fg(self.palette.foreground)
            .bg(self.lift_well(self.well_color(well), amount))
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
    ///
    /// Focus and hover shift the fill away from its lightness, hover more
    /// than focus, while keeping the label readable; focus also makes the
    /// label bold. Disabled fills blend toward the background.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::{Fill, Interaction, Theme}};
    ///
    /// let theme = Theme::default();
    /// let rest = theme.fill(Fill::Primary, Interaction::default());
    /// assert_eq!(rest.bg, Some(theme.palette.primary));
    /// assert_eq!(rest.fg, Some(theme.palette.primary_foreground));
    ///
    /// let focused = theme.fill(Fill::Primary, Interaction::default().focused(true));
    /// assert_ne!(focused.bg, rest.bg);
    /// assert!(focused.add_modifier.contains(Modifier::BOLD));
    ///
    /// assert_eq!(theme.fill(Fill::Ghost, Interaction::default()).bg, None);
    /// ```
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
/// otherwise away from the label's.
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
    // Away from the label: darker under a lighter label, lighter under a
    // darker one.
    let lighter = color::luminance(label) > color::luminance(fill);
    let away = if lighter {
        Color::Rgb(0, 0, 0)
    } else {
        Color::Rgb(255, 255, 255)
    };
    blend(fill, away, percent)
}

thread_local! {
    static CURRENT: RefCell<Rc<Theme>> = RefCell::new(Rc::new(Theme::default()));
}

/// The theme of this thread's loop: [`Theme::default`] until one is set.
/// Read it in `render`; it is cheap to call.
///
/// ```
/// use eventful_ratatui::{prelude::*, theme::{Symbols, Theme}};
///
/// let mut ascii = Theme::default();
/// ascii.symbols = Symbols::ascii();
/// theme::set(ascii);
/// assert_eq!(theme::current().symbols.checkbox, ["[ ]", "[x]"]);
/// ```
pub fn current() -> Rc<Theme> {
    CURRENT.with_borrow(Rc::clone)
}

/// Replace the theme of this thread's loop, and redraw. Use it to switch
/// themes at run time, for a theme picker; the change shows in the next
/// frame. See [`current`] for an example.
///
/// Themes are per thread: call it on the UI shard. A loop started with
/// [`Tui::theme`](crate::Tui::theme) or
/// [`Tui::theme_file`](crate::Tui::theme_file) sets its theme when it starts,
/// replacing an earlier `set`.
pub fn set(theme: Theme) {
    let _old = CURRENT.replace(Rc::new(theme));
    crate::invalidate::invalidate();
}

#[cfg(test)]
mod tests {
    use super::{color::contrast, *};

    /// The theme files this crate ships, by name.
    pub(crate) const FILES: [(&str, &str); 5] = [
        ("neutral", include_str!("../themes/neutral.crod")),
        ("catppuccin", include_str!("../themes/catppuccin.crod")),
        ("gruvbox", include_str!("../themes/gruvbox.crod")),
        ("nord", include_str!("../themes/nord.crod")),
        ("tokyo-night", include_str!("../themes/tokyo-night.crod")),
    ];

    /// Every variant of every shipped file.
    fn file_themes() -> Vec<(Appearance, Theme)> {
        FILES
            .iter()
            .flat_map(|(name, source)| {
                let file = ThemeFile::from_source(*name, *source);
                [Appearance::Dark, Appearance::Light].map(|appearance| {
                    let theme = file
                        .load(appearance)
                        .unwrap_or_else(|err| panic!("{name} {appearance:?}: {err}"));
                    (appearance, theme)
                })
            })
            .collect()
    }

    /// Terminal palettes to generate the built-in theme from: the default,
    /// xterm's, a light one, and Solarized, which is low on contrast.
    pub(crate) fn sample_terminals() -> Vec<(&'static str, TerminalColors)> {
        let hex = |h: u32| Color::Rgb((h >> 16) as u8, (h >> 8) as u8, h as u8);
        let ansi = |colors: [u32; 16]| colors.map(hex);
        vec![
            ("default", TerminalColors::default()),
            (
                "xterm",
                TerminalColors::new(
                    hex(0xffffff),
                    hex(0x000000),
                    ansi([
                        0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd,
                        0xe5e5e5, 0x7f7f7f, 0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff,
                        0x00ffff, 0xffffff,
                    ]),
                ),
            ),
            (
                "light",
                TerminalColors::new(
                    hex(0x2e3436),
                    hex(0xffffff),
                    ansi([
                        0x2e3436, 0xcc0000, 0x4e9a06, 0xc4a000, 0x3465a4, 0x75507b, 0x06989a,
                        0xd3d7cf, 0x555753, 0xef2929, 0x8ae234, 0xfce94f, 0x729fcf, 0xad7fa8,
                        0x34e2e2, 0xeeeeec,
                    ]),
                ),
            ),
            (
                "solarized",
                TerminalColors::new(
                    hex(0x839496),
                    hex(0x002b36),
                    ansi([
                        0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198,
                        0xeee8d5, 0x002b36, 0xcb4b16, 0x586e75, 0x657b83, 0x839496, 0x6c71c4,
                        0x93a1a1, 0xfdf6e3,
                    ]),
                ),
            ),
        ]
    }

    /// `theme` keeps text readable on every rung it is drawn on. `text` is the
    /// floor for body text and `well` for text in wells, which a terminal
    /// palette may not reach.
    fn check_readable(theme: &Theme, text: f64, well: f64) {
        let p = theme.palette;
        let name = &theme.name;
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
        at_least("text", p.foreground, p.background, text);
        at_least("text on field", p.foreground, p.field, well);
        // Large wells lift on focus only; the cursor row lifts past that.
        let focused = Interaction::default().focused(true);
        at_least(
            "text on cursor row",
            p.foreground,
            theme.row(focused).bg.unwrap(),
            // Two lifts: the focused well, then the row.
            (well.min(4.5 / 0.95) * 0.95).min(4.5 / 0.95) * 0.95,
        );
        at_least("muted", p.muted, p.background, 4.5);
        at_least("muted on field", p.muted, p.field, 3.0);
        at_least("primary label", p.primary_foreground, p.primary, 4.5);
        at_least(
            "destructive label",
            p.destructive_foreground,
            p.destructive,
            3.0,
        );
        for state in [Interaction::default(), hot] {
            for fill in [
                Fill::Primary,
                Fill::Secondary,
                Fill::Destructive,
                Fill::Ghost,
            ] {
                let style = theme.fill(fill, state);
                let bg = style.bg.unwrap_or(p.background);
                at_least(
                    &format!("{fill:?} label {state:?}"),
                    style.fg.unwrap(),
                    bg,
                    3.0,
                );
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

    #[test]
    fn theme_files_keep_text_readable() {
        for (_, theme) in file_themes() {
            check_readable(&theme, 4.5, 4.5);
        }
    }

    #[test]
    fn theme_file_variants_match_their_appearance() {
        for (appearance, theme) in file_themes() {
            assert_eq!(theme.appearance(), appearance, "{}", theme.name);
        }
    }

    #[test]
    fn the_terminal_theme_keeps_text_readable() {
        for (name, colors) in sample_terminals() {
            let theme = Theme::from_terminal(&colors);
            // The terminal's own text color is kept as it is.
            let text = contrast(colors.foreground, colors.background).unwrap();
            let well = terminal::well_text_floor(text);
            check_readable(&theme, text.min(4.5), well);
            assert_eq!(
                theme.appearance(),
                Appearance::of(colors.background),
                "{name}"
            );
        }
    }

    #[test]
    fn wells_lift_away_from_the_background() {
        for (_, colors) in sample_terminals() {
            let theme = Theme::from_terminal(&colors);
            let p = theme.palette;
            let rest = theme.well_color(Interaction::default());
            let hover = theme.well_color(Interaction::default().hovered(true));
            let distance = |c| contrast(c, p.background).unwrap();
            assert!(distance(hover) > distance(rest), "{}", theme.name);
        }
    }
}
