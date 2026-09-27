//! Deterministic action interleavings; only native process effects are simulated.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use crate::durable::RunId;

#[derive(Debug, Clone)]
pub(crate) struct LookupPause {
    action: &'static str,
    selector: String,
    pub reached: Arc<Notify>,
    pub proceed: Arc<Notify>,
}

static PAUSE: Mutex<Option<LookupPause>> = Mutex::new(None);
static CLIENTS: Mutex<Option<HashMap<RunId, (String, bool)>>> = Mutex::new(None);

impl LookupPause {
    pub fn at(action: &'static str, selector: &str) -> Self {
        let pause = Self {
            action,
            selector: selector.into(),
            reached: Arc::new(Notify::new()),
            proceed: Arc::new(Notify::new()),
        };
        assert!(PAUSE.lock().unwrap().replace(pause.clone()).is_none());
        pause
    }
}

pub(super) async fn after_lookup(action: &str, selector: &str) {
    let pause = PAUSE
        .lock()
        .unwrap()
        .as_ref()
        .filter(|pause| pause.action == action && pause.selector == selector)
        .cloned();
    if let Some(pause) = pause {
        pause.reached.notify_one();
        pause.proceed.notified().await;
        PAUSE.lock().unwrap().take();
    }
}

#[derive(Debug)]
pub(crate) struct NativeClients;

impl NativeClients {
    pub fn new(session: &str, runs: &[RunId]) -> Self {
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

    pub fn add(&self, session: &str, run: &RunId) {
        CLIENTS
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert(run.clone(), (session.into(), true));
    }

    pub fn active(&self) -> HashSet<RunId> {
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
        PAUSE.lock().unwrap().take();
    }
}

fn assert_launch_locked(session: &str) {
    let name = hex::encode(&Sha256::digest(session.as_bytes())[..16]);
    let path = crate::store::current_home_lf_home_dir()
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

pub(super) fn resume(run: &RunId, lock: &mut Option<File>) -> Option<Result<bool>> {
    let mut clients = CLIENTS.lock().unwrap();
    let (session, active) = clients.as_mut()?.get_mut(run)?;
    assert!(lock.is_some());
    assert_launch_locked(session);
    *active = true;
    // Simulate publication of the new client's receipt, then release startup.
    drop(lock.take());
    Some(Ok(true))
}

pub(super) fn stop(run: &RunId) -> bool {
    let mut clients = CLIENTS.lock().unwrap();
    let Some((session, active)) = clients.as_mut().and_then(|clients| clients.get_mut(run)) else {
        return false;
    };
    assert_launch_locked(session);
    *active = false;
    true
}
