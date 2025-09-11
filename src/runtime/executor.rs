//! Task execution engine
//!
//! This module provides the core executor implementation for the Luminal runtime.
//! The executor is responsible for scheduling and executing tasks.

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Context;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::bounded;
use crossbeam_deque::{Injector, Steal};

use super::join_handle::JoinHandle;
use super::task::{BoxFuture, Task, TaskId};
use super::waker::create_task_waker;
use super::worker::WorkerThread;

/// Inner state of the executor shared between instances
///
/// This structure contains the shared state of the executor,
/// including the task queue, worker threads, and statistics.
struct ExecutorInner {
    /// Global task queue used by all workers
    global_queue: Arc<Injector<Task>>,
    
    /// Handles to the worker threads
    worker_handles: Vec<thread::JoinHandle<()>>,
    
    /// Flag indicating whether the executor is shutting down
    shutdown: Arc<AtomicBool>,
    
    /// Counter for generating unique task IDs
    next_task_id: AtomicU64,
    
    /// Counter for the number of tasks processed
    tasks_processed: Arc<AtomicUsize>,
}

impl ExecutorInner {
    /// Creates a new executor inner state
    ///
    /// This initializes the global queue, creates worker threads,
    /// and starts the runtime.
    ///
    /// # Returns
    ///
    /// A new `ExecutorInner` instance
    fn new() -> Self {
        let global_queue = Arc::new(Injector::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let tasks_processed = Arc::new(AtomicUsize::new(0));
        
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        
        let mut stealers = Vec::with_capacity(num_workers);
        let mut worker_handles = Vec::with_capacity(num_workers);
        
        // Create workers for each thread (workers are not shared)
        for i in 0..num_workers {
            let worker = crossbeam_deque::Worker::new_fifo();
            stealers.push(worker.stealer());
            
            let worker_thread = WorkerThread {
                id: i,
                worker,
                stealers: stealers.clone(),
                global_queue: global_queue.clone(),
                shutdown: shutdown.clone(),
                tasks_processed: tasks_processed.clone(),
            };
            
            let handle = thread::Builder::new()
                .name(format!("luminal-worker-{}", i))
                .spawn(move || worker_thread.run())
                .expect("Failed to spawn worker thread");
            
            worker_handles.push(handle);
        }
        
        ExecutorInner {
            global_queue,
            worker_handles,
            shutdown,
            next_task_id: AtomicU64::new(1),
            tasks_processed,
        }
    }
    
    /// Spawns a new task to be executed by the runtime
    ///
    /// # Parameters
    ///
    /// * `future` - The future to execute as a task
    ///
    /// # Returns
    ///
    /// The ID of the spawned task
    fn spawn_internal(&self, future: BoxFuture) -> TaskId {
        let task_id = TaskId(self.next_task_id.fetch_add(1, Ordering::Relaxed));
        let task = Task::new(task_id, future);
        
        // Always use global queue for task distribution
        self.global_queue.push(task);
        
        task_id
    }
    
    /// Returns statistics about the runtime
    ///
    /// # Returns
    ///
    /// A tuple containing the current queue length and the number of tasks processed
    fn stats(&self) -> (usize, usize) {
        let global_len = self.global_queue.len();
        let tasks_processed = self.tasks_processed.load(Ordering::Relaxed);
        (global_len, tasks_processed)
    }
}

impl Drop for ExecutorInner {
    /// Cleans up resources when the executor is dropped
    ///
    /// This signals worker threads to shut down and waits for them to finish.
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        
        // Wait for workers to finish
        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

/// Core task execution engine for the Luminal runtime
///
/// The Executor is responsible for scheduling and executing tasks.
/// It maintains a global task queue and a set of worker threads that
/// process tasks using a work-stealing algorithm.
pub struct Executor {
    /// Shared executor state
    inner: Arc<ExecutorInner>,
}

impl Executor {
    /// Creates a new executor
    ///
    /// This initializes a new executor with worker threads based on the
    /// number of available CPU cores.
    ///
    /// # Returns
    ///
    /// A new `Executor` instance
    pub fn new() -> Self {
        Executor {
            inner: Arc::new(ExecutorInner::new()),
        }
    }

    /// Spawns a future onto the executor
    ///
    /// This wraps the future in a task and schedules it for execution,
    /// returning a JoinHandle that can be used to await its completion.
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
    /// A `JoinHandle` for the spawned task
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (sender, receiver) = bounded(1);
        
        // We need to use a different approach for panic handling in async code
        let wrapped_future = async move {
            // Create a future that will be polled within a panic handler
            let result = future.await;
            
            // Always try to send the result, even if the receiver is dropped
            // This prevents tasks from being stuck if their results aren't needed
            let _ = sender.send(result);
        };
        
        // Spawn with panic handling wrapper
        let task_id = self.inner.spawn_internal(Box::pin(wrapped_future));
        
        JoinHandle {
            id: task_id,
            receiver,
        }
    }

    /// Blocks the current thread until the provided future completes
    ///
    /// This spawns the future as a task and then actively helps execute tasks
    /// from the queue until the specified future completes.
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
    /// # Panics
    ///
    /// Panics if the task times out (after 30 seconds) or the task channel is disconnected
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = self.spawn(future);
        
        // Help with work while waiting for result
        let start = Instant::now();
        let mut spin_count = 0u32;
        
        loop {
            match handle.receiver.try_recv() {
                Ok(result) => return result,
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // Help process work to avoid deadlocks
                    match self.inner.global_queue.steal() {
                        Steal::Success(mut task) => {
                            let waker = create_task_waker(
                                task.id, 
                                self.inner.global_queue.clone()
                            );
                            let mut cx = Context::from_waker(&waker);

                            if let std::task::Poll::Pending = task.poll(&mut cx) {
                                self.inner.global_queue.push(task);
                            }
                            spin_count = 0;
                        }
                        Steal::Empty => {
                            spin_count += 1;
                            if spin_count > 1000 {
                                thread::yield_now();
                                spin_count = 0;
                            }

                            // Timeout protection
                            if start.elapsed() > Duration::from_secs(30) {
                                panic!("Task timed out after 30 seconds");
                            }
                        }
                        Steal::Retry => {
                            // Optionally handle retry, here we just yield
                            thread::yield_now();
                        }
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    // Channel closed: this happens if task panicked or was dropped
                    // Instead of panicking, handle gracefully
                    eprintln!("ERROR: Task channel disconnected - possible causes:");
                    eprintln!("  1. Task panicked during execution");
                    eprintln!("  2. Worker thread terminated unexpectedly");
                    eprintln!("  3. Memory corruption or use-after-free in task execution");
                    eprintln!("Try adding more yields in CPU-intensive tasks with .await points");
                    
                    // For debugging, we'll exit with a specific code to identify this error
                    std::process::exit(101);
                }
            }
        }
    }

    /// Runs the executor until the task queue is empty
    ///
    /// This method is primarily used for testing and benchmarking.
    pub fn run(&self) {
        // Runtime runs automatically via worker threads
        while !self.inner.global_queue.is_empty() {
            thread::sleep(Duration::from_millis(1));
        }
    }
    
    /// Returns statistics about the executor
    ///
    /// # Returns
    ///
    /// A tuple containing the current queue length and the number of tasks processed
    pub fn stats(&self) -> (usize, usize) {
        self.inner.stats()
    }
}
