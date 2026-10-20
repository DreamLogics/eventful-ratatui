//! `Invalidate`: a shard-local dirty flag plus waker. `invalidate()` is cheap
//! and coalesces any number of calls into one frame.
//!
//! Phase 2.
