use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::controller::startup::{read_work_startup_receipts_at, WorkStartupState};
use crate::durable::{RunId, WorkRef};
use crate::engine::process::{inspect_local_processes, tmux_pane_pid};
use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControllerOwner {
    pub work: WorkRef,
    pub attempt_id: String,
    pub run_id: RunId,
    pub trace_id: String,
    pub exec_id: String,
    pub pid: u32,
    pub process_started_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ControllerAuthority {
    Live { owner: ControllerOwner },
    Inactive,
    Parked { attempt_id: String },
    Unverifiable { reason: String },
}

pub(crate) async fn controller_authority(work: &WorkRef, tmux_name: &str) -> ControllerAuthority {
    let lf_home = crate::store::authority_home_dir();
    controller_authority_at(&lf_home, work, tmux_name).await
}

pub(crate) async fn controller_authority_at(
    lf_home: &Path,
    work: &WorkRef,
    tmux_name: &str,
) -> ControllerAuthority {
    let receipts = match read_work_startup_receipts_at(lf_home) {
        Ok(receipts) => receipts,
        Err(error) => {
            return unverifiable(work, format!("cannot read startup receipts: {error}"));
        }
    };
    let matching = receipts
        .iter()
        .filter(|receipt| match &receipt.state {
            WorkStartupState::Running { work: owner, .. }
            | WorkStartupState::Parked { work: owner } => owner == work,
            WorkStartupState::Failed { .. } => false,
        })
        .collect::<Vec<_>>();
    let latest_parked = matching.last().and_then(|receipt| {
        matches!(receipt.state, WorkStartupState::Parked { .. }).then(|| receipt.attempt_id.clone())
    });
    let exec_receipts = match crate::journal::read_exec_process_receipts_at(lf_home) {
        Ok(receipts) => receipts,
        Err(error) => {
            return unverifiable(work, format!("cannot read Exec receipts: {error}"));
        }
    };
    let processes = match inspect_local_processes().await {
        Ok(processes) => processes,
        Err(error) => return unverifiable(work, error),
    };
    let process_by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let mut owners = Vec::new();
    let mut broken = Vec::new();
    for receipt in matching {
        let WorkStartupState::Running {
            work: owner_work,
            run_id,
            trace_id,
            process_id,
            pid,
            process_started_at,
        } = &receipt.state
        else {
            continue;
        };
        let process = process_by_pid.get(pid).copied();
        let same_birth = process.is_some_and(|process| {
            process.matches_birth(*pid, *process_started_at)
                && !process.kernel_state.starts_with('Z')
        });
        if !same_birth {
            continue;
        }
        let live_loopflow = process.is_some_and(|process| process.is_live_loopflow());
        let exact_exec = exec_receipts.iter().any(|exec| {
            exec.trace_id == *trace_id
                && exec.exec_id == *process_id
                && exec.pid == *pid
                && exec.started_at == *process_started_at
        });
        if live_loopflow && exact_exec {
            owners.push(ControllerOwner {
                work: owner_work.clone(),
                attempt_id: receipt.attempt_id.clone(),
                run_id: run_id.clone(),
                trace_id: trace_id.clone(),
                exec_id: process_id.clone(),
                pid: *pid,
                process_started_at: *process_started_at,
            });
        } else {
            let edge = match (live_loopflow, exact_exec) {
                (false, _) => "recorded OS birth is not a live Loopflow process",
                (true, false) => "live OS birth has no matching Exec receipt",
                (true, true) => unreachable!("the validated owner was handled above"),
            };
            broken.push(format!(
                "attempt {} PID {}: {edge}",
                receipt.attempt_id, pid
            ));
        }
    }
    if !broken.is_empty() {
        return unverifiable(work, broken.join("; "));
    }
    if owners.len() > 1 {
        let identities = owners
            .iter()
            .map(|owner| {
                format!(
                    "attempt {} PID {} Exec {}",
                    owner.attempt_id, owner.pid, owner.exec_id
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return unverifiable(
            work,
            format!("multiple live controller owners: {identities}"),
        );
    }
    let pane_pid = match tmux_pane_pid(tmux_name).await {
        Ok(pid) => pid,
        Err(error) => return unverifiable(work, error),
    };
    if let Some(owner) = owners.pop() {
        if pane_pid.is_some_and(|pane_pid| pane_pid != owner.pid) {
            return unverifiable(
                work,
                format!(
                    "tmux transport {tmux_name} belongs to PID {}, not live owner PID {}",
                    pane_pid.expect("checked pane PID"),
                    owner.pid
                ),
            );
        }
        return ControllerAuthority::Live { owner };
    }
    if let Some(pid) = pane_pid {
        return unverifiable(
            work,
            format!("tmux transport {tmux_name} has unowned pane PID {pid}"),
        );
    }
    latest_parked.map_or(ControllerAuthority::Inactive, |attempt_id| {
        ControllerAuthority::Parked { attempt_id }
    })
}

fn unverifiable(work: &WorkRef, reason: impl std::fmt::Display) -> ControllerAuthority {
    ControllerAuthority::Unverifiable {
        reason: format!(
            "{} {} controller ownership is unverifiable: {reason}",
            work.kind(),
            work.id()
        ),
    }
}

pub(crate) fn matching_live_owner(
    authority: ControllerAuthority,
    expected: &ControllerOwner,
) -> Result<ControllerOwner, String> {
    match authority {
        ControllerAuthority::Live { owner } if owner == *expected => Ok(owner),
        ControllerAuthority::Live { owner } => Err(format!(
            "controller owner changed from attempt {} PID {} to attempt {} PID {}",
            expected.attempt_id, expected.pid, owner.attempt_id, owner.pid
        )),
        ControllerAuthority::Inactive => Err("controller became inactive".to_string()),
        ControllerAuthority::Parked { attempt_id } => {
            Err(format!("controller parked during attempt {attempt_id}"))
        }
        ControllerAuthority::Unverifiable { reason } => Err(reason),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControllerStop {
    Inactive,
    Parked { attempt_id: String },
}

pub(crate) async fn stop_controller_owner(
    store: &Store,
    lf_home: &Path,
    work: &WorkRef,
    tmux_name: &str,
    expected: &ControllerOwner,
) -> Result<ControllerStop, String> {
    matching_live_owner(
        controller_authority_at(lf_home, work, tmux_name).await,
        expected,
    )?;
    store
        .append_interrupt(work)
        .await
        .map_err(|error| error.to_string())?;
    let graceful_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match controller_authority_at(lf_home, work, tmux_name).await {
            ControllerAuthority::Inactive => return Ok(ControllerStop::Inactive),
            ControllerAuthority::Parked { attempt_id } => {
                return Ok(ControllerStop::Parked { attempt_id });
            }
            ControllerAuthority::Live { owner } if owner == *expected => {}
            ControllerAuthority::Live { owner } => {
                return Err(format!(
                    "controller owner changed from attempt {} PID {} to attempt {} PID {}",
                    expected.attempt_id, expected.pid, owner.attempt_id, owner.pid
                ));
            }
            ControllerAuthority::Unverifiable { reason } => return Err(reason),
        }
        if tokio::time::Instant::now() >= graceful_deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    matching_live_owner(
        controller_authority_at(lf_home, work, tmux_name).await,
        expected,
    )?;
    crate::engine::process::signal_process(expected.pid, "-TERM")
        .map_err(|error| error.to_string())?;
    let force_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match controller_authority_at(lf_home, work, tmux_name).await {
            ControllerAuthority::Inactive => return Ok(ControllerStop::Inactive),
            ControllerAuthority::Parked { attempt_id } => {
                return Ok(ControllerStop::Parked { attempt_id });
            }
            ControllerAuthority::Live { owner } if owner == *expected => {}
            ControllerAuthority::Live { owner } => {
                return Err(format!(
                    "controller owner changed to attempt {} PID {} during stop",
                    owner.attempt_id, owner.pid
                ));
            }
            ControllerAuthority::Unverifiable { reason } => return Err(reason),
        }
        if tokio::time::Instant::now() >= force_deadline {
            matching_live_owner(
                controller_authority_at(lf_home, work, tmux_name).await,
                expected,
            )?;
            crate::engine::process::signal_process(expected.pid, "-KILL")
                .map_err(|error| error.to_string())?;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let stopped_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match controller_authority_at(lf_home, work, tmux_name).await {
            ControllerAuthority::Inactive => return Ok(ControllerStop::Inactive),
            ControllerAuthority::Parked { attempt_id } => {
                return Ok(ControllerStop::Parked { attempt_id });
            }
            ControllerAuthority::Live { owner } if owner == *expected => {}
            ControllerAuthority::Live { owner } => {
                return Err(format!(
                    "controller owner changed to attempt {} PID {} after forced stop",
                    owner.attempt_id, owner.pid
                ));
            }
            ControllerAuthority::Unverifiable { reason } => {
                if tokio::time::Instant::now() >= stopped_deadline {
                    return Err(reason);
                }
            }
        }
        if tokio::time::Instant::now() >= stopped_deadline {
            return Err(format!(
                "controller PID {} remained live after forced stop",
                expected.pid
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::ControllerAuthority;

    #[test]
    fn authority_wire_shape_exposes_an_owner_only_when_live() {
        let inactive = serde_json::to_value(ControllerAuthority::Inactive).unwrap();
        assert_eq!(inactive, serde_json::json!({ "state": "inactive" }));

        let unverifiable = serde_json::to_value(ControllerAuthority::Unverifiable {
            reason: "broken identity".to_string(),
        })
        .unwrap();
        assert_eq!(
            unverifiable,
            serde_json::json!({ "state": "unverifiable", "reason": "broken identity" })
        );
    }
}
