//! The event interfaces components emit. Every method has an empty default,
//! so a receiver implements only what it needs; or connect single signals to
//! methods with `with_receiver(&receiver).connect(Type::method)`, as in the
//! example of the [module docs](super).
//!
//! Each trait's methods become signals on the components that emit it, via
//! the generated `{Trait}SignalsExt` traits, like `button.clicked()`.
//! Delivery is queued: a connected method runs when the UI shard next runs,
//! not inside the input handler.
#![allow(missing_docs)] // The generated signal types document themselves.

/// Emitted by [`Button`](super::Button).
#[eventful_rs::events]
pub trait ButtonEvents {
    /// The button was clicked, or activated with Enter or Space.
    fn clicked(&self) {}
}

/// Emitted by [`Checkbox`](super::Checkbox), [`Switch`](super::Switch) and
/// [`Chip`](super::Chip).
#[eventful_rs::events]
pub trait ToggleEvents {
    /// The user changed the state; `on` is the new state.
    fn toggled(&self, on: bool) {}
}

/// Emitted by components that select from items.
///
/// Those are [`RadioGroup`](super::RadioGroup), [`List`](super::List),
/// [`Tabs`](super::Tabs), [`Table`](super::Table) and
/// [`Select`](super::Select). Indices count from 0 in the order the items
/// were added. Only user actions emit; setting the selection from code does
/// not.
#[eventful_rs::events]
pub trait SelectEvents {
    /// The user selected the item at `index`.
    fn selected(&self, index: usize) {}
    /// The user activated the item at `index`: Enter or a double click.
    /// Lists and tables emit it; the others only select.
    fn activated(&self, index: usize) {}
}

/// Emitted by text inputs: [`TextInput`](super::TextInput),
/// [`TextArea`](super::TextArea) and [`ComboBox`](super::ComboBox).
#[eventful_rs::events]
pub trait TextEvents {
    /// The user changed the text; `text` is all of it, lines joined by `\n`
    /// in a text area.
    fn changed(&self, text: String) {}
    /// The user pressed Enter, or accepted a suggestion of a combo box. A
    /// text area, where Enter starts a new line, does not emit it.
    fn submitted(&self, text: String) {}
}

/// Emitted by components holding a number: [`Slider`](super::Slider) and
/// [`SpinBox`](super::SpinBox).
#[eventful_rs::events]
pub trait ValueEvents {
    /// The user changed the value; `value` is the new one.
    fn changed(&self, value: f64) {}
}

/// Emitted by components that trigger named actions:
/// [`MenuBar`](super::MenuBar) and [`Breadcrumb`](super::Breadcrumb).
#[eventful_rs::events]
pub trait ActionEvents {
    /// The user triggered the action with this id, as given to the item.
    fn triggered(&self, id: String) {}
}

/// Emitted by [`Tree`](super::Tree).
#[eventful_rs::events]
pub trait TreeEvents {
    /// The user selected the node with this id.
    fn selected(&self, id: String) {}
    /// The user activated the node with this id: Enter or a double click.
    fn activated(&self, id: String) {}
    /// The user expanded (`true`) or collapsed the node with this id.
    fn toggled(&self, id: String, expanded: bool) {}
}
