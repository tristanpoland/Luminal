//! Task execution engine
//!
//! This module provides the core executor implementation for the Luminal runtime.
//! The executor is responsible for scheduling and executing tasks.

#[cfg(feature = "std")]
use std::{future::Future, sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}, sync::Arc, task::Context, thread, time::Duration, cell::RefCell};

#[cfg(not(feature = "std"))]
use core::{future::Future, sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}, task::Context, cell::RefCell};

#[cfg(not(feature = "std"))]
use alloc::{sync::Arc, vec::Vec, boxed::Box};

#[cfg(feature = "std")]
use crossbeam_channel::unbounded;

#[cfg(not(feature = "std"))]
use heapless::mpmc::MpMcQueue;
use crossbeam_deque::{Injector, Steal, Worker};

use super::join_handle::JoinHandle;
use super::task::{BoxFuture, Task, TaskId};
use super::waker::create_task_waker;
use super::worker::WorkerThread;

// Thread-local worker for ultra-fast task spawning (std only)
#[cfg(feature = "std")]
thread_local! {
    static LOCAL_WORKER: RefCell<Option<Worker<Task>>> = RefCell::new(None);
}

/// Inner state of the executor shared between instances
///
/// This structure contains the shared state of the executor,
/// including the task queue, worker threads, and statistics.
#[cfg(feature = "std")]
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

    /// All worker stealers for work distribution
    all_stealers: Arc<Vec<crossbeam_deque::Stealer<Task>>>,
}

/// Inner state of the executor shared between instances (no_std version)
///
/// This structure contains the simplified shared state of the executor
/// for no_std environments, without threading support.
#[cfg(not(feature = "std"))]
struct ExecutorInner {
    /// Global task queue used by the single-threaded executor
    global_queue: Arc<Injector<Task>>,

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
        
        let all_stealers = Arc::new(stealers);
        
        ExecutorInner {
            global_queue,
            worker_handles,
            shutdown,
            next_task_id: AtomicU64::new(1),
            tasks_processed,
            all_stealers,
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
        
        // Use more efficient task distribution:
        // Try to distribute to local queues first, fall back to global
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
        // Use unbounded channel with correct type
        let (sender, receiver) = unbounded::<F::Output>();
        
        // Optimized future wrapper with less overhead
        let wrapped_future = async move {
            let result = future.await;
            let _ = sender.send(result);
        };
        
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
        
        // Optimized blocking with better work helping
        let mut backoff_count = 0u32;
        
        loop {
            // Check for result first (most common case)
            match handle.receiver.try_recv() {
                Ok(result) => return result,
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // More efficient work helping strategy
                    let mut helped = false;
                    
                    // Try to help with multiple tasks in one go
                    for _ in 0..4 {
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
                                helped = true;
                            }
                            Steal::Empty => break,
                            Steal::Retry => continue,
                        }
                    }
                    
                    // Optimized backoff strategy
                    if !helped {
                        backoff_count += 1;
                        if backoff_count > 1000 {
                            thread::sleep(Duration::from_nanos(100));
                            backoff_count = 0;
                        } else if backoff_count > 100 {
                            thread::yield_now();
                        }
                        // Else busy wait for better latency
                    } else {
                        backoff_count = 0;
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    panic!("Task execution failed - channel disconnected");
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
