//! Ready-made components: buttons, inputs, lists, tabs, panels, progress,
//! and more. Enabled by the default `components` feature.
//!
//! Every component is a cheap `Clone` handle: keep one in your own component,
//! render it with `render`, and read or change its state through methods.
//! Handles record themselves in the render tree, so focus, Tab order and mouse
//! input work without extra code. Draw styles come from the current
//! [`Theme`](crate::theme::Theme).
//!
//! The look favors fills over borders: buttons and tabs are filled blocks,
//! inputs, lists and tracks sit in "wells" a shade off the background, and
//! every control lights up under the mouse pointer. Draw your own components
//! the same way with the theme's state styles:
//!
//! ```
//! use eventful_ratatui::{prelude::*, theme::{Fill, Interaction}};
//!
//! struct Tag(&'static str);
//! impl Component for Tag {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let theme = theme::current();
//!         // Focused and hovered come from the last frame.
//!         let style = theme.fill(Fill::Ghost, Interaction::of(self));
//!         Line::styled(format!(" {} ", self.0), style).render(area, buf);
//!     }
//!     fn focusable(&self) -> bool {
//!         true
//!     }
//! }
//! ```
//!
//! Components report what the user did through eventful-rs events. Connect a
//! signal to a method of your component:
//!
//! ```
//! use eventful_ratatui::{components::*, prelude::*};
//! use eventful_rs::*;
//!
//! declare_shard!(Ui, runtime = main);
//!
//! #[eventful(shard = Ui)]
//! struct Editor {
//!     save: Button,
//!     name: TextInput,
//! }
//! impl Editor {
//!     fn save(&self) { /* … */ }
//!     fn renamed(&self, name: String) { /* … */ }
//! }
//! impl Component for Editor {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         let [name, save] = Layout::vertical([Constraint::Length(1); 2]).areas(area);
//!         self.name.render(name, buf);
//!         self.save.render(save, buf);
//!     }
//! }
//!
//! #[sharded_main(Ui)]
//! async fn main() -> eventful_ratatui::Result<()> {
//!     let editor = Editor::bind_local(Editor {
//!         save: Button::new("Save"),
//!         name: TextInput::new().placeholder("Name"),
//!         events: Default::default(),
//!     })?;
//!     let _save = editor.save.clicked().with_receiver(&editor).connect(Editor::save).scoped();
//!     let _name = editor.name.changed().with_receiver(&editor).connect(Editor::renamed).scoped();
//!     # if false {
//!     run_tui(&editor).await?;
//!     # }
//!     Ok(())
//! }
//! ```
//!
//! Delivery is queued, like all eventful-rs events. Read the current state
//! synchronously when you need it right away: `name.value()`.
mod accordion;
mod badge;
mod button;
mod choices;
mod combo_box;
mod dialog;
mod events;
mod field;
mod key_hints;
mod label;
mod list;
mod log_view;
mod menu;
mod menu_bar;
mod palette;
mod panel;
mod progress;
mod radio;
mod scroll_view;
mod select;
mod slider;
mod split;
mod stack;
mod table;
mod tabs;
mod text_area;
mod text_input;
mod toast;
mod toggle;
mod tree;

pub use crate::theme::Tone;
pub use accordion::Accordion;
pub use badge::{Badge, Breadcrumb, Chip};
pub use button::{Button, ButtonKind, ButtonSize};
pub use combo_box::ComboBox;
pub use dialog::{Dialog, alert, confirm, prompt};
pub use events::*;
pub use field::Field;
pub use key_hints::KeyHints;
pub use label::Label;
pub use list::List;
pub use log_view::LogView;
pub use menu::Menu;
pub use menu_bar::MenuBar;
pub use palette::CommandPalette;
pub use panel::Panel;
pub use progress::{ProgressBar, Spinner};
pub use radio::RadioGroup;
pub use scroll_view::ScrollView;
pub use select::Select;
pub use slider::{Slider, SpinBox};
pub use split::Split;
pub use stack::Stack;
pub use table::{Column, Table};
pub use tabs::Tabs;
pub use text_area::TextArea;
pub use text_input::TextInput;
pub use toast::{Toast, ToastKind};
pub use toggle::{Checkbox, Switch};
pub use tree::{Tree, TreeNode};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEventKind};
use ratatui_core::layout::Position;

