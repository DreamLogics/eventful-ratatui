//! [`Theme`]: the semantic styles and symbols components draw with.
//!
//! The default theme uses only the 16 ANSI colors, so it follows the user's
//! terminal scheme, light or dark. [`Theme::dark`] and [`Theme::light`] are
//! fixed RGB alternatives. Set one with [`Tui::theme`](crate::Tui::theme) or
//! [`set`]; components read [`current`] while rendering.
//!
//! ```
//! use eventful_ratatui::{prelude::*, theme::Theme};
//!
//! let mut theme = Theme::default();
//! theme.accent = Style::new().magenta();
//! theme.border_focused = theme.accent;
//! let tui = Tui::new().theme(theme);
//! ```
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use std::{cell::RefCell, rc::Rc};

/// Semantic styles plus the symbols components draw with.
///
/// Fields are public so a theme can be adjusted from a built-in one. New
/// fields may be added in minor releases.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Theme {
    /// Body text.
    pub text: Style,
    /// Secondary text: hints, captions, key help.
    pub muted: Style,
    /// The accent color: links, active tabs, progress, focused borders.
    pub accent: Style,
    /// Borders of unfocused containers.
    pub border: Style,
    /// Borders of containers holding the focus.
    pub border_focused: Style,
    /// Container titles.
    pub title: Style,
    /// The border shape.
    pub border_type: BorderType,
    /// A focused control, like a button.
    pub focused: Style,
    /// The selected item of a focused list, table or menu.
    pub selected: Style,
    /// The selected item of a list that does not have focus.
    pub selected_unfocused: Style,
    /// Disabled controls.
    pub disabled: Style,
    /// Text being edited.
    pub input: Style,
    /// Placeholder text in empty inputs.
    pub placeholder: Style,
    /// Selected text in inputs.
    pub text_selection: Style,
    /// Errors and destructive actions.
    pub error: Style,
    /// Warnings.
    pub warning: Style,
    /// Success.
    pub success: Style,
    /// Neutral information.
    pub info: Style,
    /// Overlays: dialogs, menus, toasts.
    pub surface: Style,
    /// Patched over everything below a modal overlay.
    pub backdrop: Style,
    /// Scrollbar tracks.
    pub scrollbar: Style,
    /// Scrollbar thumbs.
    pub scrollbar_thumb: Style,
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
    /// Switch off and on.
    pub switch: [&'static str; 2],
    /// Collapsed and expanded tree nodes and accordions.
    pub disclosure: [&'static str; 2],
    /// Marks the highlighted item in menus and lists.
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
}

impl Default for Symbols {
    fn default() -> Self {
        Self {
            checkbox: ["[ ]", "[x]"],
            checkbox_mixed: "[-]",
            radio: ["( )", "(•)"],
            switch: ["○──", "──●"],
            disclosure: ["▸", "▾"],
            highlight: "› ",
            spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            scrollbar: ["│", "┃"],
            submenu: "▸",
            separator: " › ",
            mask: '•',
            sort: ["▲", "▼"],
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
            switch: ["[ off]", "[ on ]"],
            disclosure: [">", "v"],
            highlight: "> ",
            spinner: &["|", "/", "-", "\\"],
            scrollbar: ["|", "#"],
            submenu: ">",
            separator: " > ",
            mask: '*',
            sort: ["^", "v"],
        }
    }
}

impl Default for Theme {
    /// The ANSI theme: follows the terminal's own colors.
    fn default() -> Self {
        Self {
            text: Style::new(),
            muted: Style::new().dark_gray(),
            accent: Style::new().cyan(),
            border: Style::new().dark_gray(),
            border_focused: Style::new().cyan(),
            title: Style::new().bold(),
            border_type: BorderType::Rounded,
            focused: Style::new().black().on_cyan(),
            selected: Style::new().black().on_cyan(),
            selected_unfocused: Style::new().reversed(),
            disabled: Style::new().dark_gray(),
            input: Style::new(),
            placeholder: Style::new().dark_gray().italic(),
            text_selection: Style::new().reversed(),
            error: Style::new().red(),
            warning: Style::new().yellow(),
            success: Style::new().green(),
            info: Style::new().blue(),
            surface: Style::new(),
            backdrop: Style::new().dark_gray().add_modifier(Modifier::DIM),
            scrollbar: Style::new().dark_gray(),
            scrollbar_thumb: Style::new(),
            symbols: Symbols::default(),
        }
    }
}

impl Theme {
    /// A fixed dark palette.
    pub fn dark() -> Self {
        Self::rgb(Palette {
            background: Color::Rgb(0x1e, 0x1f, 0x26),
            surface: Color::Rgb(0x2a, 0x2b, 0x35),
            text: Color::Rgb(0xe4, 0xe4, 0xec),
            muted: Color::Rgb(0x8a, 0x8c, 0x9e),
            accent: Color::Rgb(0x7a, 0xa2, 0xf7),
            on_accent: Color::Rgb(0x1e, 0x1f, 0x26),
            selection: Color::Rgb(0x3b, 0x3e, 0x52),
            error: Color::Rgb(0xf7, 0x76, 0x8e),
            warning: Color::Rgb(0xe0, 0xaf, 0x68),
            success: Color::Rgb(0x9e, 0xce, 0x6a),
        })
    }

    /// A fixed light palette.
    pub fn light() -> Self {
        Self::rgb(Palette {
            background: Color::Rgb(0xfa, 0xfa, 0xfc),
            surface: Color::Rgb(0xee, 0xef, 0xf3),
            text: Color::Rgb(0x24, 0x25, 0x2e),
            muted: Color::Rgb(0x6b, 0x6e, 0x80),
            accent: Color::Rgb(0x2e, 0x5c, 0xd6),
            on_accent: Color::Rgb(0xff, 0xff, 0xff),
            selection: Color::Rgb(0xd5, 0xdc, 0xf2),
            error: Color::Rgb(0xc6, 0x28, 0x3c),
            warning: Color::Rgb(0x9a, 0x67, 0x00),
            success: Color::Rgb(0x2f, 0x7d, 0x32),
        })
    }

    fn rgb(p: Palette) -> Self {
        let text = Style::new().fg(p.text).bg(p.background);
        Self {
            text,
            muted: Style::new().fg(p.muted),
            accent: Style::new().fg(p.accent),
            border: Style::new().fg(p.muted),
            border_focused: Style::new().fg(p.accent),
            title: Style::new().fg(p.text).bold(),
            border_type: BorderType::Rounded,
            focused: Style::new().fg(p.on_accent).bg(p.accent),
            selected: Style::new().fg(p.on_accent).bg(p.accent),
            selected_unfocused: Style::new().fg(p.text).bg(p.selection),
            disabled: Style::new().fg(p.muted).add_modifier(Modifier::DIM),
            input: Style::new().fg(p.text),
            placeholder: Style::new().fg(p.muted).italic(),
            text_selection: Style::new().fg(p.text).bg(p.selection),
            error: Style::new().fg(p.error),
            warning: Style::new().fg(p.warning),
            success: Style::new().fg(p.success),
            info: Style::new().fg(p.accent),
            surface: Style::new().fg(p.text).bg(p.surface),
            backdrop: Style::new().fg(p.muted).add_modifier(Modifier::DIM),
            scrollbar: Style::new().fg(p.selection),
            scrollbar_thumb: Style::new().fg(p.muted),
            symbols: Symbols::default(),
        }
    }
}

/// Colors an RGB theme is derived from.
struct Palette {
    background: Color,
    surface: Color,
    text: Color,
    muted: Color,
    accent: Color,
    on_accent: Color,
    selection: Color,
    error: Color,
    warning: Color,
    success: Color,
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
