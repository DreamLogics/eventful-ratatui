//! Procedural macros for eventful-ratatui.
//!
//! Phase 4: `#[key_events]`, a key-binding bridge that expands to
//! `#[eventful_rs::events]` on the trait plus a `{Trait}Bridge` with a
//! synchronous `dispatch(&KeyEvent) -> bool`, built on the generated
//! `{Trait}Emissions` type.
