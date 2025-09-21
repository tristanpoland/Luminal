//! # Luminal
//!
//! Luminal is a high-performance async runtime designed to solve
//! tokio's DLL boundary issues while maintaining similar performance and API compatibility.
//!
//! ## Key Features
//!
//! - **DLL Boundary Safe**: Unlike tokio, Luminal doesn't rely on thread-local storage, making it 100% safe to pass across DLL boundaries
//! - **Explicit Handle Passing**: All runtime context is explicit rather than implicit via TLS
//! - **Drop-in Replacement**: Provides tokio-compatible APIs like `spawn`, `block_on`, and `JoinHandle`
//! - **Cross-Platform**: Works on Windows, Linux, and macOS
//! - **Multi-threaded**: Uses a work-stealing scheduler with multiple worker threads for optimal CPU utilization
//! - **Efficient Work Stealing**: Implements a sophisticated work-stealing algorithm to distribute tasks evenly across worker threads
//! - **Memory Efficient**: Minimizes allocations and memory overhead in the task scheduling system
//! - **No-std Support**: Can run in `no_std` environments with `alloc` for embedded and constrained systems
//!
//! ## Basic Usage
//!
//! ```rust
//! use luminal::Runtime;
//!
//! async fn hello_world() {
//!     println!("Hello, world!");
//! }
//!
//! fn main() {
//!     let rt = Runtime::new().unwrap();
//!     let rt_clone = rt.clone();
//!     rt.block_on(async move {
//!         rt_clone.spawn(hello_world()).await;
//!     });
//! }
//! ```
//!
//! ## Explicit Runtime Usage
//!
//! ```rust
//! use luminal::Runtime;
//!
//! fn main() {
//!     let rt = Runtime::new().unwrap();
//!     rt.block_on(async {
//!         println!("Running on Luminal runtime!");
//!     });
//! }
//! ```
//!
//! ## DLL Boundary Safety
//!
//! Unlike tokio, which uses thread-local storage for its runtime context, Luminal uses explicit context passing.
//! This makes it safe to use across DLL boundaries:
//!
//! ```rust
//! // Inside a DLL
//! fn dll_function(runtime: luminal::Runtime) -> u32 {
//!     // Safe to use the runtime passed from outside
//!     runtime.block_on(async { 42 })
//! }
//!
//! // From the main application
//! fn main() {
//!     let rt = luminal::Runtime::new().unwrap();
//!     let result = dll_function(rt.clone());
//!     assert_eq!(result, 42);
//! }
//! ```
#![cfg_attr(not(feature = "std"), no_std)]


#[cfg(not(feature = "std"))]
extern crate alloc;

// Re-export core components
pub mod runtime;

// Main runtime components
pub use runtime::{
    Runtime, Handle, JoinHandle, Executor,
};

// Global convenience functions (std only)
#[cfg(feature = "std")]
pub use runtime::{spawn, block_on};

// Error types for the Luminal runtime
pub use error::RuntimeError;

/// Error types for the Luminal runtime
pub mod error {
    #[cfg(feature = "std")]
    use std::fmt;
    #[cfg(not(feature = "std"))]
    use core::fmt;

    #[cfg(not(feature = "std"))]
    use alloc::string::String;

    /// Errors that can occur when using the Luminal runtime
    #[derive(Debug)]
    pub enum RuntimeError {
        /// The task queue is full and cannot accept more tasks
        TaskQueueFull,

        /// The runtime has not been properly initialized
        RuntimeNotInitialized,

        /// A task has panicked during execution
        ///
        /// Contains the panic message if available
        TaskPanic(String),
    }

    impl fmt::Display for RuntimeError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                RuntimeError::TaskQueueFull => write!(f, "Task queue is full"),
                RuntimeError::RuntimeNotInitialized => write!(f, "Runtime not initialized"),
                RuntimeError::TaskPanic(msg) => write!(f, "Task panicked: {}", msg),
            }
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for RuntimeError {}
}