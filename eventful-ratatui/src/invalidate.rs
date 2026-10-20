//! [`Invalidate`]: a dirty flag plus waker shared by everything on the UI
//! thread. `invalidate()` is cheap and coalesces any number of calls into one
//! frame.
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    task::{Context, Poll, Waker},
};

/// Frame state for the thread running the loop.
#[derive(Default)]
struct Frame {
    /// A redraw was requested since the last frame.
    dirty: Cell<bool>,
    /// The loop's waker, while it waits for a request.
    waker: RefCell<Option<Waker>>,
}

thread_local! {
    static FRAME: Frame = Frame::default();
}

/// Requests a redraw from the loop running on this thread.
///
/// Handlers never draw; they change state and call [`invalidate`]. The loop
/// draws at most once per frame interval however often this is called.
///
/// The flag is per thread, so use it from components on the shard that runs
/// [`crate::run_tui`]. The handle is not `Send`, which keeps it there.
///
/// [`invalidate`]: Self::invalidate
#[derive(Clone, Debug, Default)]
pub struct Invalidate {
    /// Bind the handle to the UI thread.
    _local: PhantomData<*const ()>,
}

impl Invalidate {
    /// Create a handle for this thread's loop.
    pub fn new() -> Self {
        Self::default()
    }
    /// Request a redraw.
    pub fn invalidate(&self) {
        mark();
    }
}

/// Mark this thread's frame dirty and wake the loop.
pub(crate) fn mark() {
    FRAME.with(|frame| {
        frame.dirty.set(true);
        if let Some(waker) = frame.waker.take() {
            waker.wake();
        }
    });
}

/// Clear the dirty flag before drawing, so requests made while drawing
/// schedule another frame.
pub(crate) fn clear() {
    FRAME.with(|frame| frame.dirty.set(false));
}

/// Ready once a redraw was requested. Registers the waker otherwise.
pub(crate) fn poll_dirty(cx: &mut Context<'_>) -> Poll<()> {
    FRAME.with(|frame| {
        if frame.dirty.get() {
            return Poll::Ready(());
        }
        *frame.waker.borrow_mut() = Some(cx.waker().clone());
        Poll::Pending
    })
}

/// Whether a redraw was requested.
pub(crate) fn is_dirty() -> bool {
    FRAME.with(|frame| frame.dirty.get())
}
