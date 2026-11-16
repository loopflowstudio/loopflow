//! Typed operations accepted by the Machine dispatcher.

use crate::lf::commands::{read::Read, write::Write};
pub use crate::machine::wire::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type ClientMessage = crate::machine::wire::ClientMessage<Operation, Read>;

/// What a request asks for, named by its `kind`: a dotted resource and verb
/// (`task.complete`, `plan.list`). `desktop.*` kinds are Desktop's own
/// arrangement state, not part of what other clients are offered.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Operation {
    Server(ServerOperation),
    Read(Read),
    Write(Write),
}

#[cfg(test)]
mod tests {
    use super::ClientMessage;

    #[test]
    fn shared_client_fixtures_decode_and_round_trip_without_defaults() {
        for raw in [
            include_str!("../../tests/fixtures/dto/server/hello.json"),
            include_str!("../../tests/fixtures/dto/server/status_request.json"),
            include_str!("../../tests/fixtures/dto/server/subscribe.json"),
            include_str!("../../tests/fixtures/dto/server/checkpoint.json"),
            include_str!("../../tests/fixtures/dto/server/cancel.json"),
            include_str!("../../tests/fixtures/dto/server/attach.json"),
            include_str!("../../tests/fixtures/dto/server/attach_ticket.json"),
            include_str!("../../tests/fixtures/dto/server/session_start.json"),
            include_str!("../../tests/fixtures/dto/server/view_plan.json"),
            include_str!("../../tests/fixtures/dto/server/configuration.json"),
            include_str!("../../tests/fixtures/dto/server/task_from_session.json"),
            include_str!("../../tests/fixtures/dto/server/task_create.json"),
            include_str!("../../tests/fixtures/dto/server/session_continue.json"),
            include_str!("../../tests/fixtures/dto/server/session_page.json"),
            include_str!("../../tests/fixtures/dto/server/view_select.json"),
            include_str!("../../tests/fixtures/dto/server/configure_remote.json"),
            include_str!("../../tests/fixtures/dto/server/mobile_status_request.json"),
            include_str!("../../tests/fixtures/dto/server/upload.json"),
            include_str!("../../tests/fixtures/dto/server/ticket.json"),
            include_str!("../../tests/fixtures/dto/server/task_run.json"),
            include_str!("../../tests/fixtures/dto/server/task_ship.json"),
            include_str!("../../tests/fixtures/dto/server/task_hold.json"),
            include_str!("../../tests/fixtures/dto/server/task_steer.json"),
            include_str!("../../tests/fixtures/dto/server/task_report.json"),
            include_str!("../../tests/fixtures/dto/server/task_interrupt.json"),
            include_str!("../../tests/fixtures/dto/server/task_wait.json"),
            include_str!("../../tests/fixtures/dto/server/session_steer.json"),
            include_str!("../../tests/fixtures/dto/server/session_boundary.json"),
            include_str!("../../tests/fixtures/dto/server/session_status.json"),
            include_str!("../../tests/fixtures/dto/server/wave_plan_bind.json"),
            include_str!("../../tests/fixtures/dto/server/wave_plan_connect.json"),
            include_str!("../../tests/fixtures/dto/server/wave_plan_reteam.json"),
            include_str!("../../tests/fixtures/dto/server/repo_ci.json"),
            include_str!("../../tests/fixtures/dto/server/repo_release.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_commit.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_sync.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_create.json"),
            include_str!("../../tests/fixtures/dto/server/account_list.json"),
            include_str!("../../tests/fixtures/dto/server/account_next_request.json"),
            include_str!("../../tests/fixtures/dto/server/account_disconnect.json"),
            include_str!("../../tests/fixtures/dto/server/account_connect.json"),
            include_str!("../../tests/fixtures/dto/server/account_set.json"),
            include_str!("../../tests/fixtures/dto/server/account_redeem_reset.json"),
            include_str!("../../tests/fixtures/dto/server/account_use.json"),
            include_str!("../../tests/fixtures/dto/server/account_pause.json"),
            include_str!("../../tests/fixtures/dto/server/account_unpause.json"),
            include_str!("../../tests/fixtures/dto/server/graph_run.json"),
            include_str!("../../tests/fixtures/dto/server/skill_run.json"),
            include_str!("../../tests/fixtures/dto/server/skill_list.json"),
            include_str!("../../tests/fixtures/dto/server/pr_publish.json"),
            include_str!("../../tests/fixtures/dto/server/pr_open.json"),
            include_str!("../../tests/fixtures/dto/server/pr_submit.json"),
            include_str!("../../tests/fixtures/dto/server/pr_arm.json"),
            include_str!("../../tests/fixtures/dto/server/pr_land.json"),
            include_str!("../../tests/fixtures/dto/server/pr_abandon.json"),
            include_str!("../../tests/fixtures/dto/server/pr_reconcile.json"),
            include_str!("../../tests/fixtures/dto/server/pr_request_review.json"),
            include_str!("../../tests/fixtures/dto/server/pr_checks.json"),
            include_str!("../../tests/fixtures/dto/server/pr_reviews.json"),
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
