//! `run_tui(root)`, the `Tui` builder and the `Quit` handle. The loop is the
//! `#[sharded_main]` future: it selects over input, invalidation and an
//! optional tick, and draws at most once per frame interval.
//!
//! `run_tui_with(backend, stream)` takes an injected backend and input stream
//! so tests run against `TestBackend` without a terminal.
//!
//! Phase 2.
