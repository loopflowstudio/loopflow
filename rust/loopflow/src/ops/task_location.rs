//! Execution location is observed from its Machine, never imported as planning.
//! Routing follows ownership; peer readings are observations only.
use anyhow::{ensure, Context, Result};

use crate::durable::{
    Machine, OwningMachineSource, RepositoryId, TaskExecutionRoute, TaskExecutionSource, TaskId,
    TaskLocation, TaskLocationObservation,
};
use crate::ops::context::{ContextExplanation, ContextFact};
use crate::store::sqlite::SqliteStore;

pub async fn observe(
    store: &SqliteStore,
    repository: &RepositoryId,
    task: &TaskId,
    peers: bool,
    request: &str,
) -> Result<Vec<TaskLocationObservation>> {
    let local = store.observe_task_location(repository, task, request)?;
    let mut observations = vec![local];
    if peers {
        for machine in store.machines()? {
            if machine.id == observations[0].machine_id {
                continue;
            }
            observations.push(observe_peer(&machine, repository, task, request).await);
        }
    }
    Ok(observations)
}

async fn observe_peer(
    machine: &Machine,
    repository: &RepositoryId,
    task: &TaskId,
    request: &str,
) -> TaskLocationObservation {
    let result = async {
        let bytes = crate::lf::commands::ssh::request(
            machine,
            &[
                "--repository".into(),
                repository.to_string(),
                "task".into(),
                "location".into(),
                task.to_string(),
                "--request".into(),
                request.into(),
                "--json".into(),
            ],
        )
        .await?;
        let [reading]: [TaskLocationObservation; 1] = serde_json::from_slice(&bytes)
            .context("peer did not return one local execution-location reading")?;
        validate_reply(&reading, machine, repository, task, request)?;
        Ok::<_, anyhow::Error>(reading)
    }
    .await;
    result.unwrap_or_else(|error| TaskLocationObservation {
        request: request.into(),
        repository_id: repository.clone(),
        task_id: task.clone(),
        machine_id: machine.id.clone(),
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        location: TaskLocation::Unavailable {
            reason: error.to_string(),
        },
    })
}

fn validate_reply(
    reading: &TaskLocationObservation,
    machine: &Machine,
    repository: &RepositoryId,
    task: &TaskId,
    request: &str,
) -> Result<()> {
    // Nonces establish response freshness without comparing clocks on different
    // Machines. The transport deadline bounds how long this invocation waits.
    ensure!(
        reading.request == request
            && reading.repository_id == *repository
            && reading.task_id == *task
            && reading.machine_id == machine.id,
        "stale or mismatched execution-location observation"
    );
    Ok(())
}

pub fn render(readings: &[TaskLocationObservation]) -> String {
    readings
        .iter()
        .map(|reading| {
            let location = match &reading.location {
                TaskLocation::Recorded { task_id, checkout } => format!(
                    "recorded Task {task_id}: {}",
                    checkout.as_deref().unwrap_or("checkout unavailable")
                ),
                TaskLocation::Unrecorded => "no local execution record".into(),
                TaskLocation::Unavailable { reason } => format!("unavailable: {reason}"),
            };
            format!(
                "{} at {}: {location}",
                reading.machine_id, reading.observed_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Enrich presentation without copying a peer checkout into local execution.
pub fn explain(
    store: &SqliteStore,
    task: &TaskId,
    resolution: &mut ContextExplanation,
) -> Result<TaskExecutionRoute> {
    let route = match store.task_execution_route(task) {
        Ok(route) => route,
        Err(error) => {
            resolution.execution_machine = ContextFact::Unavailable {
                reason: error.to_string(),
            };
            return Err(error.into());
        }
    };
    let source = match &route.source {
        TaskExecutionSource::Owner => "owner".to_string(),
        TaskExecutionSource::OwningMachine(OwningMachineSource::Wave(wave)) => {
            format!("owning_machine:wave:{wave}")
        }
        TaskExecutionSource::OwningMachine(OwningMachineSource::ThisMachine) => {
            "owning_machine:this_machine".into()
        }
    };
    resolution.execution_machine = ContextFact::Bound {
        value: route.machine_id.to_string(),
        source,
    };
    Ok(route)
}

#[cfg(test)]
mod tests {
    use super::validate_reply;
    use crate::durable::{
        Machine, MachineId, RepositoryId, TaskId, TaskLocation, TaskLocationObservation,
    };

    #[test]
    fn peer_readings_bind_to_their_request_and_machine() {
        let repository = RepositoryId::new();
        let task = TaskId::new();
        let reading = TaskLocationObservation {
            request: "current".into(),
            repository_id: repository.clone(),
            task_id: task.clone(),
            machine_id: MachineId::new(),
            observed_at: 1,
            location: TaskLocation::Recorded {
                task_id: task.clone(),
                checkout: Some("/retained".into()),
            },
        };
        let machine = Machine {
            id: reading.machine_id.clone(),
            label: Some("peer".into()),
            repo: None,
            route: "peer".into(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        // Peer clocks may differ. Identity plus this request, not wall-clock age,
        // establishes that this response belongs to the bounded current read.
        assert!(validate_reply(&reading, &machine, &repository, &task, "current").is_ok());
        assert!(validate_reply(&reading, &machine, &repository, &task, "next").is_err());
        assert!(
            validate_reply(&reading, &machine, &RepositoryId::new(), &task, "current").is_err()
        );
    }
}
