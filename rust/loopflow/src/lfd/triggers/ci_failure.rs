use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::activation::{enqueue_pending_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{ActivationSource, Event, Signal, Stimulus};
use time::OffsetDateTime;

#[derive(Debug)]
struct CiFailureActivation {
    wave_id: LfdId,
    pr_number: u32,
    branch: String,
    commit_sha: String,
    check_name: String,
    logs_url: String,
}

pub fn spawn_ci_failure_handler(
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let mut rx = event_hub.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("ci_failure_handler shutting down");
                    break;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::CiFailure {
                            wave_id,
                            pr_number,
                            branch,
                            commit_sha,
                            check_name,
                            logs_url,
                            ..
                        }) => {
                            let activation = CiFailureActivation {
                                wave_id,
                                pr_number,
                                branch,
                                commit_sha,
                                check_name,
                                logs_url,
                            };
                            if let Err(err) =
                                handle_ci_failure_event(&store, &event_hub, activation).await
                            {
                                tracing::warn!(error = %err, "failed handling CI failure activation");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
    })
}

async fn handle_ci_failure_event(
    store: &SharedStore,
    event_hub: &EventHub,
    activation: CiFailureActivation,
) -> Result<(), String> {
    let stimulus_id = resolve_ci_failure_stimulus(store, &activation.wave_id).await?;
    let reason = format!(
        "CI failure for PR #{} on {} ({}): {}",
        activation.pr_number, activation.branch, activation.check_name, activation.logs_url
    );
    let _ = enqueue_pending_activation(
        store,
        event_hub,
        ActivationEnvelope::new(
            &activation.wave_id,
            &stimulus_id,
            ActivationSource::Push,
            reason,
            &activation.commit_sha,
            &activation.commit_sha,
        ),
    )
    .await;
    Ok(())
}

async fn resolve_ci_failure_stimulus(
    store: &SharedStore,
    wave_id: &LfdId,
) -> Result<LfdId, String> {
    let stimuli = store
        .list_stimuli(Some(wave_id))
        .await
        .map_err(|err| err.to_string())?;
    if let Some(existing) = stimuli
        .into_iter()
        .find(|stimulus| stimulus.signal == Signal::CiFailure)
    {
        return Ok(existing.id);
    }

    let stimulus = Stimulus {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        source_wave_id: None,
        signal: Signal::CiFailure,
        flow: Some("ci-fix".to_string()),
        cron: None,
        last_main_sha: None,
        last_triggered_at: Some(OffsetDateTime::now_utc().unix_timestamp()),
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
    };
    store
        .create_stimulus(&stimulus)
        .await
        .map_err(|err| err.to_string())?;
    Ok(stimulus.id)
}
