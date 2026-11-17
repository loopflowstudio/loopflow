//! Typed operations accepted by the Machine dispatcher.

use crate::lf::commands::{read::Read, write::Write};
pub use loopflow_execution::machine::wire::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type ClientMessage = loopflow_execution::machine::wire::ClientMessage<Operation, Read>;

/// What a request asks for, named by its `kind`: a dotted resource and verb
/// (`task.complete`, `plan.list`). `desktop.*` kinds are Desktop's own
/// arrangement state, not part of what other clients are offered.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Operation {
    Server(ServerOperation),
    Measured(MeasuredOperation),
    Machine(MachineOperation),
    Read(Read),
    Write(Write),
}

impl Operation {
    pub(super) fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Write(_)
                | Self::Measured(MeasuredOperation::Write { .. })
                | Self::Machine(MachineOperation::Write { .. })
        )
    }
}

/// A write another Machine sent over SSH. It runs here, in this Machine's
/// checkout of `repository`, and is never sent on.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum MachineOperation {
    #[serde(rename = "machine.write")]
    Write {
        /// The repository `change` names; its `directory` is the sender's.
        repository: loopflow_model::durable::RepositoryId,
        /// What the sending `lf` runs with: its profile, participant name,
        /// agent override and Task source. A launch the write starts carries it.
        environment: std::collections::BTreeMap<String, String>,
        /// The one-time file under this Machine's home the sender delivered
        /// its forwarded secrets in, for the launch the write starts.
        #[serde(deserialize_with = "Option::deserialize")]
        forwarded: Option<String>,
        change: Write,
    },
}

/// The ordinary operation and its per-request work receipt in one response.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum MeasuredOperation {
    #[serde(rename = "performance.read")]
    #[schemars(extend("x-result" = generator.subschema_for::<MeasuredResult>()))]
    Read { query: Read },
    #[serde(rename = "performance.write")]
    #[schemars(extend("x-result" = generator.subschema_for::<MeasuredResult>()))]
    Write { change: Write },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MeasuredResult {
    pub result: serde_json::Value,
    pub work: loopflow_storage::instrumentation::SqlReceipt,
}

#[cfg(test)]
mod tests {
    use super::ClientMessage;

    #[test]
    fn a_write_another_machine_sent_round_trips_with_its_context() {
        let raw = r#"{"type":"request","id":"1","operation":{"kind":"machine.write","repository":"repo_30115cfb3a8ecb51494082029703a1fa","environment":{"LF_USER_NAME":"Ada"},"forwarded":".lf/forward/0f","change":{"kind":"task.hold","directory":"/laptop/repo","issue":"LOO-1"}},"select":null}"#;
        let message: ClientMessage = serde_json::from_str(raw).unwrap();
        let loopflow_execution::machine::wire::ClientMessage::Request {
            operation: super::Operation::Machine(super::MachineOperation::Write { change, .. }),
            ..
        } = &message
        else {
            panic!("not a machine write: {message:?}");
        };
        assert!(change.directory().ends_with("repo"));
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            serde_json::from_str::<serde_json::Value>(raw).unwrap()
        );
        let missing = raw.replace(r#""forwarded":".lf/forward/0f","#, "");
        assert!(serde_json::from_str::<ClientMessage>(&missing).is_err());
    }

    #[test]
    fn shared_client_fixtures_decode_and_round_trip_without_defaults() {
        for raw in [
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/performance_write.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/performance_read.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/hello.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/status_request.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/subscribe.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/checkpoint.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/cancel.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/attach.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/attach_ticket.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/session_start.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/view_plan.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/configuration.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/task_from_session.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_create.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/session_continue.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/session_page.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/view_select.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/configure_remote.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/mobile_status_request.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/upload.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/ticket.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_run.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_ship.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_hold.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_steer.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_report.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/task_interrupt.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_wait.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/session_steer.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/session_boundary.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/session_status.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/wave_plan_bind.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/wave_plan_connect.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/wave_plan_reteam.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/repo_ci.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/repo_release.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/checkout_commit.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/checkout_sync.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/checkout_create.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/account_list.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/account_disconnect.json"
            ),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/account_connect.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/graph_run.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/skill_run.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/skill_list.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_publish.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_open.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_submit.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_arm.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_land.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_abandon.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_reconcile.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/pr_request_review.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_checks.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/pr_reviews.json"),
        ] {
            let message: ClientMessage = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(&message).unwrap(),
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        // A missing field is an error, never a default.
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"hello","revision":5,"min_revision":5}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"request","id":"b","operation":{"kind":"task.complete","issue":"A"}}"#
        )
        .is_err());
    }
}
