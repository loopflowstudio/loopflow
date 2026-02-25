use tokio::sync::broadcast;

use crate::lfd::types::Event;

/// EventHub broadcasts events to all subscribers.
/// Events are fire-and-forget - if no one is listening, they're dropped.
#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::AgentStatus;

    #[tokio::test]
    async fn event_hub_delivers_executor_events() {
        let hub = EventHub::new(16);
        let mut rx = hub.subscribe();

        let wave_id = LfdId::from_raw("wave-1");
        let run_id = LfdId::from_raw("run-1");
        let agent_id = LfdId::from_raw("agent-1");

        hub.send(Event::wave_started(wave_id.clone(), run_id.clone()));
        hub.send(Event::agent_started(
            agent_id.clone(),
            "implement".to_string(),
            "/tmp/wt".to_string(),
        ));
        hub.send(Event::agent_ended(agent_id, AgentStatus::Completed));
        hub.send(Event::wave_waiting(
            wave_id.clone(),
            run_id,
            "review".to_string(),
            None,
        ));
        hub.send(Event::wave_updated(wave_id));

        let types: Vec<String> = (0..5)
            .map(|_| {
                let event = rx.try_recv().unwrap();
                let json = serde_json::to_value(&event).unwrap();
                json["type"].as_str().unwrap().to_string()
            })
            .collect();

        assert_eq!(
            types,
            vec![
                "wave_started",
                "agent_started",
                "agent_ended",
                "wave_waiting",
                "wave_updated",
            ]
        );
    }
}
