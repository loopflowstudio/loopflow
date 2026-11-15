//! The `wave` namespace's planning binding: `wave.plan.bind`, `.connect`
//! and `.reteam` tie a Wave's plan to the repository's tracker. `wave.sync`
//! is the explicit exchange; the planning reads (`wave.plan`,
//! `plan.list`, `wave.list`) read the stored snapshot and exchange nothing.

use std::path::Path;

use super::{Error, ErrorCode, Result};
use crate::ops::linear::{LinearInitResult, LinearReteamResult};
use crate::ops::Progress;
use crate::work::project::Project;

/// `wave.plan.bind`: bind the existing tracker Project `project` as the
/// Wave's shared current selection, as `lf wave plan bind <wave>
/// <project>` does, and return the Project as stored.
pub fn planning_bind(directory: &Path, wave: &str, project: &str) -> Result<Project> {
    let repo = repo_root(directory)?;
    tokio::runtime::Runtime::new()
        .map_err(written)?
        .block_on(crate::ops::project::bind_project(&repo, wave, project))
        .map_err(written)
}

/// `wave.plan.connect`: connect `wave` (the ambient Wave with none, every
/// authored Wave with `all`) to its tracker Initiative and the repository's
/// Team, as `lf wave plan connect` does; one receipt per Wave connected.
pub fn planning_connect(
    directory: &Path,
    wave: Option<&str>,
    all: bool,
    team_key: Option<&str>,
    team_name: Option<&str>,
    progress: &impl Progress,
) -> Result<Vec<LinearInitResult>> {
    let repo = repo_root(directory)?;
    let targets = if all {
        // A clone's authored directories register before the store is listed.
        tokio::runtime::Runtime::new()
            .map_err(written)?
            .block_on(crate::ops::wave_lifecycle::register_authored(&repo))
            .map_err(written)?;
        crate::ops::linear::list_local_waves(&repo).map_err(written)?
    } else {
        // An explicit name the store does not know is added first by
        // `connect`, so it is only normalized here; ambient uses the
        // validating resolver.
        let name = match wave {
            Some(raw) => crate::ops::normalize_wave_name(raw).ok_or_else(|| {
                Error::new(
                    ErrorCode::Invalid,
                    "wave connect requires a non-empty wave name",
                )
            })?,
            None => crate::lf::commands::ops::planning_wave(&repo, None)
                .map_err(written)?
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::Invalid,
                        "cannot determine wave; run `lf wave plan connect <name>`",
                    )
                })?,
        };
        vec![name]
    };
    targets
        .into_iter()
        .map(|wave| {
            crate::ops::linear::connect(
                &repo,
                &crate::ops::linear::LinearInitOptions {
                    wave: Some(wave),
                    team_key: team_key.map(str::to_owned),
                    team_name: team_name.map(str::to_owned),
                },
                progress,
            )
            .map_err(written)
        })
        .collect()
}

/// `wave.plan.reteam`: reconcile the linked Waves' Projects and Tasks onto
/// the repository's tracker Team, as `lf wave plan reteam [--apply]`
/// does. Without `apply` the receipt is the plan and nothing moves.
pub fn planning_reteam(
    directory: &Path,
    apply: bool,
    progress: &impl Progress,
) -> Result<LinearReteamResult> {
    let repo = repo_root(directory)?;
    crate::ops::linear::reteam(
        &repo,
        &crate::ops::linear::LinearReteamOptions { apply },
        progress,
    )
    .map_err(written)
}

fn repo_root(directory: &Path) -> Result<std::path::PathBuf> {
    crate::repo::require_repo_root(directory, "wave planning")
        .map_err(|error| Error::refused(&error, ErrorCode::Invalid))
}

fn written(error: impl Into<anyhow::Error>) -> Error {
    Error::written(&error.into())
}

#[cfg(test)]
mod tests {
    use super::{LinearInitResult, LinearReteamResult};

    /// The receipts the fixtures carry decode without defaults and encode back
    /// unchanged.
    #[test]
    fn planning_receipts_match_the_shared_fixtures() {
        let connected: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dto/server/wave_plan_connected.json"
        ))
        .unwrap();
        let results: Vec<LinearInitResult> =
            serde_json::from_value(connected["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&results).unwrap(), connected["result"]);
        assert_eq!(results[0].wave, "product");
        let reteamed: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dto/server/wave_plan_reteamed.json"
        ))
        .unwrap();
        let result: LinearReteamResult =
            serde_json::from_value(reteamed["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&result).unwrap(), reteamed["result"]);
        assert!(!result.applied);
        assert!(
            serde_json::from_str::<LinearReteamResult>(r#"{"repository":"loopflow"}"#).is_err()
        );
    }
}
