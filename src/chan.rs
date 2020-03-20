use tokio::sync::Notify;

use std::collections::VecDeque;
use std::sync::{Mutex, Arc};

pub struct Channel<T>(Arc<ChannelInner<T>>);

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Channel(self.0.clone())
    }
}

impl<T> Channel<T> {
    pub fn send(&self, value: T) {
        self.0.send(value)
    }

    pub async fn recv(&self) -> T {
        self.0.recv().await
    }
}

pub struct ChannelInner<T> {
    values: Mutex<VecDeque<T>>,
    notify: Notify,
}

impl<T> ChannelInner<T> {
    pub fn send(&self, value: T) {
        self.values.lock().unwrap()
            .push_back(value);

        // Notify the consumer a value is available
        self.notify.notify();
    }

    pub async fn recv(&self) -> T {
        loop {
            // Drain values
            if let Some(value) = self.values.lock().unwrap().pop_front() {
                return value;
            }

            // Wait for values to be available
            self.notify.notified().await;
        }
    }
}

pub fn unbounded_channel<T>() -> (Channel<T>, Channel<T>) {
    let c = Channel(Arc::new(ChannelInner {
        values: Mutex::new(VecDeque::new()),
        notify: Notify::new(),
    }));

    (c.clone(), c)
}