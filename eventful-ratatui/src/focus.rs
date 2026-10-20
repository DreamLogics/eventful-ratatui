//! `FocusChain`: an ordered path of `ShardWeak<dyn Component>`. Input goes to
//! the focused leaf and bubbles up while it is not handled. Calls are direct,
//! not queued events, so bubbling gets an immediate answer.
//!
//! Phase 3.
