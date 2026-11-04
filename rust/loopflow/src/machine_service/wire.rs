//! Carrier-independent Machine operations. Local admission is same-user only.
use crate::id::LfProcessId;
use crate::lf::commands::{read::Read, write::Write};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_FRAME: usize = 32 * 1024 * 1024;
pub const MAX_REQUEST: usize = 64 * 1024;
pub const MAX_CONNECTIONS: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub id: String,
    pub operation: Operation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Bootstrap,
    /// Same-user local socket only. None stops remote admission; zero chooses a port.
    ConfigureRemote {
        port: Option<u16>,
    },
    SessionOwner {
        session: crate::id::LfSessionId,
    },
    Read {
        query: Read,
    },
    Subscribe {
        query: Read,
        generation: u64,
    },
    Command {
        change: Write,
    },
    /// Switch this connection to the existing owner protocol after `bridged`.
    Attach {
        owner: LfProcessId,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub id: String,
    pub instance: String,
    pub result: Reply,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Remote {
        port: Option<u16>,
    },
    Bootstrap {
        version: u32,
        build: String,
        executable_sha256: String,
        machine: serde_json::Value,
        capabilities: Vec<String>,
        max_frame: usize,
        max_connections: usize,
    },
    Read {
        output: String,
    },
    Reading {
        generation: u64,
        frame: serde_json::Value,
    },
    Command {
        trace_id: crate::id::TraceId,
        output: String,
        stderr: String,
        code: i32,
    },
    SessionOwner {
        owner: Option<LfProcessId>,
    },
    Bridged,
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Incompatible,
    Invalid,
    Unsupported,
    Unavailable,
    OutcomeUnknown,
}

#[cfg(test)]
mod tests {
    use super::{Request, Response};

    #[test]
    fn shared_client_fixtures_decode_and_round_trip_without_defaults() {
        for raw in [
            include_str!("../../../../tests/fixtures/dto/machine/subscribe.json"),
            include_str!("../../../../tests/fixtures/dto/machine/attach.json"),
            include_str!("../../../../tests/fixtures/dto/machine/session_owner.json"),
            include_str!("../../../../tests/fixtures/dto/machine/view_plan.json"),
            include_str!("../../../../tests/fixtures/dto/machine/all_view_plans.json"),
            include_str!("../../../../tests/fixtures/dto/machine/configure_remote.json"),
        ] {
            let request: Request = serde_json::from_str(raw).unwrap();
            let actual = serde_json::to_value(&request).unwrap();
            assert_eq!(
                actual,
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        for raw in [
            include_str!("../../../../tests/fixtures/dto/machine/bootstrap.json"),
            include_str!("../../../../tests/fixtures/dto/machine/command.json"),
            include_str!("../../../../tests/fixtures/dto/machine/owner.json"),
            include_str!("../../../../tests/fixtures/dto/machine/reading.json"),
            include_str!("../../../../tests/fixtures/dto/machine/remote.json"),
        ] {
            let response: Response = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(response).unwrap(),
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        assert!(
            serde_json::from_str::<Request>(r#"{"id":"a","operation":{"kind":"bootstrap"}}"#)
                .is_err()
        );
    }
}
