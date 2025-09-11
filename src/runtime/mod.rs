//! Luminal Runtime Core Implementation
//!
//! This module provides the core components of the Luminal async runtime,
//! designed as a DLL-boundary safe alternative to tokio with similar 
//! performance characteristics and API compatibility.
//!
//! ## Key Components
//!
//! - `Runtime`: Main runtime for executing async tasks
//! - `Handle`: Lightweight handle to a Runtime
//! - `JoinHandle`: Handle for awaiting the completion of async tasks
//! - `Executor`: Core task execution engine
//! - `Task`: Individual async task representation
//!
//! ## Module Structure
//!
//! The runtime is organized into several submodules:
//! - `error`: Error types specific to task execution
//! - `executor`: Core execution engine implementation
//! - `handle`: Runtime handle implementation
//! - `join_handle`: Join handle implementation
//! - `runtime`: Main runtime implementation
//! - `task`: Task representation
//! - `worker`: Worker thread implementation
//! - `waker`: Custom waker implementation

mod error;
mod executor;
mod handle;
mod join_handle;
mod runtime;
mod task;
mod worker;
mod waker;

// Re-export public components
pub use self::error::TaskError;
pub use self::executor::Executor;
pub use self::handle::Handle;
pub use self::join_handle::JoinHandle;
pub use self::runtime::Runtime;
pub use self::task::TaskId;

// Global convenience functions
pub use self::runtime::{spawn, block_on};
