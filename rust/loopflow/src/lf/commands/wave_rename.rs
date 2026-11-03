//! Rename or relocate an authored Wave and its provider mapping.
use crate::id::WaveId;
use crate::lf::WaveCommand;
use crate::store::{open_store, storage_config_from_env};
use anyhow::{anyhow, Context};
use std::path::Path;

pub fn rename(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    let WaveCommand::Rename {
        wave,
        repo: target,
        name,
        title,
        json,
    } = command
    else {
        unreachable!("Wave rename dispatcher");
    };
    tokio::runtime::Runtime::new()?.block_on(async {
        let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
        let store = std::sync::Arc::new(
            open_store(&config)
                .await
                .context("open the shared Loopflow store")?,
        );
        let wave_id = match WaveId::parse(wave) {
            Ok(id) => id,
            Err(_) => crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(repo),
                Some(wave),
                None,
            )
            .await?
            .id()
            .clone(),
        };
        if let Some(title) = title {
            let selected = store
                .get_wave(&wave_id)
                .await?
                .ok_or_else(|| anyhow!("Wave {wave_id} not found"))?;
            crate::ops::linear::rename(
                repo,
                &crate::ops::linear::LinearRenameOptions {
                    wave: Some(selected.slug().to_string()),
                    title: title.clone(),
                },
                &crate::ops::NullProgress,
            )
            .await?;
        }
        let relocated = crate::work::wave::relocate::relocate_wave(
            &store,
            &wave_id,
            repo,
            target.as_deref(),
            name.as_deref(),
        )
        .await?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&relocated)?);
        } else {
            println!("Wave {wave_id}: renamed");
        }
        Ok(())
    })
}
