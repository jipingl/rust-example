use std::{
    future::Future,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

use futures::{
    future::BoxFuture,
    task::{waker_ref, ArcWake},
};

pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shard_state = shared_state.clone();
        thread::spawn(move || {
            println!("TimerFuture::new in thread");
            thread::sleep(duration);
            let mut shared_state = thread_shard_state.lock().unwrap();
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        TimerFuture { shared_state }
    }
}

pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Task>>,
}

pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let box_future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(box_future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("Queue is full");
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self
            .task_sender
            .send(arc_self.clone())
            .expect("Queue is full");
    }
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            println!("The task received");
            let mut opt_future = task.future.lock().unwrap();
            if let Some(mut future) = opt_future.take() {
                let waker = waker_ref(&task);
                let ref mut context = Context::from_waker(&*waker);
                if future.as_mut().poll(context).is_pending() {
                    *opt_future = Some(future);
                }
            }
        }
    }
}
