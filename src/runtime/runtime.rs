//! Main runtime implementation
//!
//! This module provides the Runtime implementation, which is the main entry point
//! for using the Luminal async runtime. It also provides global convenience functions
//! for spawning tasks and blocking on futures.

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
use super::handle::Handle;
use super::join_handle::JoinHandle;

/// Main runtime for executing async tasks
///
/// The Runtime is the central coordination point for the Luminal async runtime.
/// It provides methods for spawning tasks, blocking on futures, and managing
/// the runtime itself. Unlike tokio, this runtime is safe to pass across
/// DLL boundaries as it doesn't rely on thread-local storage.
pub struct Runtime {
    /// The underlying executor that schedules and runs tasks
    executor: Arc<Executor>,
}

impl Runtime {
    /// Creates a new Luminal runtime
    ///
    /// This initializes a new multi-threaded runtime with a work-stealing scheduler
    /// using the number of available CPU cores. The runtime will create worker
    /// threads and begin processing the task queue immediately.
    ///
    /// # Returns
    /// 
    /// A new Runtime instance wrapped in a Result
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime could not be initialized
    ///
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     println!("Running on Luminal runtime!");
    /// });
    /// ```
    pub fn new() -> Result<Self, crate::error::RuntimeError> {
        Ok(Runtime {
            executor: Arc::new(Executor::new()),
        })
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
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let handle = rt.spawn(async {
    ///     // Some async work
    ///     42
    /// });
    ///
    /// let result = rt.block_on(handle); // Waits for the result
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
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let result = rt.block_on(async {
    ///     // Some async work
    ///     42
    /// });
    /// assert_eq!(result, 42);
    /// ```
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.block_on(future)
    }

    /// Returns a Handle to this runtime
    ///
    /// The Handle provides a lightweight way to interact with the runtime
    /// without cloning the entire Runtime.
    ///
    /// # Returns
    ///
    /// A new Handle to this runtime
    ///
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let handle = rt.handle();
    ///
    /// // Use the handle to spawn tasks
    /// let task = handle.spawn(async { 42 });
    /// ```
    pub fn handle(&self) -> Handle {
        Handle::new(self.executor.clone())
    }
    
    /// Returns statistics about the runtime
    ///
    /// # Returns
    ///
    /// A tuple containing the current queue length and the number of tasks processed
    ///
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    /// let (queue_len, tasks_processed) = rt.stats();
    /// println!("Queue length: {}, Tasks processed: {}", queue_len, tasks_processed);
    /// ```
    pub fn stats(&self) -> (usize, usize) {
        self.executor.stats()
    }
}

impl Clone for Runtime {
    /// Creates a new runtime referring to the same executor
    ///
    /// This allows for lightweight cloning of the runtime, as the
    /// underlying executor is reference-counted.
    ///
    /// # Returns
    ///
    /// A new Runtime instance referring to the same executor
    fn clone(&self) -> Self {
        Runtime {
            executor: self.executor.clone(),
        }
    }
}

#[cfg(feature = "std")]
mod thread_local_support {
    use super::*;

    // Thread-local runtime for global convenience functions
    thread_local! {
        /// Thread-local runtime for global convenience functions
        ///
        /// While Luminal generally avoids thread-local storage for its core functionality
        /// to ensure DLL boundary safety, these convenience functions use a thread-local
        /// runtime for ease of use when DLL boundary safety isn't a concern.
        static THREAD_RUNTIME: std::cell::RefCell<Option<Runtime>> = std::cell::RefCell::new(None);
    }

    /// Lazily initializes the thread-local runtime if needed and executes the given function with it
    fn with_thread_local_runtime<F, R>(f: F) -> R
    where
        F: FnOnce(&Runtime) -> R
    {
        THREAD_RUNTIME.with(|cell| {
            if cell.borrow().is_none() {
                // Initialize the runtime if it doesn't exist yet
                let rt = Runtime::new().expect("Failed to initialize thread-local runtime");
                *cell.borrow_mut() = Some(rt);
            }

            // Execute the function with a reference to the runtime
            f(cell.borrow().as_ref().unwrap())
        })
    }

    /// Spawns a future onto the current thread's runtime
    ///
    /// This is a convenience function that uses a thread-local runtime.
    /// For DLL boundary safety, create and use an explicit Runtime instead.
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
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// // Create an explicit runtime instead of using thread locals for doctests
    /// let rt = Runtime::new().unwrap();
    /// let handle = rt.spawn(async {
    ///     // Some async work
    ///     42
    /// });
    /// ```
    pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        with_thread_local_runtime(|rt| rt.spawn(future))
    }

    /// Blocks the current thread until the provided future completes
    ///
    /// This is a convenience function that uses a thread-local runtime.
    /// For DLL boundary safety, create and use an explicit Runtime instead.
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
    /// # Example
    ///
    /// ```
    /// use luminal::Runtime;
    ///
    /// // Create an explicit runtime instead of using thread locals for doctests
    /// let rt = Runtime::new().unwrap();
    /// let result = rt.block_on(async {
    ///     // Some async work
    ///     42
    /// });
    /// assert_eq!(result, 42);
    /// ```
    pub fn block_on<F>(future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        with_thread_local_runtime(|rt| rt.block_on(future))
    }
}

/// Spawns a future onto the current thread's runtime
///
/// This is a convenience function that uses a thread-local runtime.
/// For DLL boundary safety, create and use an explicit Runtime instead.
///
/// This function is only available when the `std` feature is enabled.
#[cfg(feature = "std")]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread_local_support::spawn(future)
}

/// Blocks the current thread until the provided future completes
///
/// This is a convenience function that uses a thread-local runtime.
/// For DLL boundary safety, create and use an explicit Runtime instead.
///
/// This function is only available when the `std` feature is enabled.
#[cfg(feature = "std")]
pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread_local_support::block_on(future)
}
