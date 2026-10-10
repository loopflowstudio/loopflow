//! Execution location is observed from its Machine, never imported as planning.
//! A fresh positive reading can route retained work. No negative reading admits
//! first start, and no observation is written into the receiving Task's checkout.
use anyhow::{anyhow, ensure, Context, Result};

use crate::durable::{
    Machine, RepositoryId, TaskExecutionRoute, TaskExecutionSource, TaskId, TaskLocation,
    TaskLocationObservation,
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
        let bytes = crate::lf::commands::ssh::read(
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

pub async fn resolve(store: &SqliteStore, task: &TaskId) -> Result<TaskExecutionRoute> {
    let route = store.task_execution_route(task)?;
    if route.source == TaskExecutionSource::RecordedCheckout {
        if route.machine_id == store.local_machine()?.id {
            return Ok(route);
        }
        let machine = store
            .machine_by_id(&route.machine_id)?
            .filter(|machine| machine.label.is_some())
            .ok_or_else(|| {
                anyhow!(
                    "recorded execution Machine {} is unavailable: no added connection",
                    route.machine_id
                )
            })?;
        let reading = observe_peer(
            &machine,
            &route.repository_id,
            task,
            &uuid::Uuid::new_v4().to_string(),
        )
        .await;
        return recorded_route(&route.repository_id, &[reading]);
    }
    if !store.task_is_shared(task)? {
        return Ok(route);
    }
    let readings = observe(
        store,
        &route.repository_id,
        task,
        true,
        &uuid::Uuid::new_v4().to_string(),
    )
    .await?;
    recorded_route(&route.repository_id, &readings)
}

fn recorded_route(
    repository: &RepositoryId,
    readings: &[TaskLocationObservation],
) -> Result<TaskExecutionRoute> {
    let mut recorded = readings
        .iter()
        .filter_map(|reading| match &reading.location {
            TaskLocation::Recorded { task_id, checkout } => Some(TaskExecutionRoute {
                repository_id: repository.clone(),
                machine_id: reading.machine_id.clone(),
                task_id: task_id.clone(),
                checkout: checkout.clone(),
                source: TaskExecutionSource::RecordedCheckout,
            }),
            _ => None,
        });
    let first = recorded.next();
    ensure!(
        recorded.next().is_none(),
        "conflicting recorded execution Machines; no checkout or launch was prepared"
    );
    first.ok_or_else(|| {
        let unavailable = readings.iter().filter_map(|reading| match &reading.location {
            TaskLocation::Unavailable { reason } => Some(format!("{}: {reason}", reading.machine_id)),
            _ => None,
        }).collect::<Vec<_>>().join("; ");
        anyhow!("shared Task first-start admission is unavailable; no recorded execution owner was observed. No checkout or launch was prepared. {unavailable}")
    })
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
                TaskLocation::Unrecorded => {
                    "no local execution record (not first-start permission)".into()
                }
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
pub async fn explain(
    store: &SqliteStore,
    task: &TaskId,
    resolution: &mut ContextExplanation,
) -> Result<TaskExecutionRoute> {
    let route = match resolve(store, task).await {
        Ok(route) => route,
        Err(error) => {
            resolution.execution_machine = ContextFact::Unavailable {
                reason: error.to_string(),
            };
            resolution.checkout = ContextFact::Unavailable {
                reason: "no fresh execution-location observation".into(),
            };
            return Err(error);
        }
    };
    let source = match route.source {
        TaskExecutionSource::EffectiveDelegation => "effective_delegation",
        TaskExecutionSource::RecordedCheckout => {
            if matches!(&resolution.machine, ContextFact::Bound { value, .. } if value == route.machine_id.as_str())
            {
                "recorded_checkout"
            } else {
                "peer_recorded_checkout"
            }
        }
    };
    resolution.execution_machine = ContextFact::Bound {
        value: route.machine_id.to_string(),
        source: source.into(),
    };
    if route.source == TaskExecutionSource::RecordedCheckout {
        resolution.checkout = match &route.checkout {
            Some(value) => ContextFact::Bound {
                value: value.clone(),
                source: source.into(),
            },
            None => ContextFact::Unavailable {
                reason: "execution owner retains no checkout path".into(),
            },
        };
    }
    Ok(route)
}

#[cfg(test)]
mod tests {
    use super::{recorded_route, validate_reply};
    use crate::durable::{
        Machine, MachineId, RepositoryId, TaskId, TaskLocation, TaskLocationObservation,
    };

    #[test]
    fn conflicting_and_negative_location_readings_never_choose_a_launch_destination() {
        let repository = RepositoryId::new();
        let task = TaskId::new();
        let reading = TaskLocationObservation {
            request: "current".into(),
            repository_id: repository.clone(),
            task_id: task.clone(),
            machine_id: MachineId::new(),
            observed_at: 1,
            location: TaskLocation::Unrecorded,
        };
        assert!(recorded_route(&repository, std::slice::from_ref(&reading))
            .unwrap_err()
            .to_string()
            .contains("first-start admission"));
        let first = TaskLocationObservation {
            location: TaskLocation::Recorded {
                task_id: task.clone(),
                checkout: Some("/retained".into()),
            },
            ..reading
        };
        let second = TaskLocationObservation {
            machine_id: MachineId::new(),
            ..first.clone()
        };
        assert!(recorded_route(&repository, &[first.clone(), second])
            .unwrap_err()
            .to_string()
            .contains("conflicting recorded"));
        assert_eq!(
            recorded_route(&repository, std::slice::from_ref(&first))
                .unwrap()
                .machine_id,
            first.machine_id
        );
        let machine = Machine {
            id: first.machine_id.clone(),
            label: Some("peer".into()),
            repo: None,
            route: "peer".into(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        // Peer clocks may differ. Identity plus this request, not wall-clock age,
        // establishes that this response belongs to the bounded current read.
        assert!(validate_reply(&first, &machine, &repository, &task, "current").is_ok());
        assert!(validate_reply(&first, &machine, &repository, &task, "next").is_err());
        assert!(validate_reply(&first, &machine, &RepositoryId::new(), &task, "current").is_err());
    }
}
