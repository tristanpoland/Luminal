//! # BUST Runtime
//!
//! BUST (Burst Oriented Response Enhancer) is an async runtime designed to solve
//! tokio's DLL boundary issues while maintaining similar performance and API compatibility.
//!
//! ## Key Features
//!
//! - **DLL Boundary Safe**: Unlike tokio, BUST doesn't rely on thread-local storage, making it 100% safe to pass across DLL boundaries
//! - **Explicit Handle Passing**: All runtime context is explicit rather than implicit via TLS
//! - **Drop-in Replacement**: Provides tokio-compatible APIs like `spawn`, `block_on`, and `JoinHandle`
//! - **Cross-Platform**: Works on Windows, Linux, and macOS
//!
//! ## Basic Usage
//!
//! ```rust
//! use bust::Runtime;
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
//! use bust::Runtime;
//!
//! fn main() {
//!     let rt = Runtime::new().unwrap();
//!     rt.block_on(async {
//!         println!("Running on BUST runtime!");
//!     });
//! }
//! ```

pub mod runtime;

pub use runtime::{
    spawn, block_on, Runtime, Handle, JoinHandle, Executor
};

/// Error types for the BUST runtime
pub mod error {
    use std::fmt;

    #[derive(Debug)]
    pub enum RuntimeError {
        TaskQueueFull,
        RuntimeNotInitialized,
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

    impl std::error::Error for RuntimeError {}
}