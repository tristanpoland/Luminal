//! Runtime handle implementation
//!
//! This module provides the implementation of Handle, which is a lightweight
//! reference to a Runtime that can be used to spawn tasks and block on futures.

#[cfg(feature = "std")]
use std::{future::Future, sync::Arc};

#[cfg(not(feature = "std"))]
use core::future::Future;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

#[cfg(feature = "std")]
use super::executor::Executor;
#[cfg(not(feature = "std"))]
use super::simple_executor::SimpleExecutor as Executor;
use super::join_handle::JoinHandle;

/// A lightweight handle to a Runtime
///
/// Handle provides a way to interact with the runtime
/// without having to clone the entire Runtime structure.
/// It allows spawning tasks and blocking on futures.
pub struct Handle {
    /// The executor that this handle refers to
    pub(crate) executor: Arc<Executor>,
}

impl Handle {
    /// Creates a new handle to the provided executor
    ///
    /// # Parameters
    ///
    /// * `executor` - The executor this handle will use
    ///
    /// # Returns
    ///
    /// A new Handle instance
    pub(crate) fn new(executor: Arc<Executor>) -> Self {
        Handle { executor }
    }

    /// Spawns a future onto the runtime
    ///
    /// This method takes a future and begins executing it on the runtime,
    /// returning a JoinHandle that can be used to await its completion and
    /// retrieve its result.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The future type
    ///
    /// # Parameters
    ///
    /// * `future` - The future to execute
    ///
    /// # Returns
    ///
    /// A JoinHandle that can be used to await the future's completion
    ///
    /// # Examples
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let handle = rt.handle();
    /// 
    /// let task = handle.spawn(async {
    ///     println!("Running from a handle");
    ///     42
    /// });
    /// 
    /// let result = handle.block_on(task);
    /// assert_eq!(result, 42);
    /// ```
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.spawn(future)
    }

    /// Blocks the current thread until the provided future completes
    ///
    /// This method takes a future and blocks the current thread until it completes,
    /// helping process other tasks while waiting to avoid deadlocks.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The future type
    ///
    /// # Parameters
    ///
    /// * `future` - The future to execute and wait for
    ///
    /// # Returns
    ///
    /// The output of the future
    ///
    /// # Examples
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let handle = rt.handle();
    /// 
    /// let result = handle.block_on(async {
    ///     println!("Blocking using a handle");
    ///     42
    /// });
    /// 
    /// assert_eq!(result, 42);
    /// ```
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.block_on(future)
    }
}

impl Clone for Handle {
    /// Creates a new handle referring to the same executor
    ///
    /// This creates a lightweight clone that shares the same
    /// underlying executor as the original handle.
    ///
    /// # Returns
    ///
    /// A new Handle instance referring to the same executor
    fn clone(&self) -> Self {
        Handle {
            executor: self.executor.clone(),
        }
    }
}
