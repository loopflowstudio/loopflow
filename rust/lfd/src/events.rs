use tokio::sync::broadcast;

use crate::types::Event;

/// EventHub broadcasts events to all subscribers.
/// Events are fire-and-forget - if no one is listening, they're dropped.
#[derive(Clone)]
pub struct EventHub {
    sender: broadcast::Sender<Event>,
}

impl EventHub {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    /// Send an event to all subscribers.
    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}
