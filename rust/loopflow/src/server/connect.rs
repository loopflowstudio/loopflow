//! `lf server connect`: another Machine's way in over SSH. It starts this
//! Machine's server when none is running, then copies bytes between stdin and
//! stdout and the server's socket until either side closes. It reads none of
//! them, so every message the protocol has works over SSH.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

pub fn run() -> anyhow::Result<()> {
    let path = loopflow_execution::machine::socket_path()?;
    // Connect to an already-running server directly instead of serializing
    // every SSH pipe behind `ensure()`'s install lock and binary digest.
    let socket = match UnixStream::connect(&path) {
        Ok(socket) => socket,
        Err(_) => {
            loopflow_execution::installation::server::ensure()?;
            UnixStream::connect(&path)?
        }
    };
    let mut to_server = socket.try_clone()?;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut to_server);
        let _ = to_server.shutdown(Shutdown::Write);
    });
    // Each read goes out at once: the server's answers are what the other
    // Machine waits for.
    let (mut from_server, mut stdout) = (&socket, std::io::stdout().lock());
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = from_server.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        stdout.write_all(&buffer[..read])?;
        stdout.flush()?;
    }
}
