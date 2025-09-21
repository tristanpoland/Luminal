//! Simple single-threaded executor for no_std environments
//!
//! This module provides a simplified executor implementation for no_std environments
//! that doesn't use threads but still provides the same API as the full executor.

#[cfg(not(feature = "std"))]
use core::{future::Future, sync::atomic::{AtomicU64, AtomicUsize, Ordering}, task::Context};

#[cfg(not(feature = "std"))]
use alloc::{sync::Arc, boxed::Box};

#[cfg(not(feature = "std"))]
use heapless::mpmc::MpMcQueue;

#[cfg(not(feature = "std"))]
use heapless::Deque;

#[cfg(not(feature = "std"))]
use super::join_handle::JoinHandle;

#[cfg(not(feature = "std"))]
use super::task::{Task, TaskId};

#[cfg(not(feature = "std"))]
use super::waker::create_task_waker;

/// Simple executor for no_std environments
///
/// This executor provides basic task scheduling without threading,
/// suitable for embedded or constrained environments.
#[cfg(not(feature = "std"))]
pub struct SimpleExecutor {
    /// Global task queue using heapless deque with fixed capacity
    global_queue: Arc<parking_lot::Mutex<Deque<Task, 1024>>>,

    /// Counter for generating unique task IDs
    next_task_id: AtomicU64,

    /// Counter for the number of tasks processed
    tasks_processed: Arc<AtomicUsize>,
}

#[cfg(not(feature = "std"))]
impl SimpleExecutor {
    /// Creates a new simple executor
    pub fn new() -> Self {
        SimpleExecutor {
            global_queue: Arc::new(parking_lot::Mutex::new(Deque::new())),
            next_task_id: AtomicU64::new(1),
            tasks_processed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Spawns a future onto the executor
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // Create a bounded queue for this task's result
        let result_queue = Arc::new(MpMcQueue::<F::Output, 16>::new());
        let result_queue_clone = result_queue.clone();

        // Wrap the future to send its result to the queue
        let wrapped_future = async move {
            let result = future.await;
            // Attempt to enqueue the result, ignore if queue is full
            let _ = result_queue_clone.enqueue(result);
        };

        let task_id = TaskId(self.next_task_id.fetch_add(1, Ordering::Relaxed));
        let task = Task::new(task_id, Box::pin(wrapped_future));

        // Push to the back of the queue
        let mut queue = self.global_queue.lock();
        let _ = queue.push_back(task); // Ignore error if queue is full

        JoinHandle {
            id: task_id,
            receiver: result_queue,
        }
    }

    /// Blocks until the provided future completes
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = self.spawn(future);

        // Simple busy-wait polling loop
        loop {
            // Process some tasks from the queue
            for _ in 0..10 {
                // Try to pop a task from the front of the queue
                let task_opt = {
                    let mut queue = self.global_queue.lock();
                    queue.pop_front()
                };

                if let Some(mut task) = task_opt {
                    let waker = create_task_waker(task.id, self.global_queue.clone());
                    let mut cx = Context::from_waker(&waker);

                    match task.poll(&mut cx) {
                        core::task::Poll::Ready(()) => {
                            self.tasks_processed.fetch_add(1, Ordering::Relaxed);
                        }
                        core::task::Poll::Pending => {
                            // Re-queue the task
                            let mut queue = self.global_queue.lock();
                            let _ = queue.push_back(task);
                        }
                    }
                } else {
                    break; // No more tasks
                }
            }

            // Check if our result is ready
            if let Some(result) = handle.receiver.dequeue() {
                return result;
            }
        }
    }

    /// Returns statistics about the executor
    pub fn stats(&self) -> (usize, usize) {
        let global_len = {
            let queue = self.global_queue.lock();
            queue.len()
        };
        let tasks_processed = self.tasks_processed.load(Ordering::Relaxed);
        (global_len, tasks_processed)
    }
}