//! Claim, move and unassign a Task. Commands already run on the Task's
//! writer: entry dispatch forwards them over `lf --machine` before they arrive.
use std::path::Path;

use crate::durable::{MachineId, TaskId, TaskOwnership};
use crate::ops::{OpsError, OpsResult};
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipChange {
    Claim,
    /// A registered Machine label or a Machine id.
    Move(String),
    /// Nobody owns it, and Linear shows no assignee.
    Unassign,
}

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// A registered Machine label, this Machine's hostname, or a Machine id.
pub(crate) fn resolve_machine(store: &SqliteStore, target: &str) -> OpsResult<MachineId> {
    if let Some(machine) = store
        .machines()
        .map_err(message)?
        .into_iter()
        .find(|machine| machine.label.as_deref() == Some(target))
    {
        return Ok(machine.id);
    }
    if target == crate::store::sqlite::task_ownership::local_label() {
        return Ok(store.local_machine().map_err(message)?.id);
    }
    // A handoff needs only the target's identity, not a connection to it.
    MachineId::parse(target).map_err(|_| {
        message(format!(
            "{target} is neither a registered Machine label nor a Machine id; `lf machine add` it"
        ))
    })
}

pub fn change(repo: &Path, issue: &str, change: OwnershipChange) -> OpsResult<TaskOwnership> {
    super::task::block_on_task(async move {
        let (store, task) = super::task::resolve_saved_task(repo, None, issue).await?;
        match &change {
            OwnershipChange::Claim => {
                store.sqlite.claim_task(&task.id).map_err(message)?;
            }
            OwnershipChange::Move(target) => {
                let target = resolve_machine(&store.sqlite, target)?;
                store
                    .sqlite
                    .move_claim(&task.id, &target)
                    .map_err(message)?;
            }
            OwnershipChange::Unassign => {
                store.sqlite.unassign_task(&task.id).map_err(message)?;
                store
                    .sqlite
                    .assign_task_owner(&task.id, None)
                    .map_err(message)?;
            }
        }
        let repository = super::task::repository_for_task(&store, &task)?;
        assert_assignee(&store.sqlite, &repository, &task.id).await;
        super::planning_sync::sync_after_save(&store, &repository.to_string_lossy()).await;
        store.sqlite.task_ownership(&task.id).map_err(message)
    })
}

/// While this Machine holds an unsuperseded claim, Linear shows its person as
/// assignee. The person is the Linear viewer on this Machine, which is also
/// who this Machine belongs to (`people` id `linear:<viewer>`). Without a viewer
/// the assignee is left alone; ownership is already saved.
pub(crate) async fn assert_assignee(store: &SqliteStore, repository: &Path, task: &TaskId) {
    if !super::planning_client::is_linear(&repository.to_string_lossy()) {
        return;
    }
    match store.task_claimed_here(task) {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
            tracing::warn!(%task, %error, "ownership unreadable; Linear assignee unchanged");
            return;
        }
    }
    let viewer = match super::linear::issue_client().await {
        Ok(client) => client.viewer_id().await.map_err(message),
        Err(error) => Err(error),
    };
    match viewer {
        Ok(viewer) => {
            let name = crate::config::participant_name_in(repository)
                .ok()
                .flatten();
            if let Err(error) =
                store.record_local_person(&format!("linear:{viewer}"), name.as_deref())
            {
                tracing::warn!(%error, "the Linear viewer was not recorded as this Machine's person");
            }
            if let Err(error) = store.assign_task_owner(task, Some(viewer.as_str())) {
                tracing::warn!(%task, %error, "Linear assignee was not queued");
            }
        }
        Err(error) => {
            crate::journal::request::diagnostic(format_args!(
                "warning: Linear viewer unavailable ({error}); the assignee is unchanged"
            ));
        }
    }
}

/// Where a Task that another Machine owns runs, and since its claim reached
/// that Machine: `Runs on mini since Nov 12 10:42`. `None` when `local` owns
/// it or nobody does.
pub fn runs_elsewhere(ownership: &TaskOwnership, local: &MachineId) -> Option<String> {
    let machine = ownership.owner_elsewhere(local)?;
    let since = ownership.since(machine.id.as_ref()?)?;
    Some(format!(
        "Runs on {} since {}",
        machine.name(),
        crate::lf::output::format_time(since)
    ))
}

