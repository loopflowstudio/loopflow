//! Whether a Task's Rex views retain it. Only a successful inventory confirms
//! absence; anything unread or unresolved stays unknown and never releases.
//! The shared lifetime owner decides cleanup; this read keeps no timer or closure.
use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use super::super::ViewHold;
use super::{find_native, Rex};
use crate::durable::TaskId;
use crate::store::sqlite::host_workspaces::{HostCreation, HostOccurrence, HostWorkspace};
use crate::store::sqlite::SqliteStore;

/// Rex windows of a workspace's container, `None` when the container is confirmed absent.
type Inventory<'a> = dyn FnMut(&HostWorkspace) -> Result<Option<Vec<Value>>> + 'a;

/// Read every Rex occurrence of the Task, `None` when it has none.
pub(super) fn task_hold(store: &SqliteStore, task: &TaskId) -> Result<Option<ViewHold>> {
    let occurrences: Vec<_> = store
        .task_host_occurrences(task)?
        .into_iter()
        .filter(|(workspace, _)| workspace.host == "rex")
        .collect();
    // One session list per server; an unreadable list stays an error for each occurrence.
    let mut sessions: HashMap<String, Result<Vec<Value>, String>> = HashMap::new();
    let mut read = |workspace: &HostWorkspace| -> Result<Option<Vec<Value>>> {
        let rex = Rex {
            server: workspace.server.clone(),
            caller: None,
        };
        let listed = sessions
            .entry(workspace.server.clone())
            .or_insert_with(|| rex.sessions().map_err(|error| format!("{error:#}")))
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let recorded = workspace.container_id.as_deref();
        match find_native(listed, "session_id", recorded, &workspace.id)
            .and_then(|session| session["session_id"].as_str())
        {
            Some(container) => rex.windows(container).map(Some),
            None => Ok(None),
        }
    };
    Ok(hold(&occurrences, &mut read))
}

/// Held wins, then unknown; only all-confirmed absence releases.
fn hold(
    occurrences: &[(HostWorkspace, HostOccurrence)],
    inventory: &mut Inventory<'_>,
) -> Option<ViewHold> {
    occurrences
        .iter()
        .filter_map(|(workspace, occurrence)| occurrence_hold(workspace, occurrence, inventory))
        .reduce(|a, b| match (a, b) {
            (ViewHold::Held, _) | (_, ViewHold::Held) => ViewHold::Held,
            (ViewHold::Unknown, _) | (_, ViewHold::Unknown) => ViewHold::Unknown,
            _ => ViewHold::Released,
        })
}

/// `None` when the occurrence never had a native view.
fn occurrence_hold(
    workspace: &HostWorkspace,
    occurrence: &HostOccurrence,
    inventory: &mut Inventory<'_>,
) -> Option<ViewHold> {
    let Some(destination) = occurrence.destination_id.as_deref() else {
        return (occurrence.creation == HostCreation::Pending).then_some(ViewHold::Unknown);
    };
    if workspace.creation_pending {
        return Some(ViewHold::Unknown);
    }
    Some(match inventory(workspace) {
        Err(error) => {
            tracing::debug!(%error, "Rex inventory is unknown; the hold stays");
            ViewHold::Unknown
        }
        // Native identity is the hold, whether or not its programs survived.
        Ok(Some(windows))
            if find_native(&windows, "window_id", Some(destination), &occurrence.id).is_some() =>
        {
            ViewHold::Held
        }
        Ok(_) => ViewHold::Released,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use serde_json::json;

    use super::{hold, HostCreation, HostOccurrence, HostWorkspace, ViewHold};
    use crate::durable::TaskId;
    use crate::work::selection::{Selection, SelectionExpression, SelectionId};

    fn occurrence(container: &str, window: Option<&str>) -> (HostWorkspace, HostOccurrence) {
        let workspace = HostWorkspace {
            id: format!("workspace-{container}"),
            repository_id: "repository".into(),
            host: "rex".into(),
            server: "unix:///rex".into(),
            selection: Selection {
                id: SelectionId::new(),
                expression: SelectionExpression::Repository,
            },
            copy_name: None,
            container_id: Some(container.into()),
            creation_pending: false,
            updated_at: 0,
        };
        let occurrence = HostOccurrence {
            id: format!("occurrence-{container}"),
            workspace_id: workspace.id.clone(),
            task_id: TaskId::from_raw("TASK_fixture"),
            destination_id: window.map(str::to_owned),
            creation: HostCreation::Complete,
        };
        (workspace, occurrence)
    }

    fn read(occurrences: &[(HostWorkspace, HostOccurrence)]) -> Option<ViewHold> {
        hold(
            occurrences,
            &mut |workspace| match workspace.container_id.as_deref().unwrap_or("") {
                "present" => Ok(Some(vec![json!({"window_id":"window","label":"Task"})])),
                "emptied" => Ok(Some(Vec::new())),
                "gone" => Ok(None),
                _ => Err(anyhow!("Rex session.list: server is unavailable")),
            },
        )
    }

    #[test]
    fn a_present_occurrence_holds_even_beside_unknown_inventory() {
        let held = [occurrence("present", Some("window"))];
        assert_eq!(read(&held), Some(ViewHold::Held));
        let mixed = [
            occurrence("unreachable", Some("window")),
            occurrence("present", Some("window")),
        ];
        assert_eq!(read(&mixed), Some(ViewHold::Held));
    }

    #[test]
    fn only_confirmed_absence_releases() {
        let released = [
            occurrence("gone", Some("window")),
            occurrence("emptied", Some("window")),
        ];
        assert_eq!(read(&released), Some(ViewHold::Released));
    }

    #[test]
    fn unreadable_inventory_or_unresolved_creation_stays_unknown() {
        let unreadable = [
            occurrence("gone", Some("window")),
            occurrence("unreachable", Some("window")),
        ];
        assert_eq!(read(&unreadable), Some(ViewHold::Unknown));
        let (workspace, mut pending) = occurrence("gone", None);
        pending.creation = HostCreation::Pending;
        assert_eq!(read(&[(workspace, pending)]), Some(ViewHold::Unknown));
    }

    #[test]
    fn a_task_without_views_reports_nothing() {
        assert_eq!(read(&[]), None);
    }
}
