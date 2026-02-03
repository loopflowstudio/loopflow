use tokio::sync::broadcast;

#[derive(Debug, Clone)]
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

    pub fn subscribe(&self) -> broadcast::Receiver<OutputEvent> {
        self.sender.subscribe()
    }
}