/// [`runs_elsewhere`], read against this Machine's registry.
pub fn runs_elsewhere_here(ownership: &TaskOwnership) -> OpsResult<Option<String>> {
    let Some(store) = crate::store::read_existing_registry().map_err(message)? else {
        return Ok(None);
    };
    let local = store.sqlite.local_machine().map_err(message)?.id;
    Ok(runs_elsewhere(ownership, &local))
}

/// `Not checked out on mini`: this Machine has no checkout of the Task, so it
/// shows no path, whichever Machine runs it.
pub fn not_checked_out_here() -> String {
    format!(
        "Not checked out on {}",
        crate::store::sqlite::task_ownership::local_label()
    )
}

/// A move of the Task this Machine began that has not arrived, with the
/// command that carries it on: `Leaving for mini since Nov 12 10:42: not
/// sent` or `Moving to mini since Nov 12 10:42: sent, not arrived`. `None`
/// when no move is in flight.
pub fn moving_here(task: &TaskId, identifier: &str) -> OpsResult<Option<String>> {
    let Some(store) = crate::store::read_existing_registry().map_err(message)? else {
        return Ok(None);
    };
    let leaving = store.sqlite.task_move_leaving(task).map_err(message)?;
    let (to, at, state, progress) = match leaving {
        Some(leaving) => (leaving.to_machine_id, leaving.at, "Leaving for", "not sent"),
        None => match store.sqlite.task_move_sent(task).map_err(message)? {
            Some(sent) => (
                sent.to_machine_id,
                sent.at,
                "Moving to",
                "sent, not arrived",
            ),
            None => return Ok(None),
        },
    };
    let place = store
        .sqlite
        .machine_by_id(&to)
        .map_err(message)?
        .map_or_else(|| to.to_string(), |machine| machine.name());
    Ok(Some(format!(
        "{state} {place} since {}: {progress}\n  retry: lf task move {identifier} --machine {place}",
        crate::lf::output::format_time(at)
    )))
}

pub fn render(ownership: &TaskOwnership) -> String {
    let mut lines = vec![match &ownership.owner {
        Some(owner) => format!("Owner: {owner}"),
        None => "Unowned".to_string(),
    }];
    if let Some(later) = &ownership.superseded_by {
        lines.push(format!("Also claimed later by {later}"));
    }
    for entry in &ownership.log {
        lines.push(format!(
            "  {}  {}  {}",
            crate::lf::output::format_time(entry.at),
            entry.kind.label(),
            entry.owner()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::durable::{
        MachineId, OwnershipEntryId, OwnershipKind, TaskId, TaskOwnership, TaskOwnershipEntry,
    };

    /// Lee's ownership log holding one `kind` entry on lee-mbp.
    fn lees(kind: OwnershipKind) -> TaskOwnership {
        TaskOwnership {
            owner: None,
            superseded_by: None,
            log: vec![TaskOwnershipEntry {
                id: OwnershipEntryId::new(),
                task_id: TaskId::new(),
                kind,
                person: Some("Lee".into()),
                person_key: None,
                person_id: None,
                machine_id: Some(MachineId::new()),
                machine_label: Some("lee-mbp".into()),
                writer_machine_id: None,
                at: 1_792_600_000,
            }],
        }
    }

    #[test]
    fn log_lines_read_as_a_time_and_a_person_on_a_named_machine() {
        let rendered = render(&lees(OwnershipKind::Claim));
        let line = rendered.lines().nth(1).unwrap();
        assert!(line.ends_with("  claim  Lee on lee-mbp"), "{line}");
        assert!(!line.contains("1792600000"), "{line}");
    }

    #[test]
    fn an_unassign_reads_as_unassign_and_is_stored_as_released() {
        let ownership = lees(OwnershipKind::Unassign);
        let rendered = render(&ownership);
        let line = rendered.lines().nth(1).unwrap();
        assert!(line.ends_with("  unassign  Lee on lee-mbp"), "{line}");
        let wire = serde_json::to_value(&ownership.log[0]).unwrap();
        assert_eq!(wire["kind"], "unclaim");
    }
}
