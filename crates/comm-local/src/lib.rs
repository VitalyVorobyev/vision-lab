//! In-process communication helpers for local component runtimes.

use comm_core::{ApiError, EventEnvelope, EventStream};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{RwLock, broadcast, mpsc, oneshot, watch};

#[derive(Debug, Default)]
pub struct MonotonicCounter {
    next: AtomicU64,
}

impl MonotonicCounter {
    pub fn new(start: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
        }
    }

    pub fn advance(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current(&self) -> u64 {
        self.next.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct CommandRequest<C, R> {
    pub command: C,
    pub respond_to: oneshot::Sender<Result<R, ApiError>>,
}

#[derive(Debug, Clone)]
pub struct CommandClient<C, R> {
    tx: mpsc::Sender<CommandRequest<C, R>>,
}

pub type CommandInbox<C, R> = mpsc::Receiver<CommandRequest<C, R>>;

pub fn command_channel<C, R>(capacity: usize) -> (CommandClient<C, R>, CommandInbox<C, R>) {
    let (tx, rx) = mpsc::channel(capacity);
    (CommandClient { tx }, rx)
}

impl<C, R> CommandClient<C, R>
where
    C: Send + 'static,
    R: Send + 'static,
{
    pub async fn submit(&self, command: C) -> Result<R, ApiError> {
        let (respond_to, response) = oneshot::channel();
        self.tx
            .send(CommandRequest {
                command,
                respond_to,
            })
            .await
            .map_err(|_| ApiError::Unavailable("component command mailbox closed".into()))?;
        response
            .await
            .map_err(|_| ApiError::Unavailable("component command response dropped".into()))?
    }
}

#[derive(Debug)]
pub struct EventBus<T> {
    tx: broadcast::Sender<EventEnvelope<T>>,
}

impl<T> EventBus<T>
where
    T: Clone + Send + 'static,
{
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: EventEnvelope<T>) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> EventStream<T> {
        self.tx.subscribe()
    }
}

#[derive(Debug)]
pub struct StateCell<T> {
    value: RwLock<T>,
}

impl<T> StateCell<T>
where
    T: Clone,
{
    pub fn new(initial: T) -> Self {
        Self {
            value: RwLock::new(initial),
        }
    }

    pub async fn get(&self) -> T {
        self.value.read().await.clone()
    }

    pub async fn set(&self, value: T) {
        *self.value.write().await = value;
    }
}

#[derive(Debug)]
pub struct LatestValueBus<T> {
    tx: watch::Sender<Option<Arc<T>>>,
    published: AtomicU64,
    replaced: AtomicU64,
}

impl<T> LatestValueBus<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> Self {
        let (tx, _) = watch::channel(None);
        Self {
            tx,
            published: AtomicU64::new(0),
            replaced: AtomicU64::new(0),
        }
    }

    pub fn publish(&self, value: T) {
        if self.tx.borrow().is_some() {
            self.replaced.fetch_add(1, Ordering::SeqCst);
        }
        self.published.fetch_add(1, Ordering::SeqCst);
        self.tx.send_replace(Some(Arc::new(value)));
    }

    pub fn subscribe(&self) -> LatestValueReceiver<T> {
        LatestValueReceiver {
            rx: self.tx.subscribe(),
            last_seen_generation: 0,
        }
    }

    pub fn subscribe_raw(&self) -> watch::Receiver<Option<Arc<T>>> {
        self.tx.subscribe()
    }

    pub fn published_count(&self) -> u64 {
        self.published.load(Ordering::SeqCst)
    }

    pub fn replaced_count(&self) -> u64 {
        self.replaced.load(Ordering::SeqCst)
    }
}

impl<T> Default for LatestValueBus<T>
where
    T: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct LatestValueReceiver<T> {
    rx: watch::Receiver<Option<Arc<T>>>,
    last_seen_generation: u64,
}

impl<T> LatestValueReceiver<T>
where
    T: Send + Sync + 'static,
{
    pub async fn changed(&mut self) -> Result<Option<Arc<T>>, ApiError> {
        self.rx
            .changed()
            .await
            .map_err(|_| ApiError::StreamClosed)?;
        self.last_seen_generation = self.last_seen_generation.saturating_add(1);
        Ok(self.rx.borrow().clone())
    }

    pub fn latest(&self) -> Option<Arc<T>> {
        self.rx.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use comm_core::{ComponentIdentity, event};

    #[test]
    fn counter_is_monotonic() {
        let counter = MonotonicCounter::default();
        assert_eq!(counter.advance(), 1);
        assert_eq!(counter.advance(), 2);
        assert_eq!(counter.current(), 2);
    }

    #[tokio::test]
    async fn event_subscription_receives_published_event() {
        let bus = EventBus::new(8);
        let mut rx = bus.subscribe();
        let identity = ComponentIdentity::new("test", "unit", "0");
        bus.publish(event(identity, 1, None, "ready".to_string()));
        let received = rx.recv().await.unwrap();
        assert_eq!(received.sequence, 1);
        assert_eq!(received.payload, "ready");
    }

    #[test]
    fn latest_value_counts_replacements() {
        let bus = LatestValueBus::new();
        bus.publish(1_u32);
        bus.publish(2_u32);
        assert_eq!(bus.published_count(), 2);
        assert_eq!(bus.replaced_count(), 1);
    }
}
