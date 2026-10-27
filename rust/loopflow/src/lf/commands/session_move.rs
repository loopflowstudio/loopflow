//! `lf --machine M resume <session>` moves the conversation before resuming
//! it there. `send` runs where the Session lives and `receive` on M, each
//! locally when that side is this Machine, otherwise over one SSH request, so
//! laptop → mini, rental → laptop and rental → mini are the same relay.

use anyhow::{anyhow, Context, Result};

use crate::durable::Machine;
use crate::ops::human_session::moves::{self, Move};
use crate::store::SharedStore;

/// Move Session `id`, which this Machine knows, to `target` unless it is
/// already there.
pub(crate) async fn relay(store: &SharedStore, id: &str, target: &Machine) -> Result<()> {
    let session = crate::ops::human_session::session_by_id(store, id)
        .await?
        .ok_or_else(|| anyhow!("Session {id} was not found"))?;
    let id = session.id.to_string();
    let moved = match store.sqlite.session_moved_to(&id)? {
        Some(machine) if machine == target.id => return Ok(()),
        Some(machine) => {
            let source = store.sqlite.machine_by_id(&machine)?.ok_or_else(|| {
                anyhow!("Session {id} moved to Machine {machine}, which is not registered here")
            })?;
            let bytes = super::ssh::request(
                &source,
                &[
                    "session".into(),
                    "send".into(),
                    id.clone(),
                    "--to".into(),
                    target.id.to_string(),
                ],
                None,
            )
            .await?;
            serde_json::from_slice(&bytes).context("read the moved Session")?
        }
        None => moves::send(store, &id, &target.id).await?,
    };
    receive(&moved, target).await.map_err(|error| {
        anyhow!(
            "{error:#}\nSession {id} was sent but did not arrive on {}; where it lived, retry with\n  lf session send {id} --to {} | lf --machine {} session receive",
            label(target),
            target.id,
            label(target),
        )
    })?;
    eprintln!("Moved {} to {}", session.title, label(target));
    Ok(())
}

async fn receive(moved: &Move, target: &Machine) -> Result<()> {
    super::ssh::request(
        target,
        &[
            "--repository".into(),
            moved.checkout.repository_id.to_string(),
            "session".into(),
            "receive".into(),
        ],
        Some(&serde_json::to_vec(moved)?),
    )
    .await?;
    Ok(())
}

fn label(machine: &Machine) -> String {
    machine
        .label
        .clone()
        .unwrap_or_else(|| machine.id.to_string())
}
