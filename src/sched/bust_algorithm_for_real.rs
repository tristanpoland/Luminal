use std::future::Future;
use std::{collections::BTreeMap, sync::Arc, task::Wake};
use std::task::{Context, Poll, Waker};
use std::sync::atomic::{AtomicU64, Ordering};
use std::pin::Pin;

use crossbeam_queue::ArrayQueue;

#[cfg(not(debug_assertions))]
macro_rules! println {
    () => ();
    ($($arg:tt)*) => ();
}

#[cfg(debug_assertions)]
macro_rules! println {
    () => {
        std::print!("\n")
    };
    ($($arg:tt)*) => {{
        // this probably isn't the best way to do this because the standard println! macro does
        // that on its own
        // who cares tbh
        std::println!("{}", format_args!($($arg)*));
    }};
}

/// wtf tasque from the hit game deltaroon chapter two
pub struct Tasque {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Tasque {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Tasque {
        Tasque {
            id: TaskId::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}


struct WakeyWakey {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl WakeyWakey {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(WakeyWakey {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        if let Err(err) = self.task_queue.push(self.task_id) {
            panic!("task queue full: {:?} (capacity is {}, queue length is {})", 
                err, self.task_queue.capacity(), self.task_queue.len());
        }
    }
}

impl Wake for WakeyWakey {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

pub struct Executioner {
    tasks: BTreeMap<TaskId, Tasque>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executioner {
    pub fn new() -> Self {
        Executioner {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(42069)),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Tasque) {
        let task_id = task.id;
        if let None = self.tasks.insert(task.id, task) {
            panic!("task with same ID already in tasks"); // TODO: make this only panic in dev
        }
        if let Err(err) = self.task_queue.push(task_id) {
            if self.task_queue.is_full() {
                panic!("task queue full: {:?} (capacity is {}, queue length is {})", 
                    err, self.task_queue.capacity(), self.task_queue.len());
            }
            panic!("error inserting task into queue: {:?} (capacity is {}, queue length is {})",
                err, self.task_queue.capacity(), self.task_queue.len());
        }
    }

    pub fn run(&mut self) {
        loop {
            if self.tasks.len() == 0 {
                return;
            }
            self.run_ready_tasks();
            std::thread::yield_now(); // we're idle now
        }
    }

    fn run_ready_tasks(&mut self) {
        println!("run_ready_tasks: attempting to run a task");
        while let Some(task_id) = self.task_queue.pop() {
            let task = if let Some(task) = self.tasks.get_mut(&task_id) { task } else { continue };
            println!("got task id: {:?}", &task.id);
            let waker = self.waker_cache
                .entry(task_id)
                .or_insert_with(|| WakeyWakey::new(task_id, self.task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    println!("polled task with id {:?}, and its completed.", &task_id);
                    self.tasks.remove(&task_id);
                    self.waker_cache.remove(&task_id);
                }
                Poll::Pending => {
                    println!("polled task with id {:?}, and its pending.", &task_id);
                }
            }
        }
    }
}

