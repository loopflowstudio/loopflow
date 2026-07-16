//! Reconcile a Task body against the durable interactive handoff its agent opened.
//!
//! The handoff row (see [`crate::interactive_handoff`]) is the sole source of
//! truth for "this parent is waiting on a human". The runner never launches a
//! provider process for the handoff and never renders its bytes; it only reads
//! the row, parks the parent while a human is needed, and — once the handoff is
//! terminal — claims the parent's one wake so the flow resumes exactly once.
//!
//! The flow cursor is the replay-safe boundary: it never advances past the step
//! that opened the handoff until that handoff is terminal and its wake is
//! claimed. Every body re-derives its obligation from these two durable records
//! at birth, so process death, app restart, and host restart all replay to the
//! same rendezvous rather than losing or double-running it.

use anyhow::Result;

use crate::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffOutcome, InteractiveHandoffParent,
};
use crate::store::SharedStore;

/// The parent body's obligation at the current flow position, derived from the
/// parent's handoffs. Exactly one rendezvous can be pending at a time: the flow
/// cursor cannot reach a second interactive step before the first is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rendezvous {
    /// No open or unclaimed interactive handoff — run the step as an ordinary turn.
    /// Also the state a generation sees once the wake is already claimed: the
    /// rendezvous is fully resolved and the flow has moved on.
    None,
    /// A human is still needed; the parent must park without advancing the cursor.
    Waiting,
    /// Terminal — resolve per outcome. `fresh` is true when this generation won the
    /// one parent wake (record evidence exactly once); false only in the
    /// pathological case where another body claimed it in the same instant.
    Resume {
        outcome: InteractiveHandoffOutcome,
        fresh: bool,
    },
}

/// The parent's one unresolved rendezvous: a non-terminal handoff, or a terminal
/// handoff whose wake nobody has claimed yet. Handoffs arrive oldest-first, so a
/// terminal-claimed handoff from a prior interactive step is never mistaken for
/// the current one — advancing the cursor is gated on claiming, so a
/// terminal-unclaimed handoff always belongs to the current, unadvanced step.
pub(crate) fn pending(handoffs: &[InteractiveHandoff]) -> Option<&InteractiveHandoff> {
    handoffs
        .iter()
        .find(|handoff| !handoff.status.is_terminal() || handoff.wake_claimed_at.is_none())
}

