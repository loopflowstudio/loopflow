//! Copied app bundles: where Loopflow.app lives and how a dragged copy becomes
//! the machine installation.
//!
//! The DMG route is "drag to Applications, open". First launch runs the
//! bundled helper as `lf install activate-app --bundle <the opened app>`. It
//! refuses a disk image or translocated copy, verifies the signature, refuses
//! to replace a newer installation, and then hands the bundle to ordinary
//! published promotion with the bundle as both source and destination. That
//! promotion records the destination, so later updates replace this copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use super::shell_path;

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
        None => crate::machine_install::active_install(&crate::machine_install::root()?)?
            .and_then(|active| active.activation)
            .and_then(|activation| activation.app),
    };
    Ok(choose_app_destination(
        explicit,
        recorded,
        is_writable_directory(Path::new(DEFAULT_APPLICATIONS)),
        &crate::machine_install::account_home()?,
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

/// `lf install activate-app --bundle <Loopflow.app>`.
pub fn activate_app(bundle: &Path) -> Result<()> {
    let bundle = fs::canonicalize(bundle)
        .with_context(|| format!("resolve opened app {}", bundle.display()))?;
    check_placement(&bundle)?;
    let helper = bundle.join("Contents/MacOS/lf");
    let daemon = bundle.join("Contents/MacOS/lfd");
    // The opened bundle's own helper owns activation, whether it runs inside
    // the bundle or from the retained copy promotion re-execs.
    let current = std::env::current_exe().context("resolve the activating helper")?;
    if super::binary_digest(&current)? != super::binary_digest(&helper)? {
        return Err(anyhow!(
            "activate {} with its bundled helper {}",
            bundle.display(),
            helper.display()
        ));
    }
    // Published promotion verifies the signature before any target changes.
    let opened = super::CandidateIdentity::current();
    if let Some(active) = crate::machine_install::active_install(&crate::machine_install::root()?)?
    {
        let cli = active
            .selection
            .artifact_set
            .artifact(&crate::machine_install::ArtifactRole::Cli)
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
    let daemon_target = directory.join("lfd");
    super::promote(
        super::PromotionArtifacts {
            cli_target: &cli_target,
            daemon_source: &daemon,
            daemon_target: &daemon_target,
            app_source: Some(&bundle),
            app_target: Some(&bundle),
            legacy_app_target: None,
        },
        true,
        false,
        None,
        None,
        false,
        None,
    )?;
    let file = shell_path::persist(&directory)?;
    println!("Terminal: new zsh windows find lf ({})", file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_placement, choose_app_destination, refuse_downgrade};
    use std::path::{Path, PathBuf};

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
