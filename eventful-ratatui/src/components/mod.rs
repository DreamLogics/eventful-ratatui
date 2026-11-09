//! Ready-made components: buttons, inputs, lists, tabs, panels, progress,
//! and more. Enabled by the default `components` feature.
//!
//! Every component is a cheap `Clone` handle: keep one in your own component,
//! render it with `render`, and read or change its state through methods.
//! Handles record themselves in the render tree, so focus, Tab order and mouse
//! input work without extra code. Draw styles come from the current
//! [`Theme`](crate::theme::Theme).
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
mod button;
mod events;
mod key_hints;
mod label;
mod list;
mod log_view;
mod panel;
mod progress;
mod radio;
mod scroll_view;
mod stack;
mod tabs;
mod text_input;
mod toggle;

pub use button::{Button, ButtonKind};
pub use events::*;
pub use key_hints::KeyHints;
pub use label::Label;
pub use list::List;
pub use log_view::LogView;
pub use panel::Panel;
pub use progress::{ProgressBar, Spinner};
pub use radio::RadioGroup;
pub use scroll_view::ScrollView;
pub use stack::Stack;
pub use tabs::Tabs;
pub use text_input::TextInput;
pub use toggle::{Checkbox, Switch};

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
