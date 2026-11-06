//! The `repository` namespace's operations: `repository.ci`, the Machine's
//! record of failed-CI recovery, and `repository.release`, which runs the
//! release workflow detached ([`super::launch`]) and reports the Process that
//! carries it. The other `lf repository release` verbs are its steps and stay
//! CLI commands.

use std::path::Path;

use super::{launch, Error, ErrorCode, Result};
use crate::lf::commands::ci::CiReportDto;
use crate::ops::{Progress, ReleaseRunOutcome};

/// `repository.ci`: how failed CI was detected, repaired and landed since
/// `since` (`7d`, `24h`, or an RFC 3339 instant), as `lf repository ci` does.
pub fn ci(since: &str, wave: Option<&str>, repo: Option<&str>) -> Result<CiReportDto> {
    crate::lf::commands::ci::report(since, wave, repo)
        .map_err(|error| Error::refused(&error, ErrorCode::Unavailable))
}

/// `repository.release`: start `lf repository release run [<version>] [-t
/// <target>]` detached in the repository containing `directory`, and return
/// the Process that carries it. `version` is `patch` (the default), `minor`,
/// `major` or `X.Y.Z`.
pub fn release(
    directory: &Path,
    version: Option<&str>,
    target: Option<&str>,
) -> Result<launch::Launched> {
    let mut args = ["repository", "release", "run"].map(str::to_owned).to_vec();
    args.extend(version.map(str::to_owned));
    if let Some(target) = target {
        args.extend(["--target".to_owned(), target.to_owned()]);
    }
    args.push("--json".to_owned());
    launch::start(directory, &args)
}

/// `lf repository release run` in this process: the whole workflow, through
/// to the tag and the published artifacts. Started by [`release`], it reports
/// its Process first and carries on in its own log.
pub fn carry_release(
    repo: &Path,
    version: Option<&str>,
    target: Option<&str>,
    progress: &impl Progress,
) -> Result<ReleaseRunOutcome> {
    let version = version.unwrap_or("patch");
    launch::carry(
        "repository.release",
        format!(
            "release {version} ({})",
            target.unwrap_or("the default target")
        ),
    )
    .map_err(|error| Error::refused(&error, ErrorCode::Unavailable))?;
    crate::ops::release_run(repo, version, target, progress)
        .map_err(|error| Error::outcome_unknown(&error.into()))
}

#[cfg(test)]
mod tests {
    use super::{CiReportDto, ReleaseRunOutcome};

    #[test]
    fn receipts_match_the_shared_fixtures_without_defaults() {
        let ci: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/server/repository_ci_report.json"
        ))
        .unwrap();
        let report: CiReportDto = serde_json::from_value(ci["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&report).unwrap(), ci["result"]);
        assert_eq!(report.summary.incidents, report.incidents.len());
        let released: ReleaseRunOutcome = serde_json::from_value(serde_json::json!({
            "state": "released",
            "target": "lf",
            "version": "0.9.1",
            "tag": "v0.9.1",
            "commit": "0123abc",
            "workflow_run_id": 7,
            "workflow_url": null,
            "release_exists": true
        }))
        .unwrap();
        assert!(matches!(released, ReleaseRunOutcome::Released(_)));
        assert!(
            serde_json::from_str::<CiReportDto>(r#"{"since":"2026-11-01T00:00:00Z"}"#).is_err()
        );
    }
}
