//! Task run legality, independent of Workflow selection and execution history.
use loopflow_storage::execution::TaskState;
use loopflow_storage::sqlite::task_pause::PauseCleanup;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskRunControl {
    pub unavailable: Option<String>,
}

/// Facts beyond the Graph record that decide control legality.
#[derive(Debug)]
pub struct TaskRunEvidence<'a> {
    pub status: TaskState,
    pub issue: &'a str,
    pub running_step: Option<&'a str>,
    pub pause_cleanup: Option<PauseCleanup>,
    pub plan_terminal_reason: Option<&'a str>,
    pub worktree_blocker: Option<&'a str>,
    pub launch_refusal: Option<&'a str>,
}

pub fn task_run_control(evidence: &TaskRunEvidence) -> TaskRunControl {
    let unavailable = match evidence.status {
        TaskState::Paused => Some(match (evidence.running_step, evidence.pause_cleanup) {
            (Some(step), _) => format!("Task is pausing; {step} is still running"),
            (
                _,
                Some(PauseCleanup {
                    worker_session: Some(session),
                }),
            ) => format!("Task is pausing; worker OS session {session} is stopping"),
            (_, Some(_)) => "Task is pausing; cleanup is in progress".into(),
            (_, None) => loopflow_storage::sqlite::task_pause::run_refusal(evidence.issue),
        }),
        TaskState::Done => Some("Task is complete".into()),
        TaskState::Abandoned => Some("Task is abandoned; recover it before running a Graph".into()),
        _ => evidence
            .plan_terminal_reason
            .or(evidence.worktree_blocker)
            .or(evidence.launch_refusal)
            .map(str::to_string),
    };
    TaskRunControl { unavailable }
}

#[cfg(test)]
mod tests {
    use super::{task_run_control, TaskRunEvidence};
    use loopflow_storage::execution::TaskState;
    use loopflow_storage::sqlite::task_pause::PauseCleanup;

    #[test]
    fn a_pausing_task_names_the_work_to_wait_for() {
        let mut evidence = TaskRunEvidence {
            status: TaskState::Paused,
            issue: "FIX-1",
            running_step: Some("land"),
            pause_cleanup: Some(PauseCleanup {
                worker_session: Some(42),
            }),
            plan_terminal_reason: None,
            worktree_blocker: Some("checkout is initializing"),
            launch_refusal: None,
        };
        let refusal = |e: &TaskRunEvidence| task_run_control(e).unavailable.unwrap();
        assert_eq!(refusal(&evidence), "Task is pausing; land is still running");
        evidence.running_step = None;
        assert_eq!(
            refusal(&evidence),
            "Task is pausing; worker OS session 42 is stopping"
        );
        evidence.pause_cleanup = None;
        assert_eq!(
            refusal(&evidence),
            "Task is paused; run `lf task unpause FIX-1` to resume it"
        );
    }

    #[test]
    fn run_legality_preserves_terminal_checkout_and_process_refusals() {
        for (status, plan, checkout, process, expected) in [
            (TaskState::Idle, None, None, None, None),
            (TaskState::Done, None, None, None, Some("Task is complete")),
            (
                TaskState::Unstarted,
                Some("withdrawn"),
                None,
                None,
                Some("withdrawn"),
            ),
            (
                TaskState::Idle,
                None,
                Some("missing checkout"),
                None,
                Some("missing checkout"),
            ),
            (
                TaskState::Idle,
                None,
                None,
                Some("active process"),
                Some("active process"),
            ),
        ] {
            let control = task_run_control(&TaskRunEvidence {
                status,
                issue: "FIX-1",
                running_step: None,
                pause_cleanup: None,
                plan_terminal_reason: plan,
                worktree_blocker: checkout,
                launch_refusal: process,
            });
            assert_eq!(control.unavailable.as_deref(), expected);
        }
    }
}
