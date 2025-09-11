//! Worker thread implementation
//!
//! This module provides the implementation of worker threads,
//! which are responsible for executing tasks in the Luminal runtime.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Context;
use std::thread;
use std::time::Duration;

use crossbeam_deque::{Injector, Stealer, Worker};

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
            
            // Apply exponential backoff when no work is available
            if !found_work {
                idle_count += 1;
                if idle_count > 1000 {
                    // Exponential backoff to reduce CPU usage
                    let sleep_duration = std::cmp::min(idle_count - 1000, 1000);
                    thread::sleep(Duration::from_micros(sleep_duration as u64));
                } else {
                    thread::yield_now();
                }
            }
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
        
        match task.poll(&mut cx) {
            std::task::Poll::Ready(()) => {
                // Task completed successfully, increment the counter
                self.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            std::task::Poll::Pending => {
                // Re-queue the task for later execution
                // SAFETY IMPROVEMENT: Use a better approach than unsafe ptr::read
                // which could lead to double-free or use-after-free issues
                
                // Clone the task and push it back to the queue
                // This avoids unsafe code altogether
                let task_id = task.id;
                
                // Use global_queue instead of worker's queue to avoid potential overflow
                // This is safer at the cost of some performance
                self.global_queue.push(std::mem::replace(
                    task,
                    // This placeholder will be dropped immediately
                    Task::new(task_id, Box::pin(async {}))
                ));
            }
        }
    }
    
    /// Attempts to steal work from other workers or the global queue
    ///
    /// This implements the work-stealing algorithm that allows
    /// idle workers to take tasks from busy workers, improving
    /// overall throughput and load balancing.
    ///
    /// # Returns
    ///
    /// `true` if work was successfully stolen, `false` otherwise
    pub fn steal_work(&self) -> bool {
        // Try to steal from global queue first (batch steal for efficiency)
        for _ in 0..16 {
            match self.global_queue.steal() {
                crossbeam_deque::Steal::Success(task) => {
                    // Only push to worker queue if there's room (to prevent overflow)
                    if self.worker.len() < 100 {
                        self.worker.push(task);
                    } else {
                        // Push back to global queue if worker queue is too full
                        self.global_queue.push(task);
                    }
                    return true;
                }
                crossbeam_deque::Steal::Empty => break,
                crossbeam_deque::Steal::Retry => continue,
            }
        }
        
        // Try to steal from other workers with backoff on failures
        let mut retry_count = 0;
        for stealer in &self.stealers {
            // Skip self (compare by raw pointer address)
            if stealer as *const _ != &self.worker.stealer() as *const _ {
                match stealer.steal() {
                    crossbeam_deque::Steal::Success(task) => {
                        // Only push to worker queue if there's room
                        if self.worker.len() < 100 {
                            self.worker.push(task);
                        } else {
                            // Push back to global queue if worker queue is too full
                            self.global_queue.push(task);
                        }
                        return true;
                    }
                    crossbeam_deque::Steal::Empty => continue,
                    crossbeam_deque::Steal::Retry => {
                        retry_count += 1;
                        if retry_count > 10 {
                            thread::yield_now();
                            retry_count = 0;
                        }
                        continue;
                    }
                }
            }
        }
        
        false
    }
}
