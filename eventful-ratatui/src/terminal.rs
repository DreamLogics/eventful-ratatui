//! `TerminalGuard` and the panic hook.
//!
//! The guard restores the terminal on drop, including while a panic in the
//! main future unwinds. The hook replaces `ratatui::init()`'s, which would
//! restore the terminal on any panic, including callback panics that
//! eventful-rs isolates. Unwind builds only record the message; `panic =
//! "abort"` builds restore the terminal from the hook. Callback panics reach
//! the in-app log through `eventful_rs::set_panic_reporter`.
//!
//! Phase 2.
