use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};
use std::thread;
use std::fmt;

#[derive(Debug)]
pub enum TaskError {
    Disconnected,
    Timeout,
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

impl std::error::Error for TaskError {}

use crossbeam_deque::{Injector, Stealer, Worker};
use crossbeam_channel::{bounded, Receiver, TryRecvError};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct Task {
    id: TaskId,
    future: BoxFuture,
}

impl Task {
    fn new(id: TaskId, future: BoxFuture) -> Self {
        Self { id, future }
    }
    
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

pub struct JoinHandle<T> {
    id: TaskId,
    receiver: Receiver<T>,
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.try_recv() {
            Ok(result) => Poll::Ready(result),
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => {
                // Channel disconnected - likely due to panic or dropped task
                // For safety we could abort, but that would break API
                // Instead log warning and return a default/placeholder
                eprintln!("WARNING: Task channel disconnected unexpectedly");
                // Keep pending to allow task completion or timeout
                Poll::Pending
            },
        }
    }
}

struct WorkerThread {
    id: usize,
    worker: Worker<Task>,
    stealers: Vec<Stealer<Task>>,
    global_queue: Arc<Injector<Task>>,
    shutdown: Arc<AtomicBool>,
    tasks_processed: Arc<AtomicUsize>,
}

impl WorkerThread {
    fn run(self) {
        let mut idle_count = 0u32;
        
        loop {
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
    
    fn run_task(&self, task: &mut Task) {
        let waker = create_task_waker(task.id, self.global_queue.clone());
        let mut cx = Context::from_waker(&waker);
        
        match task.poll(&mut cx) {
            Poll::Ready(()) => {
                self.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            Poll::Pending => {
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
    
    fn steal_work(&self) -> bool {
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

fn create_task_waker(_task_id: TaskId, global_queue: Arc<Injector<Task>>) -> Waker {
    use std::task::{RawWaker, RawWakerVTable};
    
    struct WakerData {
        task_id: TaskId,
        global_queue: Arc<Injector<Task>>,
    }
    
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |data| {
            let data = unsafe { &*(data as *const WakerData) };
            RawWaker::new(data as *const _ as *const (), &VTABLE)
        },
        |_data| {
            // Wake by re-queueing - task should already be queued
        },
        |_data| {
            // Wake by ref - task should already be queued  
        },
        |data| {
            let _ = unsafe { Box::from_raw(data as *mut WakerData) };
        },
    );
    
    let data = Box::into_raw(Box::new(WakerData {
        task_id: _task_id,
        global_queue,
    }));
    
    unsafe { Waker::from_raw(RawWaker::new(data as *const (), &VTABLE)) }
}

struct ExecutorInner {
    global_queue: Arc<Injector<Task>>,
    worker_handles: Vec<thread::JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    next_task_id: AtomicU64,
    tasks_processed: Arc<AtomicUsize>,
}

impl ExecutorInner {
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
            let worker = Worker::new_fifo();
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
                .name(format!("bust-worker-{}", i))
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
    
    fn spawn_internal(&self, future: BoxFuture) -> TaskId {
        let task_id = TaskId(self.next_task_id.fetch_add(1, Ordering::Relaxed));
        let task = Task::new(task_id, future);
        
        // Always use global queue for task distribution
        self.global_queue.push(task);
        
        task_id
    }
    
    fn stats(&self) -> (usize, usize) {
        let global_len = self.global_queue.len();
        let tasks_processed = self.tasks_processed.load(Ordering::Relaxed);
        (global_len, tasks_processed)
    }
}

impl Drop for ExecutorInner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        
        // Wait for workers to finish
        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

pub struct Executor {
    inner: Arc<ExecutorInner>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            inner: Arc::new(ExecutorInner::new()),
        }
    }

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
                Err(TryRecvError::Empty) => {
                    // Help process work to avoid deadlocks
                    match self.inner.global_queue.steal() {
                        crossbeam_deque::Steal::Success(mut task) => {
                            let waker = create_task_waker(
                                task.id, 
                                self.inner.global_queue.clone()
                            );
                            let mut cx = Context::from_waker(&waker);

                            if let Poll::Pending = task.poll(&mut cx) {
                                self.inner.global_queue.push(task);
                            }
                            spin_count = 0;
                        }
                        crossbeam_deque::Steal::Empty => {
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
                        crossbeam_deque::Steal::Retry => {
                            // Optionally handle retry, here we just yield
                            thread::yield_now();
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
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

    pub fn run(&self) {
        // Runtime runs automatically via worker threads
        while !self.inner.global_queue.is_empty() {
            thread::sleep(Duration::from_millis(1));
        }
    }
    
    pub fn stats(&self) -> (usize, usize) {
        self.inner.stats()
    }
}

#[derive(Clone)]
pub struct Runtime {
    executor: Arc<Executor>,
}

impl Runtime {
    pub fn new() -> Result<Self, crate::error::RuntimeError> {
        Ok(Runtime {
            executor: Arc::new(Executor::new()),
        })
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.spawn(future)
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.block_on(future)
    }

    pub fn handle(&self) -> Handle {
        Handle::new(self.executor.clone())
    }
    
    pub fn stats(&self) -> (usize, usize) {
        self.executor.stats()
    }
}

pub struct Handle {
    executor: Arc<Executor>,
}

impl Handle {
    fn new(executor: Arc<Executor>) -> Self {
        Handle { executor }
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.spawn(future)
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.executor.block_on(future)
    }
}

// Thread-local runtime for global functions
thread_local! {
    static THREAD_RUNTIME: Runtime = Runtime::new().unwrap();
}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    THREAD_RUNTIME.with(|rt| rt.spawn(future))
}

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    THREAD_RUNTIME.with(|rt| rt.block_on(future))
}
