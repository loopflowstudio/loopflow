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
    Read(Read),
    Write(Write),
}

impl Operation {
    pub(super) fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Write(_) | Self::Measured(MeasuredOperation::Write { .. })
        )
    }

    /// The directory a command names, whose repository it reads or changes.
    pub(super) fn directory(&mut self) -> Option<std::path::PathBuf> {
        match self {
            Self::Read(read) => read.directory_mut().cloned(),
            Self::Write(write) => write.directory_mut().cloned(),
            Self::Server(_) | Self::Measured(_) => None,
        }
    }
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
    fn a_request_another_machine_sent_carries_its_secrets_and_never_prints_them() {
        let raw = include_str!(
            "../../../loopflow-execution/tests/fixtures/dto/server/machine_request.json"
        );
        let message: ClientMessage = serde_json::from_str(raw).unwrap();
        let loopflow_execution::machine::wire::ClientMessage::Request {
            sender: Some(sender),
            ..
        } = &message
        else {
            panic!("no sender: {message:?}");
        };
        assert_eq!(sender.secrets.0[0].0, "CLAUDE_CODE_OAUTH_TOKEN");
        assert!(!format!("{message:?}").contains("sk-fixture"));
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            serde_json::from_str::<serde_json::Value>(raw).unwrap()
        );
    }

    #[test]
    fn shared_client_fixtures_decode_and_round_trip_without_defaults() {
        for raw in crate::server::schema::REQUESTS.iter().copied().chain([
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/attach_ticket.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/cancel.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/checkpoint.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/hello.json"),
            include_str!(
                "../../../loopflow-execution/tests/fixtures/dto/server/status_request.json"
            ),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/subscribe.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/task_wait.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/ticket.json"),
            include_str!("../../../loopflow-execution/tests/fixtures/dto/server/view_select.json"),
        ]) {
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
