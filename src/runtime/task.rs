//! Task representation and management
//!
//! This module defines the core task structures used in the Luminal runtime,
//! including task identifiers and the task structure itself.

#[cfg(feature = "std")]
use std::{future::Future, pin::Pin, sync::atomic::{AtomicU64, Ordering}, task::{Context, Poll}};

#[cfg(not(feature = "std"))]
use core::{future::Future, pin::Pin, sync::atomic::{AtomicU64, Ordering}, task::{Context, Poll}};

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

/// Type alias for a boxed future that can be sent across threads
///
/// This represents a task's future that has been pinned and boxed
/// to allow for dynamic dispatch and safe sending across thread boundaries.
pub type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A unique identifier for tasks within the runtime
///
/// Each task is assigned a unique ID when it's created, which is used
/// for tracking and managing the task throughout its lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub(crate) u64);

impl TaskId {
    /// Creates a new unique task ID
    ///
    /// Uses an atomic counter to ensure uniqueness across threads.
    /// This provides a reliable way to identify tasks even in a 
    /// multithreaded environment.
    ///
    /// # Returns
    ///
    /// A new unique `TaskId`
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
    
    /// Gets the raw ID value
    ///
    /// # Returns
    ///
    /// The underlying u64 identifier
    #[inline]
    pub fn get(&self) -> u64 {
        self.0
    }
}

/// Represents an async task that can be scheduled and executed by the runtime
///
/// Contains the task's unique identifier and its underlying future.
/// This is the core unit of work within the Luminal runtime.
pub struct Task {
    /// The unique identifier for this task
    pub(crate) id: TaskId,
    
    /// The actual future that will be executed
    pub(crate) future: BoxFuture,
}

impl Task {
    /// Creates a new task with the given ID and future
    ///
    /// # Parameters
    ///
    /// * `id` - Unique identifier for the task
    /// * `future` - The future to be executed as part of this task
    ///
    /// # Returns
    ///
    /// A new `Task` instance
    pub fn new(id: TaskId, future: BoxFuture) -> Self {
        Self { id, future }
    }
    
    /// Polls the task's future, advancing its execution
    ///
    /// This is the core method used by the runtime to make progress on tasks.
    /// It calls the underlying future's poll method with the given context.
    ///
    /// # Parameters
    ///
    /// * `cx` - The poll context, containing the waker
    ///
    /// # Returns
    ///
    /// `Poll::Ready(())` when the future completes, or `Poll::Pending` if it's not ready yet.
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

impl Default for Task {
    /// Creates a default task with a completed future
    ///
    /// This is used as a placeholder when taking ownership of tasks
    /// to avoid memory alignment issues.
    fn default() -> Self {
        Self {
            id: TaskId::new(),
            future: Box::pin(async {}),
        }
    }
}