/// Define a component handle: a `Clone` wrapper around `Rc<$state>` that
/// records itself in the render tree, plus focus helpers and, with
/// `events = (Set, SignalsExt)`, the event source impls over the state's
/// `events` field.
macro_rules! handle {
    ($(#[$meta:meta])* $name:ident($state:ty) $(, events = ($set:ty, $ext:path))?) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name(::std::rc::Rc<$state>);

        impl $crate::Component for $name {
            fn render(&self, area: ::ratatui_core::layout::Rect, buf: &mut ::ratatui_core::buffer::Buffer) {
                let focusable = $crate::Component::focusable(&*self.0);
                let _node = $crate::tree::record(
                    area,
                    $crate::tree::Handle::of(self),
                    focusable,
                    || (&self.0).into(),
                );
                $crate::Component::render(&*self.0, area, buf);
            }
            fn on_input(&self, event: &::crossterm::event::Event) -> $crate::Handled {
                $crate::Component::on_input(&*self.0, event)
            }
            fn focusable(&self) -> bool {
                $crate::Component::focusable(&*self.0)
            }
            fn preferred_size(&self, available: ::ratatui_core::layout::Size) -> ::ratatui_core::layout::Size {
                $crate::Component::preferred_size(&*self.0, available)
            }
            fn __records(&self) -> bool {
                true
            }
        }

        impl $name {
            /// Give it keyboard focus.
            pub fn focus(&self) {
                $crate::focus::set(&*self.0);
            }
            /// Whether it has keyboard focus.
            pub fn is_focused(&self) -> bool {
                $crate::focus::is_focused(&*self.0)
            }
            /// Where it was last drawn, if it was.
            pub fn area(&self) -> ::core::option::Option<::ratatui_core::layout::Rect> {
                $crate::mouse::area_of(&*self.0)
            }
            /// Whether the mouse pointer is over it.
            pub fn is_hovered(&self) -> bool {
                $crate::mouse::is_hovered(&*self.0)
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_tuple(::core::stringify!($name)).finish_non_exhaustive()
            }
        }

        $(
            impl ::eventful_rs::HasEvents<$set> for $name {
                fn events(&self) -> &::std::sync::Arc<$set> {
                    self.0.events.signals()
                }
            }
            impl $ext for $name {}
        )?
    };
}
pub(crate) use handle;

/// The key of a press or repeat.
pub(crate) fn key(event: &Event) -> Option<&KeyEvent> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(key),
        _ => None,
    }
}

/// Whether `event` presses `code` without modifiers (shift allowed for
/// characters).
pub(crate) fn pressed(event: &Event, code: KeyCode) -> bool {
    key(event).is_some_and(|key| {
        key.code == code && (key.modifiers - crossterm::event::KeyModifiers::SHIFT).is_empty()
    })
}

/// Enter or Space: the "activate" keys of buttons and toggles.
pub(crate) fn activates(event: &Event) -> bool {
    pressed(event, KeyCode::Enter) || pressed(event, KeyCode::Char(' '))
}

/// Where the left button went down, if it did.
pub(crate) fn left_press(event: &Event) -> Option<Position> {
    match event {
        Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
            Some(Position::new(m.column, m.row))
        }
        _ => None,
    }
}

/// Where the left button went up, if it did.
pub(crate) fn left_release(event: &Event) -> Option<Position> {
    match event {
        Event::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left) => {
            Some(Position::new(m.column, m.row))
        }
        _ => None,
    }
}

/// Wheel steps: positive scrolls down.
pub(crate) fn wheel(event: &Event) -> Option<i32> {
    match event {
        Event::Mouse(m) if m.kind == MouseEventKind::ScrollDown => Some(1),
        Event::Mouse(m) if m.kind == MouseEventKind::ScrollUp => Some(-1),
        _ => None,
    }
}

/// Display width of `text` in cells.
pub(crate) fn width(text: &str) -> u16 {
    u16::try_from(unicode_width::UnicodeWidthStr::width(text)).unwrap_or(u16::MAX)
}

