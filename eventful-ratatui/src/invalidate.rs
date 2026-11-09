//! [`Invalidate`]: a dirty flag plus waker shared by everything on the UI
//! thread. `invalidate()` is cheap and coalesces any number of calls into one
//! frame.
use futures::FutureExt;
use futures_timer::Delay;
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

/// Frame state for the thread running the loop.
#[derive(Default)]
struct Frame {
    /// A redraw was requested since the last frame.
    dirty: Cell<bool>,
    /// The loop's waker, while it waits for a request.
    waker: RefCell<Option<Waker>>,
    /// The earliest frame requested with [`after`].
    deadline: Cell<Option<Instant>>,
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
    /// Request a redraw once `delay` has passed. See [`after`].
    pub fn invalidate_after(&self, delay: Duration) {
        after(delay);
    }
}

/// Request a redraw from the loop running on this thread.
pub fn invalidate() {
    mark();
}

/// Request a redraw once `delay` has passed, for animations and timeouts:
/// a spinner asks for its next frame while rendering. Requests coalesce; the
/// loop keeps only the earliest.
pub fn after(delay: Duration) {
    let at = Instant::now() + delay;
    FRAME.with(|frame| {
        if frame.deadline.get().is_none_or(|deadline| at < deadline) {
            frame.deadline.set(Some(at));
            if let Some(waker) = frame.waker.take() {
                waker.wake();
            }
        }
    });
}

/// Ready once the deadline set with [`after`] has passed, which clears it.
/// `timer` holds the loop's delay between polls.
pub(crate) fn poll_deadline(
    cx: &mut Context<'_>,
    timer: &mut Option<(Instant, Delay)>,
) -> Poll<()> {
    let Some(deadline) = FRAME.with(|frame| frame.deadline.get()) else {
        *timer = None;
        return Poll::Pending;
    };
    let now = Instant::now();
    if deadline > now {
        if timer.as_ref().is_none_or(|(at, _)| *at != deadline) {
            *timer = Some((deadline, Delay::new(deadline - now)));
        }
        if let Some((_, delay)) = timer
            && delay.poll_unpin(cx).is_pending()
        {
            return Poll::Pending;
        }
    }
    FRAME.with(|frame| frame.deadline.set(None));
    *timer = None;
    Poll::Ready(())
}

/// Fire a pending [`after`] deadline now.
pub(crate) fn fire_deadline() {
    if FRAME.with(|frame| frame.deadline.take()).is_some() {
        mark();
    }
}

/// Forget pending timers, for a new loop run.
pub(crate) fn reset() {
    FRAME.with(|frame| {
        frame.deadline.set(None);
        frame.dirty.set(false);
    });
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

/// Ready once a redraw was requested. Registers the waker otherwise, which
/// [`after`] also wakes.
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
