//! Join handle implementation
//!
//! This module provides the implementation of `JoinHandle`,
//! which is used to await the completion of async tasks.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crossbeam_channel::{Receiver, TryRecvError};

use super::task::TaskId;

/// Handle for awaiting the completion of an asynchronous task
///
/// Similar to tokio's JoinHandle, this allows waiting for a task to complete
/// and retrieving its result. It implements `Future` so it can be awaited
/// in async contexts.
///
/// # Type Parameters
///
/// * `T` - The output type of the task
///
/// # Examples
///
/// ```
/// use bust::Runtime;
///
/// let rt = Runtime::new().unwrap();
/// let handle = rt.spawn(async { 42 });
///
/// // Await the handle
/// let result = rt.block_on(async {
///     handle.await
/// });
/// assert_eq!(result, 42);
/// ```
pub struct JoinHandle<T> {
    /// The unique identifier of the task
    pub(crate) id: TaskId,
    
    /// Channel for receiving the task's result when it completes
    pub(crate) receiver: Receiver<T>,
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    /// Poll method implementation for JoinHandle
    ///
    /// This checks if the result is ready by attempting to receive
    /// from the channel. If the result is available, it's returned as Ready.
    /// If not, it returns Pending to be polled again later.
    ///
    /// # Parameters
    ///
    /// * `self` - Pinned mutable reference to self
    /// * `_cx` - Context for the poll (not used in this implementation)
    ///
    /// # Returns
    ///
    /// `Poll::Ready(T)` when the task completes, or `Poll::Pending` if still running
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.try_recv() {
            Ok(result) => Poll::Ready(result),
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => {
                // Channel disconnected - likely due to panic or dropped task
                // For safety we could abort, but that would break API
                // Instead log warning and return a default/placeholder
                eprintln!("WARNING: Task channel disconnected unexpectedly");
                // Keep pending to allow task completion or timeout
                Poll::Pending
            },
        }
    }
}