/// Paint a button-shaped fill of `label` across `area`: one row, or with
/// half-block caps above and below when `area` is three rows tall. A fill
/// without a background (a resting ghost) paints only the label.
pub(crate) fn paint_button(
    buf: &mut ratatui_core::buffer::Buffer,
    area: ratatui_core::layout::Rect,
    label: &str,
    style: ratatui::style::Style,
    theme: &crate::theme::Theme,
) {
    use ratatui::{layout::Alignment, style::Style, text::Line, widgets::Widget};
    use ratatui_core::layout::Rect;
    if area.is_empty() {
        return;
    }
    let tall = area.height >= 3;
    let middle = Rect::new(area.x, area.y + u16::from(tall), area.width, 1);
    if tall && let Some(fill) = style.bg {
        let [top, bottom] = theme.symbols.caps;
        let caps = Style::new().fg(fill);
        let row = |symbol: &str| Line::styled(symbol.repeat(usize::from(area.width)), caps);
        row(top).render(Rect { height: 1, ..area }, buf);
        row(bottom).render(Rect::new(area.x, area.y + 2, area.width, 1), buf);
    }
    buf.set_style(middle, style);
    Line::styled(label, style)
        .alignment(Alignment::Center)
        .render(middle, buf);
}

/// `area` a cell in from either side, where the text of a well goes.
pub(crate) fn pad(area: ratatui_core::layout::Rect) -> ratatui_core::layout::Rect {
    if area.width > 2 {
        ratatui_core::layout::Rect {
            x: area.x + 1,
            width: area.width - 2,
            ..area
        }
    } else {
        area
    }
}

/// The style of a row in a list-like well in state `well`: the cursor row
/// lifts, the row under the pointer lifts a little.
pub(crate) fn row_style(
    theme: &crate::theme::Theme,
    well: crate::theme::Interaction,
    cursor: bool,
    hovered: bool,
) -> ratatui::style::Style {
    if well.disabled {
        theme.well(well)
    } else if cursor {
        theme.row(well)
    } else if hovered {
        theme.well(well).patch(theme.row_hover(well))
    } else {
        theme.well(well)
    }
}

/// The marker cell at the start of a row: the highlight symbol on the
/// cursor row of a focused list, blank otherwise.
pub(crate) fn row_marker(theme: &crate::theme::Theme, show: bool) -> ratatui::text::Span<'static> {
    let symbol = theme.symbols.highlight;
    if show {
        ratatui::text::Span::styled(symbol, theme.accent())
    } else {
        ratatui::text::Span::raw(" ".repeat(usize::from(width(symbol))))
    }
}

/// The row of `area` under the pointer, if `component` is hovered.
pub(crate) fn hovered_row<C: ?Sized>(
    component: &C,
    area: ratatui_core::layout::Rect,
) -> Option<u16> {
    let position = crate::mouse::hover_position(component)?;
    area.contains(position).then(|| position.y - area.y)
}

/// Draw a vertical scrollbar at the right edge of `area`.
pub(crate) fn scrollbar(
    buf: &mut ratatui_core::buffer::Buffer,
    area: ratatui_core::layout::Rect,
    theme: &crate::theme::Theme,
    content: usize,
    position: usize,
    viewport: usize,
) {
    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget};
    let mut state = ScrollbarState::new(content)
        .position(position)
        .viewport_content_length(viewport);
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some(theme.symbols.scrollbar[0]))
        .thumb_symbol(theme.symbols.scrollbar[1])
        .track_style(theme.scrollbar())
        .thumb_style(theme.scrollbar_thumb())
        .render(area, buf, &mut state);
}

/// `label` as a pill: half-block ends in the fill color around ` label `.
/// Without a fill, the ends are blank.
pub(crate) fn pill(
    theme: &crate::theme::Theme,
    label: &str,
    style: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::{style::Style, text::Span};
    let [left, right] = theme.symbols.ends;
    let (left, right) = match style.bg {
        Some(fill) => (
            Span::styled(left, Style::new().fg(fill)),
            Span::styled(right, Style::new().fg(fill)),
        ),
        None => (Span::raw(" "), Span::raw(" ")),
    };
    vec![left, Span::styled(format!(" {label} "), style), right]
}