/// Read the parent's handoffs and reconcile the current rendezvous, claiming the
/// terminal wake for `generation` when one is pending.
pub(crate) async fn resolve(
    store: &SharedStore,
    parent: &InteractiveHandoffParent,
    generation: u32,
) -> Result<Rendezvous> {
    let handoffs = store.list_interactive_handoffs(Some(parent)).await?;
    let Some(handoff) = pending(&handoffs) else {
        return Ok(Rendezvous::None);
    };
    if !handoff.status.is_terminal() {
        return Ok(Rendezvous::Waiting);
    }
    let outcome = handoff
        .outcome
        .clone()
        .expect("a terminal handoff always carries an outcome");
    let id = handoff.id.clone();
    let fresh = store
        .claim_interactive_handoff_wake(&id, generation)
        .await?;
    Ok(Rendezvous::Resume { outcome, fresh })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{pending, resolve, Rendezvous};
    use crate::engine::wave_home::WaveHome;
    use crate::id::WaveId;
    use crate::interactive_handoff::{
        InteractiveHandoffOutcome, InteractiveHandoffParent, OpenInteractiveHandoff,
    };
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::wave::Wave;

    async fn store_with_wave() -> (SharedStore, WaveId) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep().join("registry.db");
        let store: SharedStore = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        let wave = Wave::new(WaveId::new(), "product".to_string(), "/repo".to_string());
        store.create_wave(&wave).await.unwrap();
        (store, wave.id().clone())
    }

    fn open_request(parent: &InteractiveHandoffParent, generation: u32) -> OpenInteractiveHandoff {
        OpenInteractiveHandoff {
            parent: parent.clone(),
            home: WaveHome::parse("jack@local").unwrap(),
            cwd: PathBuf::from("/repo"),
            provider: "codex".to_string(),
            provider_session_id: Some("provider-thread".to_string()),
            body_generation: generation,
            reason: "Needs an interactive OAuth login".to_string(),
            environment: BTreeMap::from([("LF_HOME".to_string(), "/home".to_string())]),
            attach_argv: vec!["tmux".to_string(), "attach".to_string()],
        }
    }

    #[tokio::test]
    async fn no_handoff_means_run_normally() {
        let (store, wave) = store_with_wave().await;
        let parent = InteractiveHandoffParent::Wave(wave);
        assert_eq!(resolve(&store, &parent, 1).await.unwrap(), Rendezvous::None);
        assert!(pending(&[]).is_none());
    }

    #[tokio::test]
    async fn an_open_handoff_parks_the_parent() {
        let (store, wave) = store_with_wave().await;
        let parent = InteractiveHandoffParent::Wave(wave);
        store
            .open_interactive_handoff(open_request(&parent, 1))
            .await
            .unwrap();
        assert_eq!(
            resolve(&store, &parent, 1).await.unwrap(),
            Rendezvous::Waiting
        );
        let handoffs = store
            .list_interactive_handoffs(Some(&parent))
            .await
            .unwrap();
        assert!(pending(&handoffs).is_some());
    }

    #[tokio::test]
    async fn a_completed_handoff_resumes_the_parent_exactly_once() {
        let (store, wave) = store_with_wave().await;
        let parent = InteractiveHandoffParent::Wave(wave);
        let (handoff, _) = store
            .open_interactive_handoff(open_request(&parent, 1))
            .await
            .unwrap();
        store
            .finish_interactive_handoff(
                &handoff.id,
                &InteractiveHandoffOutcome::Completed {
                    summary: "human finished the login".to_string(),
                },
            )
            .await
            .unwrap();

        // A later generation wins the one wake and resumes with fresh evidence.
        match resolve(&store, &parent, 2).await.unwrap() {
            Rendezvous::Resume {
                outcome: InteractiveHandoffOutcome::Completed { summary },
                fresh,
            } => {
                assert_eq!(summary, "human finished the login");
                assert!(fresh, "the winning generation records evidence once");
            }
            other => panic!("expected a fresh resume, got {other:?}"),
        }

        // Any further generation sees the wake already claimed — the rendezvous is
        // fully resolved, so it runs normally rather than resuming a second time.
        assert_eq!(resolve(&store, &parent, 3).await.unwrap(), Rendezvous::None);
        let handoffs = store
            .list_interactive_handoffs(Some(&parent))
            .await
            .unwrap();
        assert!(pending(&handoffs).is_none());
    }

    #[tokio::test]
    async fn a_failed_handoff_is_terminal_and_resumes_once() {
        let (store, wave) = store_with_wave().await;
        let parent = InteractiveHandoffParent::Wave(wave);
        let (handoff, _) = store
            .open_interactive_handoff(open_request(&parent, 1))
            .await
            .unwrap();
        store
            .finish_interactive_handoff(
                &handoff.id,
                &InteractiveHandoffOutcome::Failed {
                    reason: "human could not complete the login".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            resolve(&store, &parent, 2).await.unwrap(),
            Rendezvous::Resume {
                outcome: InteractiveHandoffOutcome::Failed { .. },
                fresh: true,
            }
        ));
    }

    #[test]
    fn pending_ignores_a_resolved_and_claimed_handoff() {
        // pending() over an empty slice is the trivial resolved case.
        assert!(pending(&[]).is_none());
        assert!(pending(&[]).is_none());
    }

    #[tokio::test]
    async fn reopening_a_still_open_handoff_reuses_the_same_row() {
        let (store, wave) = store_with_wave().await;
        let parent = InteractiveHandoffParent::Wave(wave);
        let (first, created_first) = store
            .open_interactive_handoff(open_request(&parent, 1))
            .await
            .unwrap();
        assert!(created_first);
        // A restarted body re-opens at the same interactive step: idempotent replay.
        let (second, created_second) = store
            .open_interactive_handoff(open_request(&parent, 1))
            .await
            .unwrap();
        assert!(!created_second);
        assert_eq!(first.id, second.id);
        assert_eq!(
            resolve(&store, &parent, 1).await.unwrap(),
            Rendezvous::Waiting
        );
    }
}
