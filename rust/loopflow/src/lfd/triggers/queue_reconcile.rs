use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::attention::reconcile_attention_items;
use crate::lfd::config::GitHubConfig;
use crate::lfd::events::EventHub;
use crate::lfd::queue::{reconcile_wave_queue_with_events, QueueTrigger};
use crate::lfd::store::SharedStore;

pub fn spawn_queue_reconciler(
    store: SharedStore,
    github: GitHubConfig,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("queue_reconciler shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let waves = match store.list_waves(None).await {
                        Ok(waves) => waves,
                        Err(err) => {
                            tracing::warn!(error = %err, "queue reconcile: failed to list waves");
                            continue;
                        }
                    };
                    for wave in waves {
                        if let Err(err) = reconcile_wave_queue_with_events(
                            &store,
                            &github,
                            wave.id(),
                            QueueTrigger::Poll,
                            Some(&event_hub),
                        )
                        .await
                        {
                            tracing::warn!(wave_id = %wave.id(), error = %err, "queue reconcile poll failed");
                        }
                    }
                    match reconcile_attention_items(&store).await {
                        Ok(resolved_items) => {
                            for item in resolved_items {
                                event_hub.send(crate::lfd::types::Event::attention_resolved(item));
                            }
                        }
                        Err(err) => {
                        tracing::warn!(error = %err, "attention reconcile poll failed");
                        }
                    }
                }
            }
        }
    })
}
