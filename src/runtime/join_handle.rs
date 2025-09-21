//! Join handle implementation
//!
//! This module provides the implementation of `JoinHandle`,
//! which is used to await the completion of async tasks.

#[cfg(feature = "std")]
use std::{future::Future, pin::Pin, task::{Context, Poll}};

#[cfg(not(feature = "std"))]
use core::{future::Future, pin::Pin, task::{Context, Poll}};

#[cfg(feature = "std")]
use crossbeam_channel::{Receiver, TryRecvError};

#[cfg(not(feature = "std"))]
use heapless::mpmc::MpMcQueue;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

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
/// use luminal::Runtime;
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
#[cfg(feature = "std")]
pub struct JoinHandle<T> {
    /// The unique identifier of the task
    #[allow(dead_code)]
    pub(crate) id: TaskId,

    /// Channel for receiving the task's result when it completes
    pub(crate) receiver: Receiver<T>,
}

/// Handle for awaiting the completion of an asynchronous task (no_std version)
///
/// Similar to tokio's JoinHandle, this allows waiting for a task to complete
/// and retrieving its result. It implements `Future` so it can be awaited
/// in async contexts.
///
/// In no_std environments, this uses a bounded queue with a default capacity of 16.
#[cfg(not(feature = "std"))]
pub struct JoinHandle<T> {
    /// The unique identifier of the task
    #[allow(dead_code)]
    pub(crate) id: TaskId,

    /// Channel for receiving the task's result when it completes
    /// Uses a shared bounded queue for no_std environments
    pub(crate) receiver: Arc<MpMcQueue<T, 16>>,
}

#[cfg(feature = "std")]
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

#[cfg(not(feature = "std"))]
impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    /// Poll method implementation for JoinHandle (no_std version)
    ///
    /// This checks if the result is ready by attempting to receive
    /// from the heapless queue. If the result is available, it's returned as Ready.
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
        #[cfg(feature = "std")]
        use std::task::Poll;
        #[cfg(not(feature = "std"))]
        use core::task::Poll;

        // Try to dequeue a result from the bounded queue
        match self.receiver.dequeue() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
