//! `lf server stdio`: one connection's requests on stdin and stdout, for
//! another Machine sending a typed operation over SSH. Each request runs here
//! in `directory`, the repository `--repo-id` named, and is never forwarded.
//! Subscriptions and the server's own operations belong to the resident server.

use std::io::{BufRead, Write};
use std::path::Path;

use loopflow_execution::engine::{Error, ErrorCode};

use super::wire::{
    Hello, Limits, Operation, ServerMessage, Welcome, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION,
    REVISION,
};

type Message = loopflow_execution::machine::wire::ClientMessage;

pub fn run(directory: &Path, input: impl BufRead, mut output: impl Write) -> anyhow::Result<()> {
    let mut lines = input.lines();
    let Some(first) = lines.next().transpose()? else {
        return Ok(());
    };
    let hello = match serde_json::from_str::<Message>(&first) {
        Ok(Message::Hello(hello)) => hello,
        Ok(_) => {
            return send(
                &mut output,
                &refused(None, "the first message must be hello"),
            )
        }
        Err(error) => return send(&mut output, &refused(None, error.to_string())),
    };
    let Some(revision) = super::service::negotiate(&hello) else {
        return send(&mut output, &incompatible(&hello));
    };
    send(&mut output, &welcome(revision)?)?;
    for line in lines {
        let answer = match serde_json::from_str::<Message>(&line?) {
            Ok(Message::Request {
                id,
                operation,
                select: None,
            }) => answer(id, directory, operation),
            Ok(Message::Request { id, .. }) => {
                refused(Some(id), "select needs the resident server")
            }
            Ok(Message::Subscribe { id, .. }) => refused(
                Some(id),
                "subscriptions need the resident server; `lf server stdio` answers requests",
            ),
            Ok(_) => refused(None, "`lf server stdio` answers requests"),
            Err(error) => refused(None, error.to_string()),
        };
        send(&mut output, &answer)?;
    }
    Ok(())
}

/// Read where the request came from: the sender shows it.
fn incompatible(hello: &Hello) -> ServerMessage {
    let (age, update) = if hello.min_revision > REVISION {
        ("older", "run `lf install` on it")
    } else {
        ("newer", "update this Machine with `lf install`")
    };
    ServerMessage::error(
        None,
        Error::new(
            ErrorCode::Incompatible,
            format!(
                "the receiving Machine speaks protocol revisions {MIN_REVISION}–{REVISION}, {age} than this request's {}–{}; {update}",
                hello.min_revision, hello.revision
            ),
        )
        .with(serde_json::json!({
            "server": { "revision": REVISION, "min_revision": MIN_REVISION },
            "update": if hello.min_revision > REVISION { "server" } else { "client" },
        })),
    )
}

fn welcome(revision: u32) -> anyhow::Result<ServerMessage> {
    Ok(ServerMessage::Welcome(Welcome {
        client_id: "stdio".into(),
        revision,
        server_revision: REVISION,
        min_revision: MIN_REVISION,
        instance: uuid::Uuid::new_v4().to_string(),
        build: crate::build_info::BUILD_VERSION.to_owned(),
        executable_sha256: loopflow_execution::installation::ArtifactIdentity::capture(
            loopflow_execution::installation::ArtifactRole::Cli,
            &std::env::current_exe()?,
        )?
        .sha256,
        machine: crate::lf::commands::read::Read::MachineSelf.read().ok(),
        limits: Limits {
            max_message: MAX_MESSAGE,
            max_connections: MAX_CONNECTIONS,
            max_open: 1,
            max_retained_bytes: MAX_MESSAGE,
        },
    }))
}

/// Run `operation` here. Its `directory` is the sender's path; the operation
/// runs in this Machine's checkout of the same repository.
fn answer(id: String, directory: &Path, mut operation: serde_json::Value) -> ServerMessage {
    if let Some(sent) = operation.get_mut("directory") {
        *sent = serde_json::json!(directory);
    }
    let result = match serde_json::from_value::<Operation>(operation.clone()) {
        Ok(Operation::Read(read)) => read
            .read()
            .map_err(|error| Error::refused(&error, ErrorCode::Unavailable)),
        Ok(Operation::Write(write)) => write
            .run_here()
            .and_then(|result| Ok(serde_json::to_value(result)?))
            .map_err(|error| Error::written(&error)),
        Ok(Operation::Server(_)) => Err(Error::new(
            ErrorCode::Invalid,
            "this operation needs the resident server; `lf server stdio` answers reads and writes",
        )),
        Err(_) => Err(super::service::refused_request(&operation)),
    };
    match result {
        Ok(result) => ServerMessage::Response {
            id,
            trace_id: loopflow_model::id::TraceId::default(),
            result,
        },
        Err(error) => ServerMessage::error(Some(id), error),
    }
}

fn refused(id: Option<String>, message: impl Into<String>) -> ServerMessage {
    ServerMessage::error(id, Error::new(ErrorCode::Invalid, message))
}

fn send(output: &mut impl Write, message: &ServerMessage) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *output, message)?;
    output.write_all(b"\n")?;
    Ok(output.flush()?)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    fn answers(lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
        let mut output = Vec::new();
        super::run(std::path::Path::new("/"), input.as_bytes(), &mut output).unwrap();
        output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect()
    }

    fn hello(revision: u32) -> Value {
        json!({"type": "hello", "revision": revision, "min_revision": revision,
            "client": {"kind": "machine", "platform": "macos", "name": "lf", "version": "0"}})
    }

    #[test]
    fn requests_run_here_and_the_resident_servers_work_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let theme = directory.path().join("theme.yaml");
        let answers = answers(&[
            hello(super::REVISION),
            json!({"type": "request", "id": "1", "select": null,
                "operation": {"kind": "config.theme", "path": theme, "value": "dark"}}),
            json!({"type": "subscribe", "id": "2", "select": null, "resume": null,
                "operation": {"kind": "task.wait", "issue": "LOO-1", "until": "done"}}),
            json!({"type": "request", "id": "3", "select": null,
                "operation": {"kind": "mobile.status"}}),
            json!({"type": "request", "id": "4", "select": null,
                "operation": {"kind": "task.unknown"}}),
        ]);
        assert_eq!(answers[0]["type"], "welcome");
        assert_eq!(answers[1]["type"], "response");
        assert_eq!(answers[1]["result"]["value"], "dark");
        assert!(std::fs::read_to_string(theme).unwrap().contains("dark"));
        for (answer, id) in [(&answers[2], "2"), (&answers[3], "3")] {
            assert_eq!(answer["type"], "error");
            assert_eq!(answer["id"], id);
            assert_eq!(answer["code"], "invalid");
            assert!(answer["message"]
                .as_str()
                .unwrap()
                .contains("resident server"));
        }
        assert_eq!(
            answers[4]["message"],
            "this server has no operation `task.unknown`; update the selected installation"
        );
    }

    #[test]
    fn another_revision_is_incompatible_and_answers_nothing_more() {
        let answers = answers(&[
            hello(super::REVISION + 1),
            json!({"type": "request", "id": "1", "select": null,
                "operation": {"kind": "mobile.status"}}),
        ]);
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0]["code"], "incompatible");
        assert!(answers[0]["message"]
            .as_str()
            .unwrap()
            .contains("older than this request's"));
    }
}
