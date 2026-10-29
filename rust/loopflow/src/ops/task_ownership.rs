//! Claim, hand off and unassign a Task. Commands already run on the Task's
//! writer: entry dispatch forwards them over `lf --machine` before they arrive.
use std::path::Path;

use crate::durable::{MachineId, TaskId, TaskOwnership};
use crate::ops::{OpsError, OpsResult};
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipChange {
    Claim,
    /// A registered Machine label or a Machine id.
    Handoff(String),
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
            OwnershipChange::Handoff(target) => {
                let target = resolve_machine(&store.sqlite, target)?;
                store
                    .sqlite
                    .handoff_task(&task.id, &target)
                    .map_err(message)?;
            }
            OwnershipChange::Unassign => {
                store.sqlite.unclaim_task(&task.id).map_err(message)?;
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
/// assignee. The person is the Linear viewer on this Machine. Without a viewer
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
            if let Err(error) = store.assign_task_owner(task, Some(viewer.as_str())) {
                tracing::warn!(%task, %error, "Linear assignee was not queued");
            }
        }
        Err(error) => {
            eprintln!("warning: Linear viewer unavailable ({error}); the assignee is unchanged");
        }
    }
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
            entry.kind.as_str(),
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

    #[test]
    fn log_lines_read_as_a_time_and_a_person_on_a_named_machine() {
        let ownership = TaskOwnership {
            owner: None,
            superseded_by: None,
            log: vec![TaskOwnershipEntry {
                id: OwnershipEntryId::new(),
                task_id: TaskId::new(),
                kind: OwnershipKind::Claim,
                person: Some("Lee".into()),
                person_key: None,
                machine_id: Some(MachineId::new()),
                machine_label: Some("lee-mbp".into()),
                at: 1_792_600_000,
            }],
        };
        let rendered = render(&ownership);
        let line = rendered.lines().nth(1).unwrap();
        assert!(line.ends_with("  claim  Lee on lee-mbp"), "{line}");
        assert!(!line.contains("1792600000"), "{line}");
    }
}
