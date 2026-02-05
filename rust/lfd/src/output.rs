//! Output streaming hub for terminal output lines.
//!
//! Reserved for HTTP output streaming endpoint.

use tokio::sync::broadcast;

#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for output streaming endpoint.
pub struct OutputEvent {
    pub wave_run_id: String,
    pub agent_id: String,
    pub text: String,
}

#[derive(Clone)]
pub struct OutputHub {
    sender: broadcast::Sender<OutputEvent>,
}

impl OutputHub {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    pub fn send(&self, event: OutputEvent) {
        let _ = self.sender.send(event);
    }

    #[allow(dead_code)] // Reserved for output streaming endpoint.
    pub fn subscribe(&self) -> broadcast::Receiver<OutputEvent> {
        self.sender.subscribe()
    }
}
