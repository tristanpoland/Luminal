//! Error types for the Luminal runtime
//!
//! This module defines the error types that can occur during
//! task execution and management within the Luminal runtime.

#[cfg(feature = "std")]
use std::fmt;
#[cfg(not(feature = "std"))]
use core::fmt;

/// Errors that can occur when working with tasks in the runtime
///
/// These represent the various failure modes that can occur when 
/// scheduling, executing, or awaiting tasks.
#[derive(Debug)]
pub enum TaskError {
    /// The channel used to communicate with a task has been disconnected
    ///
    /// This typically occurs when a task or its handle is dropped
    /// before the task completes.
    Disconnected,
    
    /// A timeout occurred while waiting for a task to complete
    ///
    /// This can happen when using timeouts with task execution or
    /// when the runtime's block_on method times out.
    Timeout,
    
    /// The task panicked during execution
    ///
    /// This error is produced when a task panics during its execution.
    /// The runtime attempts to capture and propagate this error safely.
    Panicked,
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::Disconnected => write!(f, "Task channel disconnected"),
            TaskError::Timeout => write!(f, "Task timed out"),
            TaskError::Panicked => write!(f, "Task panicked"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TaskError {}
