//! Recovery advice is an observation of retained bytes and their own store.

use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use super::{Compatibility, PromotionPreview, Verdict};
use crate::build_info::MigrationAuthority;
use crate::engine::process::shell_escape;
use crate::machine_install::{self, ArtifactRole, InstallSelection, InstallSource};

pub(crate) fn development_store_recovery() -> String {
    match machine_install::root().and_then(|root| recovery_route(&root)) {
        Ok(route) => route,
        Err(error) => format!(
            "No compatible retained executable/database pair was verified: {error}. Keep the affected data directory; recover the build matching its draft receipts before reopening it."
        ),
    }
}

fn recovery_route(root: &Path) -> Result<String> {
    let current = machine_install::current_selection(root)?;
    let selections = machine_install::known_installations(root)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut failures = Vec::new();
    for selection in selections {
        if selection.source != InstallSource::Development {
            continue;
        }
        let active = current.as_ref() == Some(&selection);
        let receipt =
            machine_install::retained_development_receipt(root, &selection.installation_id)
                .ok()
                .filter(|receipt| receipt.target == selection);
        if !active && receipt.is_none() {
            continue;
        }
        if Instant::now() >= deadline {
            failures.push("retained-build inspection exceeded 10 seconds".to_string());
            break;
        }
        if let Err(error) = verify_pair(&selection, deadline) {
            failures.push(format!("{}: {error}", selection.installation_id));
            continue;
        }
        let cli = selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .expect("verified CLI");
        let home = selection.store.parent().expect("absolute store has parent");
        let pair = format!(
            "Verified retained pair: executable {}, data directory {}, database {}. Its exact-store preflight requires no migration. Returning to that database preserves its history; it does not repair or transfer the affected database's private writes.",
            cli.path.display(), home.display(), selection.store.display()
        );
        if active {
            return Ok(format!(
                "{pair}\nContinue with the installed executable: {}",
                shell_escape(&cli.path.to_string_lossy())
            ));
        }
        let activation = receipt
            .expect("inactive pair has a settled receipt")
            .activation;
        let mut command = format!(
            "{} install promote --from-build {} --reuse-home {} --cli-target {} --preview",
            shell_escape(&cli.path.to_string_lossy()),
            shell_escape(&cli.path.to_string_lossy()),
            shell_escape(&selection.installation_id),
            shell_escape(&activation.cli.to_string_lossy()),
        );
        if let Some(target) = activation.app {
            let Some(app) = selection.artifact_set.artifact(&ArtifactRole::App) else {
                failures.push(format!(
                    "{}: retained app bytes are missing",
                    selection.installation_id
                ));
                continue;
            };
            let source = machine_install::app_bundle_for_executable(&app.path)?;
            command.push_str(&format!(
                " --app-source {} --app-target {}",
                shell_escape(&source.to_string_lossy()),
                shell_escape(&target.to_string_lossy()),
            ));
        }
        return Ok(format!(
            "{pair}\nOutside Task execution, preview restoration of this retained pair:\n  {command}\nReview the preview before applying it without --preview. No installation was changed."
        ));
    }
    if failures.is_empty() {
        Err(anyhow!(
            "no retained development installation evidence is available"
        ))
    } else {
        Err(anyhow!("{}", failures.join("; ")))
    }
}

fn verify_pair(selection: &InstallSelection, deadline: Instant) -> Result<()> {
    selection.artifact_set.verify(&[ArtifactRole::Cli])?;
    let cli = selection
        .artifact_set
        .artifact(&ArtifactRole::Cli)
        .expect("verified CLI");
    let output = tempfile::NamedTempFile::new()?;
    let mut command = Command::new(&cli.path);
    // Inspection has no inherited execution authority, even in a Task worker.
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    let mut child = command
        .args(["install", "local-preflight", "--store"])
        .arg(&selection.store)
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(output.reopen()?)
        .stderr(Stdio::null())
        .spawn()
        .context("start retained executable preflight")?;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("retained executable preflight timed out"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !status.success() {
        return Err(anyhow!(
            "retained executable rejected its database ({status})"
        ));
    }
    let preview: PromotionPreview = serde_json::from_reader(File::open(output.path())?)
        .context("retained executable returned no readable compatibility evidence")?;
    if preview.candidate.source_revision != selection.artifact_set.source_revision
        || preview.candidate.source_identity != selection.artifact_set.source_identity
        || preview.candidate.authority != MigrationAuthority::ValidationOnly
        || Path::new(&preview.database_path) != selection.store
        || !matches!(preview.compatibility, Compatibility::Exact { .. })
        || preview.verdict != Verdict::Promote
    {
        return Err(anyhow!(
            "retained executable did not verify its recorded identity and exact store"
        ));
    }
    selection.artifact_set.verify(&[ArtifactRole::Cli])
}
