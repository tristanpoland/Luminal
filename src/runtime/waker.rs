//! Custom waker implementation
//!
//! This module provides a custom waker implementation for the Luminal runtime.
//! The waker is responsible for notifying the runtime when a task is ready to make progress.

use std::sync::Arc;
use std::task::{RawWaker, RawWakerVTable, Waker};
use crossbeam_deque::Injector;

use super::task::{Task, TaskId};

/// Data structure containing the necessary information for waking tasks
///
/// This stores references to the task ID and global queue needed to
/// re-queue a task when it's woken.
struct WakerData {
    /// The ID of the task this waker is associated with
    #[allow(dead_code)]
    task_id: TaskId,
    
    /// Reference to the global task queue for re-enqueueing
    #[allow(dead_code)]
    global_queue: Arc<Injector<Task>>,
}

/// Creates a task waker for the Luminal runtime
///
/// This function creates a custom waker that will re-queue tasks into the
/// global queue when they are woken. This is a crucial part of the runtime's
/// task scheduling system.
///
/// # Parameters
///
/// * `task_id` - The ID of the task this waker is for
/// * `global_queue` - Reference to the global task queue for re-enqueueing
///
/// # Returns
///
/// A `Waker` that can be used to wake the specified task
pub(crate) fn create_task_waker(task_id: TaskId, global_queue: Arc<Injector<Task>>) -> Waker {
    // Define the vtable for our custom waker
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        // Clone function
        |data| {
            let data = unsafe { &*(data as *const WakerData) };
            RawWaker::new(data as *const _ as *const (), &VTABLE)
        },
        // Wake function
        |_data| {
            // Wake by re-queueing - task should already be queued
        },
        // Wake by ref function
        |_data| {
            // Wake by ref - task should already be queued  
        },
        // Drop function
        |data| {
            let _ = unsafe { Box::from_raw(data as *mut WakerData) };
        },
    );
    
    // Create the waker data and convert it to a raw pointer
    let data = Box::into_raw(Box::new(WakerData {
        task_id,
        global_queue,
    }));
    
    // Create and return the waker
    unsafe { Waker::from_raw(RawWaker::new(data as *const (), &VTABLE)) }
}
