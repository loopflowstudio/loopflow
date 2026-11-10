use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::json;

use crate::store::{StoreError, StoreResult};

/// Connect before claiming dispatch. The caller records its receipt and fences
/// `send` with the owning provider context; a socket write is never an ack.
#[derive(Debug)]
pub(super) struct Inbox(UnixStream);

impl Inbox {
    pub(super) fn connect(path: &str) -> StoreResult<Self> {
        let socket = UnixStream::connect(path).map_err(io_error)?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(io_error)?;
        Ok(Self(socket))
    }

    pub(super) fn send(
        &mut self,
        token: &str,
        thread: &str,
        request: &str,
        content: &str,
    ) -> StoreResult<()> {
        let wire = format!(
            "{}\n{}\n",
            json!({"type":"auth","token":token}),
            json!({"type":"user","session_id":thread,"uuid":request,
                "priority":"next","from":"loopflow", "message":{"role":"user","content":content}})
        );
        self.0.write_all(wire.as_bytes()).map_err(io_error)
    }
}

fn io_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}
