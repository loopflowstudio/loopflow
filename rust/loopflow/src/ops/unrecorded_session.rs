//! A launch never waits on bookkeeping. When the store cannot take a
//! conversation's rows, its reservation waits beside the Run until an `lf`
//! operation that touches that Run can record it. Until then it does not list.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::durable::RunId;
use crate::session::{Run, Session};
use crate::store::SharedStore;

const RESERVATION: &str = "unrecorded-session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Reservation {
    pub(crate) session: Session,
    pub(crate) run: Run,
}

fn path(run_id: &RunId) -> Result<PathBuf> {
    crate::run_record::record_dir(&crate::store::observability_home_dir(), run_id)
        .map(|dir| dir.join(RESERVATION))
        .ok_or_else(|| anyhow!("Run {run_id} has no record directory"))
}

pub(crate) fn defer(reservation: &Reservation) -> Result<()> {
    std::fs::write(
        path(&reservation.run.id)?,
        serde_json::to_vec_pretty(reservation)?,
    )
    .context("keep the Session reservation beside its Run")
}

pub(crate) fn read(run_id: &RunId) -> Result<Option<Reservation>> {
    match std::fs::read(path(run_id)?) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).context("parse Session reservation")?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("read Session reservation"),
    }
}

pub(crate) fn forget(run_id: &RunId) -> Result<()> {
    match std::fs::remove_file(path(run_id)?) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).context("remove Session reservation")
        }
        _ => Ok(()),
    }
}

/// Store the Run's waiting reservation, if it has one.
pub(crate) async fn record(store: &SharedStore, run_id: &RunId) -> Result<Option<(Session, Run)>> {
    let Some(Reservation { session, run }) = read(run_id)? else {
        return Ok(None);
    };
    let recorded = store.create_session(session, run).await?;
    forget(run_id)?;
    Ok(Some(recorded))
}
