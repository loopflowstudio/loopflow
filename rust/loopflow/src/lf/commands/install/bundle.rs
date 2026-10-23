//! Copied app bundles: where Loopflow.app lives and how a dragged copy becomes
//! the machine installation.
//!
//! The DMG route is "drag to Applications, open". First launch runs the
//! bundled helper as `lf install activate-app --bundle <the opened app>`. It
//! refuses a disk image or translocated copy, verifies the signature, refuses
//! to replace a newer installation, and then hands the bundle to ordinary
//! published promotion with the bundle as both source and destination. That
//! promotion records the destination, so later updates replace this copy.
//! Later launches of the settled copy return before verification or any
//! write, so a person's edited `.zshrc` stays as they left it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use super::shell_path;
use crate::build_info::MigrationAuthority;
use crate::installation::{ArtifactRole, InstallationState};

const DEFAULT_APPLICATIONS: &str = "/Applications";

/// The Loopflow.app a published update replaces: an explicit
/// `LF_APPLICATIONS_DIR`, else the recorded activation, else `/Applications`,
/// or `~/Applications` when the account cannot write `/Applications`.
pub fn app_destination() -> Result<PathBuf> {
    let explicit = std::env::var_os("LF_APPLICATIONS_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let recorded = match &explicit {
        Some(_) => None,
        None => crate::installation::active_install(&crate::installation::root()?)?
            .and_then(|active| active.activation)
            .and_then(|activation| activation.app),
    };
    Ok(choose_app_destination(
        explicit,
        recorded,
        is_writable_directory(Path::new(DEFAULT_APPLICATIONS)),
        &crate::installation::account_home()?,
    ))
}

fn choose_app_destination(
    explicit: Option<PathBuf>,
    recorded: Option<PathBuf>,
    system_writable: bool,
    account_home: &Path,
) -> PathBuf {
    if let Some(directory) = explicit {
        return directory.join("Loopflow.app");
    }
    if let Some(app) = recorded {
        return app;
    }
    if system_writable {
        return Path::new(DEFAULT_APPLICATIONS).join("Loopflow.app");
    }
    account_home.join("Applications/Loopflow.app")
}

