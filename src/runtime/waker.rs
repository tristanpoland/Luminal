//! Custom waker implementation
//!
//! This module provides a custom waker implementation for the Luminal runtime.
//! The waker is responsible for notifying the runtime when a task is ready to make progress.

#[cfg(feature = "std")]
use std::{sync::Arc, task::{RawWaker, RawWakerVTable, Waker}, mem};

#[cfg(not(feature = "std"))]
use core::{task::{RawWaker, RawWakerVTable, Waker}, mem};

#[cfg(not(feature = "std"))]
use alloc::{sync::Arc, boxed::Box};

#[cfg(feature = "std")]
use crossbeam_deque::Injector;

#[cfg(not(feature = "std"))]
use heapless::Deque;

use super::task::{Task, TaskId};

/// Data structure containing the necessary information for waking tasks (std version)
///
/// This stores references to the task ID and global queue needed to
/// re-queue a task when it's woken.
#[cfg(feature = "std")]
#[repr(C, align(8))]
struct WakerData {
    /// The ID of the task this waker is associated with
    #[allow(dead_code)]
    task_id: TaskId,

    /// Reference to the global task queue for re-enqueueing
    #[allow(dead_code)]
    global_queue: Arc<Injector<Task>>,
}

/// Data structure containing the necessary information for waking tasks (no_std version)
///
/// This stores references to the task ID and global queue needed to
/// re-queue a task when it's woken.
#[cfg(not(feature = "std"))]
#[repr(C, align(8))]
struct WakerData {
    /// The ID of the task this waker is associated with
    #[allow(dead_code)]
    task_id: TaskId,

    /// Reference to the global task queue for re-enqueueing
    #[allow(dead_code)]
    global_queue: Arc<parking_lot::Mutex<Deque<Task, 1024>>>,
}

/// Creates a task waker for the Luminal runtime (std version)
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
#[cfg(feature = "std")]
pub(crate) fn create_task_waker(task_id: TaskId, global_queue: Arc<Injector<Task>>) -> Waker {
    // Define the vtable for our custom waker
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        // Clone function - safe pointer handling with alignment checks
        |data| {
            // Ensure proper alignment before dereferencing
            assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer in clone");
            
            let waker_data = unsafe { &*(data as *const WakerData) };
            let cloned_data = Box::into_raw(Box::new(WakerData {
                task_id: waker_data.task_id,
                global_queue: waker_data.global_queue.clone(),
            }));
            RawWaker::new(cloned_data as *const (), &VTABLE)
        },
        // Wake function
        |_data| {
            // Wake by re-queueing - task should already be queued
        },
        // Wake by ref function
        |_data| {
            // Wake by ref - task should already be queued  
        },
        // Drop function - safe pointer handling with alignment checks
        |data| {
            if !data.is_null() {
                // Ensure proper alignment before dereferencing
                assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer in drop");
                let _ = unsafe { Box::from_raw(data as *mut WakerData) };
            }
        },
    );
    
    // Create the waker data with proper alignment
    let data = Box::into_raw(Box::new(WakerData {
        task_id,
        global_queue,
    }));
    
    // Verify alignment before creating the waker
    assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer");
    
    // Create and return the waker
    unsafe { Waker::from_raw(RawWaker::new(data as *const (), &VTABLE)) }
}

/// Creates a task waker for the Luminal runtime (no_std version)
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
#[cfg(not(feature = "std"))]
pub(crate) fn create_task_waker(task_id: TaskId, global_queue: Arc<parking_lot::Mutex<Deque<Task, 1024>>>) -> Waker {
    // Define the vtable for our custom waker
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        // Clone function - safe pointer handling with alignment checks
        |data| {
            // Ensure proper alignment before dereferencing
            assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer in clone");

            let waker_data = unsafe { &*(data as *const WakerData) };
            let cloned_data = Box::into_raw(Box::new(WakerData {
                task_id: waker_data.task_id,
                global_queue: waker_data.global_queue.clone(),
            }));
            RawWaker::new(cloned_data as *const (), &VTABLE)
        },
        // Wake function
        |_data| {
            // Wake by re-queueing - task should already be queued
        },
        // Wake by ref function
        |_data| {
            // Wake by ref - task should already be queued
        },
        // Drop function - safe pointer handling with alignment checks
        |data| {
            if !data.is_null() {
                // Ensure proper alignment before dereferencing
                assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer in drop");
                let _ = unsafe { Box::from_raw(data as *mut WakerData) };
            }
        },
    );

    // Create the waker data with proper alignment
    let data = Box::into_raw(Box::new(WakerData {
        task_id,
        global_queue,
    }));

    // Verify alignment before creating the waker
    assert_eq!(data as usize % mem::align_of::<WakerData>(), 0, "Misaligned WakerData pointer");

    // Create and return the waker
    unsafe { Waker::from_raw(RawWaker::new(data as *const (), &VTABLE)) }
}
