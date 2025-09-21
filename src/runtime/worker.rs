//! Worker thread implementation
//!
//! This module provides the implementation of worker threads,
//! which are responsible for executing tasks in the Luminal runtime.

#[cfg(feature = "std")]
use std::{sync::atomic::{AtomicBool, AtomicUsize, Ordering}, sync::Arc, task::Context, thread, time::Duration};

#[cfg(not(feature = "std"))]
use core::{sync::atomic::{AtomicBool, AtomicUsize, Ordering}, task::Context};

#[cfg(not(feature = "std"))]
use alloc::{sync::Arc, vec::Vec};

use crossbeam_deque::{Injector, Stealer, Worker, Steal};

use super::task::Task;
use super::waker::create_task_waker;

/// A worker thread that processes tasks from local and global queues
///
/// Each worker maintains its own local queue and can steal work from other workers
/// or the global queue when its local queue is empty. This is the foundation of
/// the work-stealing scheduler used in Luminal.
pub(crate) struct WorkerThread {
    /// Unique ID for this worker
    #[allow(dead_code)]
    pub(crate) id: usize,
    
    /// Local task queue for this worker
    pub(crate) worker: Worker<Task>,
    
    /// References to other workers' queues for work stealing
    pub(crate) stealers: Vec<Stealer<Task>>,
    
    /// Reference to the global task queue
    pub(crate) global_queue: Arc<Injector<Task>>,
    
    /// Flag indicating whether the worker should shut down
    pub(crate) shutdown: Arc<AtomicBool>,
    
    /// Counter for the number of tasks processed by this worker
    pub(crate) tasks_processed: Arc<AtomicUsize>,
}

impl WorkerThread {
    /// The main execution loop for the worker thread
    ///
    /// This method continuously processes tasks from its local queue,
    /// stealing from other workers when necessary, and backing off when
    /// no work is available to reduce CPU usage.
    ///
    /// Note: In no_std environments, this provides basic task processing
    /// without thread sleep/yield functionality.
    #[cfg(feature = "std")]
    pub fn run(self) {
        let mut idle_count = 0u32;
        
        loop {
            // Check if shutdown signal has been received
            if self.shutdown.load(Ordering::Acquire) {
                break;
            }
            
            let mut found_work = false;
            
            // Process local queue first (LIFO for better cache locality)
            while let Some(mut task) = self.worker.pop() {
                self.run_task(&mut task);
                found_work = true;
                idle_count = 0;
            }
            
            // Try to steal work if no local work
            if !found_work {
                found_work = self.steal_work();
            }
            
            // Optimized backoff strategy for better performance
            if !found_work {
                idle_count += 1;
                if idle_count > 100 {
                    // More aggressive sleeping to reduce CPU overhead
                    if idle_count > 10000 {
                        thread::sleep(Duration::from_micros(1000));
                    } else if idle_count > 1000 {
                        thread::sleep(Duration::from_micros(10));
                    } else {
                        thread::yield_now();
                    }
                } else if idle_count > 10 {
                    thread::yield_now();
                }
                // Busy wait for first 10 iterations for better latency
            }
        }
    }

    /// The main execution loop for the worker thread (no_std version)
    ///
    /// This provides a simplified execution loop that doesn't use thread-specific
    /// functions like sleep or yield_now, making it suitable for no_std environments.
    #[cfg(not(feature = "std"))]
    pub fn run(self) {
        loop {
            // Check if shutdown signal has been received
            if self.shutdown.load(Ordering::Acquire) {
                break;
            }

            let mut found_work = false;

            // Process local queue first (LIFO for better cache locality)
            while let Some(mut task) = self.worker.pop() {
                self.run_task(&mut task);
                found_work = true;
            }

            // Try to steal work if no local work
            if !found_work {
                found_work = self.steal_work();
            }

            // In no_std environments, we can't sleep/yield, so we just continue
            // This may result in higher CPU usage but maintains compatibility
        }
    }
    
    /// Executes a single task
    ///
    /// This method creates a waker for the task, polls it, and handles
    /// the result appropriately. If the task is not yet complete, it's
    /// re-queued for later execution.
    ///
    /// # Parameters
    ///
    /// * `task` - The task to execute
    pub fn run_task(&self, task: &mut Task) {
        // Create a waker that will re-queue the task when woken
        let waker = create_task_waker(task.id, self.global_queue.clone());
        let mut cx = Context::from_waker(&waker);
        
        #[cfg(feature = "std")]
        use std::task::Poll;
        #[cfg(not(feature = "std"))]
        use core::task::Poll;

        match task.poll(&mut cx) {
            Poll::Ready(()) => {
                // Task completed successfully, increment the counter
                self.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            Poll::Pending => {
                // Re-queue the task for later execution without unsafe operations
                // Take ownership of the task to avoid memory issues
                #[cfg(feature = "std")]
                let owned_task = std::mem::take(task);
                #[cfg(not(feature = "std"))]
                let owned_task = core::mem::take(task);
                self.global_queue.push(owned_task);
            }
        }
    }
    
    /// Attempts to steal work from other workers or the global queue
    ///
    /// This implements an optimized work-stealing algorithm that reduces
    /// contention and improves cache locality for better performance.
    ///
    /// # Returns
    ///
    /// `true` if work was successfully stolen, `false` otherwise
    pub fn steal_work(&self) -> bool {
        // Try global queue first with optimized batch stealing
        match self.global_queue.steal_batch_and_pop(&self.worker) {
            crossbeam_deque::Steal::Success(task) => {
                // Execute the task immediately for better cache locality
                let mut task = task;
                self.run_task(&mut task);
                return true;
            }
            crossbeam_deque::Steal::Empty => {}
            crossbeam_deque::Steal::Retry => {
                // Quick retry on contention
                match self.global_queue.steal() {
                    crossbeam_deque::Steal::Success(mut task) => {
                        self.run_task(&mut task);
                        return true;
                    }
                    _ => {}
                }
            }
        }
        
        // Optimized worker stealing with round-robin approach
        let start_idx = self.id % self.stealers.len();
        for i in 0..self.stealers.len() {
            let idx = (start_idx + i) % self.stealers.len();
            if idx == self.id { continue; } // Skip self
            
            let stealer = &self.stealers[idx];
            match stealer.steal_batch_and_pop(&self.worker) {
                crossbeam_deque::Steal::Success(task) => {
                    // Execute immediately for better performance
                    let mut task = task;
                    self.run_task(&mut task);
                    return true;
                }
                crossbeam_deque::Steal::Empty => continue,
                crossbeam_deque::Steal::Retry => {
                    // Single retry on contention
                    if let crossbeam_deque::Steal::Success(mut task) = stealer.steal() {
                        self.run_task(&mut task);
                        return true;
                    }
                }
            }
        }
        
        false
    }
}