fn is_writable_directory(path: &Path) -> bool {
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a live NUL-terminated string for the duration of the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

/// Refuse a bundle that cannot be the permanent installation, in words for the
/// person who opened it.
fn check_placement(bundle: &Path) -> Result<()> {
    let path = bundle.to_string_lossy();
    if path.contains("/AppTranslocation/") || is_read_only_volume(bundle)? {
        return Err(anyhow!(
            "Loopflow is running from the disk image or a protected download location. \
             Drag Loopflow into Applications, eject the disk image, then open Loopflow from Applications."
        ));
    }
    if bundle
        .extension()
        .is_none_or(|extension| extension != "app")
    {
        return Err(anyhow!("{} is not an app bundle", bundle.display()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_read_only_volume(path: &Path) -> Result<bool> {
    let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .with_context(|| format!("{} contains a NUL byte", path.display()))?;
    // SAFETY: a zeroed `statfs` is a valid output buffer; `c_path` outlives the call.
    let mut volume: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the duration of the call.
    if unsafe { libc::statfs(c_path.as_ptr(), &mut volume) } != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("inspect the volume holding {}", path.display()));
    }
    Ok(u64::from(volume.f_flags) & libc::MNT_RDONLY as u64 != 0)
}

#[cfg(not(target_os = "macos"))]
fn is_read_only_volume(_path: &Path) -> Result<bool> {
    Ok(false)
}

/// Gatekeeper's own acceptance of a release bundle: a valid deep signature and
/// a notarized, executable assessment.
#[cfg(target_os = "macos")]
pub(super) fn verify_signed_app(bundle: &Path) -> Result<()> {
    for (program, args) in [
        ("/usr/bin/codesign", &["--verify", "--deep", "--strict"][..]),
        ("/usr/sbin/spctl", &["--assess", "--type", "execute"][..]),
    ] {
        let output = Command::new(program)
            .args(args)
            .arg(bundle)
            .output()
            .with_context(|| format!("run {program}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "{} failed signature verification ({program}): {}",
                bundle.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_signed_app(bundle: &Path) -> Result<()> {
    Err(anyhow!(
        "{} cannot be verified: app bundles install only on macOS",
        bundle.display()
    ))
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    version
        .split(['.', '-', '+'])
        .take(3)
        .map(|part| part.parse().ok())
        .collect()
}

/// Opening an older copy must not replace a newer installation.
fn refuse_downgrade(installed: &str, opened: &str, installed_app: Option<&Path>) -> Result<()> {
    let (Some(installed_parts), Some(opened_parts)) =
        (version_parts(installed), version_parts(opened))
    else {
        return Ok(());
    };
    if opened_parts < installed_parts {
        let location = installed_app
            .map(|app| format!(" at {}", app.display()))
            .unwrap_or_default();
        return Err(anyhow!(
            "Loopflow {installed} is already installed{location}; this copy is {opened}. \
             Open the installed Loopflow, or download the latest release."
        ));
    }
    Ok(())
}

/// This bundle is already the settled installation: no switch is pending, the
/// active record activated this copy, and its CLI is this bundle's helper. A
/// pending receipt, even one past commit, still owes recovery.
fn is_settled_here(state: &InstallationState, bundle: &Path, helper_digest: &str) -> bool {
    let InstallationState::Settled(active) = state else {
        return false;
    };
    active
        .activation
        .as_ref()
        .is_some_and(|activation| activation.app.as_deref() == Some(bundle))
        && active
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .is_some_and(|cli| cli.sha256 == helper_digest)
}

/// `lf install activate-app --bundle <Loopflow.app>`.
pub fn activate_app(bundle: &Path) -> Result<()> {
    // Development builds carry a validation-only helper and are installed by
    // `scripts/install.py local`; opening one leaves the installation alone.
    let opened = super::CandidateIdentity::current();
    if opened.authority != MigrationAuthority::Published {
        println!("Development build: the installation is unchanged.");
        return Ok(());
    }
    let bundle = fs::canonicalize(bundle)
        .with_context(|| format!("resolve opened app {}", bundle.display()))?;
    check_placement(&bundle)?;
    let helper = bundle.join("Contents/MacOS/lf");
    // The opened bundle's own helper owns activation, whether it runs inside
    // the bundle or from the retained copy promotion re-execs.
    let current = std::env::current_exe().context("resolve the activating helper")?;
    let helper_digest = super::binary_digest(&helper)?;
    if super::binary_digest(&current)? != helper_digest {
        return Err(anyhow!(
            "activate {} with its bundled helper {}",
            bundle.display(),
            helper.display()
        ));
    }
    let root = crate::installation::root()?;
    // A settled copy's installed CLI is this helper, so it cannot be a downgrade.
    if is_settled_here(
        &crate::installation::read_state(&root)?,
        &bundle,
        &helper_digest,
    ) {
        println!(
            "Loopflow {} is installed at {}.",
            opened.display_version(),
            bundle.display()
        );
        return Ok(());
    }
    // Published promotion verifies the signature before any target changes.
    if let Some(active) = crate::installation::active_install(&root)? {
        let cli = active
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .expect("validated active install has a CLI");
        if let Ok(installed) = super::read_binary_preflight(&cli.path) {
            refuse_downgrade(
                installed.candidate.display_version(),
                opened.display_version(),
                active.activation.as_ref().and_then(|a| a.app.as_deref()),
            )?;
        }
    }
    let directory = super::published::install_dir()?;
    fs::create_dir_all(&directory)
        .with_context(|| format!("create install directory {}", directory.display()))?;
    let cli_target = directory.join("lf");
    super::promote(
        super::PromotionArtifacts {
            cli_target: &cli_target,
            app_source: Some(&bundle),
            app_target: Some(&bundle),
            legacy_app_target: None,
        },
        true,
        false,
    )?;
    let file = shell_path::persist(&directory)?;
    println!("Terminal: new zsh windows find lf ({})", file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        activate_app, check_placement, choose_app_destination, is_settled_here, refuse_downgrade,
    };
    use crate::installation::{
        ActivationTargets, ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet,
        InstallSelection, InstallSource, InstallationState, RecoveryOwner, SwitchPhase,
        SwitchReceipt,
    };
    use std::path::{Path, PathBuf};

    fn settled_install(directory: &Path, app: &Path) -> (ActiveInstall, String) {
        let cli = directory.join("lf");
        std::fs::write(&cli, "published lf").unwrap();
        let cli = ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap();
        let digest = cli.sha256.clone();
        let set = ArtifactSet {
            id: "published".to_string(),
            source: InstallSource::Published,
            source_revision: "revision".to_string(),
            source_identity: "identity".to_string(),
            content_sha256: digest.clone(),
            artifacts: vec![cli],
        };
        let active = ActiveInstall {
            schema_version: 1,
            selection: InstallSelection {
                installation_id: "published-1".to_string(),
                source: InstallSource::Published,
                artifact_set: set.clone(),
                store: directory.join("loopflow.db"),
            },
            published_fallback: set.clone(),
            retained_published_sets: vec![set],
            activation: Some(ActivationTargets {
                cli: directory.join("bin/lf"),
                daemon: None,
                app: Some(app.to_path_buf()),
                legacy_app: None,
            }),
        };
        (active, digest)
    }

    #[test]
    fn opening_the_settled_copy_again_skips_promotion() {
        let directory = tempfile::tempdir().unwrap();
        let app = directory.path().join("Loopflow.app");
        let (active, digest) = settled_install(directory.path(), &app);
        let settled = InstallationState::Settled(Box::new(active));
        assert!(is_settled_here(&settled, &app, &digest));
        assert!(!is_settled_here(&settled, &app, "other release"));
        assert!(!is_settled_here(
            &settled,
            &directory.path().join("Copy/Loopflow.app"),
            &digest
        ));
    }

    #[test]
    fn a_pending_switch_still_enters_promotion_for_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let app = directory.path().join("Loopflow.app");
        let (active, digest) = settled_install(directory.path(), &app);
        let cli = active
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone();
        // Committed but not archived: Home restart or app reopen is still owed.
        let receipt = SwitchReceipt {
            schema_version: 1,
            id: "switch-pending".to_string(),
            prior: None,
            target: active.selection.clone(),
            published_fallback: Some(active.published_fallback.clone()),
            target_published_fallback: Some(active.published_fallback.clone()),
            phase: SwitchPhase::Settled,
            recovery_owner: RecoveryOwner::Candidate,
            target_store_advance_started: true,
            target_store_advanced: true,
            active_selection_committed: true,
            coordinator: cli.clone(),
            candidate: cli,
            activation: active.activation.clone().unwrap(),
            app_was_running: false,
            disposable_store_owned: false,
        };
        let switching = InstallationState::Switching(Box::new(receipt));
        assert!(!is_settled_here(&switching, &app, &digest));
    }

    #[test]
    fn updates_keep_the_recorded_copy_unless_a_directory_is_explicit() {
        let home = Path::new("/Users/kim");
        let recorded = || Some(PathBuf::from("/Users/kim/Applications/Loopflow.app"));
        assert_eq!(
            choose_app_destination(None, recorded(), true, home),
            PathBuf::from("/Users/kim/Applications/Loopflow.app")
        );
        assert_eq!(
            choose_app_destination(Some("/tmp/apps".into()), recorded(), true, home),
            PathBuf::from("/tmp/apps/Loopflow.app")
        );
        assert_eq!(
            choose_app_destination(None, None, true, home),
            PathBuf::from("/Applications/Loopflow.app")
        );
        assert_eq!(
            choose_app_destination(None, None, false, home),
            PathBuf::from("/Users/kim/Applications/Loopflow.app")
        );
    }

    #[test]
    fn development_builds_leave_the_installation_alone() {
        // Test builds carry a validation-only helper, like any local app build.
        activate_app(Path::new("/nonexistent/Loopflow.app")).unwrap();
    }

    #[test]
    fn older_copy_cannot_replace_a_newer_installation() {
        let app = Path::new("/Users/kim/Applications/Loopflow.app");
        let error = refuse_downgrade("0.13.2", "0.13.1", Some(app)).unwrap_err();
        assert!(error.to_string().contains("0.13.2 is already installed at"));
        assert!(refuse_downgrade("0.13.2", "0.13.2", None).is_ok());
        assert!(refuse_downgrade("0.9.9", "0.10.0", None).is_ok());
    }

    #[test]
    fn translocated_or_unbundled_copies_explain_the_drag() {
        let translocated = Path::new("/private/var/folders/x/AppTranslocation/ABC/d/Loopflow.app");
        assert!(check_placement(translocated)
            .unwrap_err()
            .to_string()
            .contains("Drag Loopflow into Applications"));
        let directory = tempfile::tempdir().unwrap();
        assert!(check_placement(directory.path())
            .unwrap_err()
            .to_string()
            .contains("not an app bundle"));
        let app = directory.path().join("Loopflow.app");
        std::fs::create_dir(&app).unwrap();
        check_placement(&app).unwrap();
    }
}
