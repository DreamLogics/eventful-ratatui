//! The event interfaces components emit. Every method has an empty default,
//! so a receiver implements only what it needs; or connect single signals to
//! methods with `with_receiver(&receiver).connect(Type::method)`.
#![allow(missing_docs)] // The generated signal types document themselves.

/// Emitted by [`Button`](super::Button).
#[eventful_rs::events]
pub trait ButtonEvents {
    /// The button was clicked, or activated with Enter or Space.
    fn clicked(&self) {}
}

/// Emitted by [`Checkbox`](super::Checkbox) and [`Switch`](super::Switch).
#[eventful_rs::events]
pub trait ToggleEvents {
    /// The user changed the state; `on` is the new state.
    fn toggled(&self, on: bool) {}
}

/// Emitted by components that select from items: radio groups, lists, tabs,
/// tables, trees, selects and menus.
#[eventful_rs::events]
pub trait SelectEvents {
    /// The user selected the item at `index`.
    fn selected(&self, index: usize) {}
    /// The user activated the item at `index`: Enter or a double click.
    fn activated(&self, index: usize) {}
}

/// Emitted by text inputs.
#[eventful_rs::events]
pub trait TextEvents {
    /// The user changed the text.
    fn changed(&self, text: String) {}
    /// The user pressed Enter.
    fn submitted(&self, text: String) {}
}
