//! Deterministic action interleavings; only native process effects are simulated.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::sync::Mutex;

use anyhow::Result;
use fs2::FileExt;
use sha2::{Digest, Sha256};

static CLIENTS: Mutex<Option<HashMap<String, (String, bool)>>> = Mutex::new(None);

#[derive(Debug)]
pub(crate) struct NativeClients;

impl NativeClients {
    pub fn new(session: &str, runs: &[String]) -> Self {
        assert!(CLIENTS
            .lock()
            .unwrap()
            .replace(
                runs.iter()
                    .map(|run| { (run.clone(), (session.into(), true)) })
                    .collect()
            )
            .is_none());
        Self
    }

    pub fn active(&self) -> HashSet<String> {
        CLIENTS
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, (_, active))| *active)
            .map(|(run, _)| run.clone())
            .collect()
    }
}

impl Drop for NativeClients {
    fn drop(&mut self) {
        CLIENTS.lock().unwrap().take();
    }
}

fn assert_launch_locked(session: &str) {
    let name = hex::encode(&Sha256::digest(session.as_bytes())[..16]);
    let path = crate::store::lf_home_dir()
        .join(super::LAUNCH_LOCK_DIRECTORY)
        .join(format!(".{name}.launch.lock"));
    let probe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    assert_eq!(
        FileExt::try_lock_exclusive(&probe).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "native handoff must exclude replacement and completion"
    );
}

pub(super) fn resume(run: &String, lock: &mut Option<File>) -> Option<Result<bool>> {
    let mut clients = CLIENTS.lock().unwrap();
    let (session, active) = clients.as_mut()?.get_mut(run)?;
    assert!(lock.is_some());
    assert_launch_locked(session);
    *active = true;
    // Simulate publication of the new client's receipt, then release startup.
    drop(lock.take());
    Some(Ok(true))
}

pub(super) fn stop(run: &String) -> bool {
    let mut clients = CLIENTS.lock().unwrap();
    let Some((session, active)) = clients.as_mut().and_then(|clients| clients.get_mut(run)) else {
        return false;
    };
    assert_launch_locked(session);
    *active = false;
    true
}
