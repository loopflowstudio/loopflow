//! Fixed machine authority for installed artifact/store selection.
//!
//! This state lives outside every Loopflow Home. A development Home cannot
//! select the reliable store by changing `LF_HOME`, and a published Home cannot
//! hide an interrupted cross-store switch by selecting another database.

use std::collections::HashSet;
#[cfg(unix)]
use std::ffi::{CStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::durable::WorkRef;

const SCHEMA_VERSION: u32 = 1;
const ACTIVE_FILE: &str = "active.json";
const SWITCH_FILE: &str = "switch.json";
const GATE_DIRECTORY: &str = "gates/1";
pub const INSTALL_SWITCH_ENV: &str = "LF_INSTALL_SWITCH";
pub const INSTALL_SWITCH_CONTROLLER_HANDOFF_ENV: &str = "LF_INSTALL_SWITCH_CONTROLLER_HANDOFF";
static AUTHORIZED_CURRENT: OnceLock<Option<InstallSelection>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ArtifactRole {
    Cli,
    Daemon,
    App,
    AppHelper(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ArtifactIdentity {
    pub role: ArtifactRole,
    pub path: PathBuf,
    pub sha256: String,
}

impl ArtifactIdentity {
    pub fn capture(role: ArtifactRole, path: &Path) -> Result<Self> {
        let path = fs::canonicalize(path)
            .with_context(|| format!("resolve install artifact {}", path.display()))?;
        if !path.is_file() {
            return Err(anyhow!("install artifact {} is not a file", path.display()));
        }
        Ok(Self {
            role,
            sha256: file_sha256(&path)?,
            path,
        })
    }

    pub fn verify(&self) -> Result<()> {
        if !self.path.is_absolute() {
            return Err(anyhow!(
                "install artifact path {} is not absolute",
                self.path.display()
            ));
        }
        let canonical = fs::canonicalize(&self.path)
            .with_context(|| format!("resolve install artifact {}", self.path.display()))?;
        if canonical != self.path {
            return Err(anyhow!(
                "install artifact path {} is not canonical",
                self.path.display()
            ));
        }
        if !canonical.is_file() {
            return Err(anyhow!(
                "install artifact {} is not a file",
                canonical.display()
            ));
        }
        let actual = file_sha256(&canonical)?;
        if actual != self.sha256 {
            return Err(anyhow!(
                "install artifact {} digest mismatch: expected {}, got {}",
                canonical.display(),
                self.sha256,
                actual
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InstallSource {
    Published,
    Development,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ArtifactSet {
    pub id: String,
    pub source: InstallSource,
    pub source_revision: String,
    pub source_identity: String,
    pub content_sha256: String,
    pub artifacts: Vec<ArtifactIdentity>,
}

impl ArtifactSet {
    pub fn verify(&self, required_roles: &[ArtifactRole]) -> Result<()> {
        let roles = self.validate_structure()?;
        for artifact in &self.artifacts {
            artifact.verify()?;
        }
        let cli = self
            .artifact(&ArtifactRole::Cli)
            .expect("validated artifact set has a CLI");
        let daemon = self
            .artifact(&ArtifactRole::Daemon)
            .expect("validated artifact set has a daemon");
        let app = self
            .artifact(&ArtifactRole::App)
            .map(|artifact| app_bundle_for_executable(&artifact.path))
            .transpose()?;
        let actual = artifact_set_sha256(&cli.path, &daemon.path, app)?;
        if actual != self.content_sha256 {
            return Err(anyhow!(
                "install artifact set {} content digest mismatch: expected {}, got {}",
                self.id,
                self.content_sha256,
                actual
            ));
        }
        for role in required_roles {
            if !roles.contains(role) {
                return Err(anyhow!(
                    "install artifact set {} is missing role {:?}",
                    self.id,
                    role
                ));
            }
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<HashSet<ArtifactRole>> {
        if self.id.is_empty() {
            return Err(anyhow!("install artifact set id is empty"));
        }
        if self.source_revision.is_empty() || self.source_identity.is_empty() {
            return Err(anyhow!(
                "install artifact set {} has incomplete source identity",
                self.id
            ));
        }
        if self.content_sha256.len() != 64
            || !self
                .content_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(anyhow!(
                "install artifact set {} has an invalid content digest",
                self.id
            ));
        }
        let mut roles = HashSet::new();
        for artifact in &self.artifacts {
            if !roles.insert(artifact.role.clone()) {
                return Err(anyhow!(
                    "install artifact set {} repeats role {:?}",
                    self.id,
                    artifact.role
                ));
            }
        }
        for role in [ArtifactRole::Cli, ArtifactRole::Daemon] {
            if !roles.contains(&role) {
                return Err(anyhow!(
                    "install artifact set {} is missing role {:?}",
                    self.id,
                    role
                ));
            }
        }
        Ok(roles)
    }

    pub fn artifact(&self, role: &ArtifactRole) -> Option<&ArtifactIdentity> {
        self.artifacts
            .iter()
            .find(|artifact| &artifact.role == role)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InstallSelection {
    pub installation_id: String,
    pub source: InstallSource,
    pub artifact_set: ArtifactSet,
    pub store: PathBuf,
}

impl InstallSelection {
    fn validate(&self) -> Result<()> {
        if self.installation_id.is_empty() {
            return Err(anyhow!("installation id is empty"));
        }
        if self.source != self.artifact_set.source {
            return Err(anyhow!(
                "installation {} source disagrees with artifact set {}",
                self.installation_id,
                self.artifact_set.id
            ));
        }
        self.artifact_set.validate_structure()?;
        if !self.store.is_absolute() {
            return Err(anyhow!(
                "installation {} store {} is not absolute",
                self.installation_id,
                self.store.display()
            ));
        }
        let canonical = crate::store::canonicalize_with_missing_tail(&self.store)
            .with_context(|| format!("resolve install store {}", self.store.display()))?;
        if canonical != self.store {
            return Err(anyhow!(
                "installation {} store {} is not canonical",
                self.installation_id,
                self.store.display()
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ActiveInstall {
    pub schema_version: u32,
    pub selection: InstallSelection,
    pub published_fallback: ArtifactSet,
    pub retained_published_sets: Vec<ArtifactSet>,
}

impl ActiveInstall {
    pub fn validate(&self) -> Result<()> {
        require_schema(self.schema_version)?;
        self.selection.validate()?;
        if self.published_fallback.source != InstallSource::Published {
            return Err(anyhow!("published fallback artifact set is not published"));
        }
        self.published_fallback.validate_structure()?;
        if self.selection.source == InstallSource::Published
            && (self.selection.artifact_set.source_revision
                != self.published_fallback.source_revision
                || self.selection.artifact_set.source_identity
                    != self.published_fallback.source_identity
                || self.selection.artifact_set.content_sha256
                    != self.published_fallback.content_sha256)
        {
            return Err(anyhow!(
                "published installation {} does not match its retained fallback source",
                self.selection.installation_id
            ));
        }
        if self
            .retained_published_sets
            .iter()
            .any(|set| set.source != InstallSource::Published)
        {
            return Err(anyhow!(
                "retained published artifact sets contain development bytes"
            ));
        }
        for set in &self.retained_published_sets {
            set.validate_structure()?;
        }
        if !self
            .retained_published_sets
            .iter()
            .any(|set| set == &self.published_fallback)
        {
            return Err(anyhow!(
                "published fallback {} is not retained by the active install",
                self.published_fallback.id
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwitchPhase {
    Planned,
    Quiesced,
    TargetPrepared,
    Advancing,
    Activated,
    Settled,
}

impl SwitchPhase {
    fn order(self) -> u8 {
        match self {
            Self::Planned => 0,
            Self::Quiesced => 1,
            Self::TargetPrepared => 2,
            Self::Advancing => 3,
            Self::Activated => 4,
            Self::Settled => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryOwner {
    Coordinator,
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ActivationTargets {
    pub cli: PathBuf,
    pub daemon: PathBuf,
    pub app: Option<PathBuf>,
    pub legacy_app: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ControllerHandoffState {
    Captured,
    Quiesced,
    Parked { parked_attempt_id: String },
    Restarted { target_attempt_id: String },
}

impl ControllerHandoffState {
    fn validate_transition_from(&self, prior: &Self) -> bool {
        match (prior, self) {
            (Self::Captured, Self::Captured | Self::Quiesced | Self::Parked { .. })
            | (Self::Quiesced, Self::Quiesced | Self::Parked { .. } | Self::Restarted { .. }) => {
                true
            }
            (Self::Parked { .. }, Self::Parked { .. })
            | (Self::Restarted { .. }, Self::Restarted { .. }) => self == prior,
            _ => false,
        }
    }

    pub(crate) fn is_settled(&self) -> bool {
        matches!(self, Self::Parked { .. } | Self::Restarted { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ControllerHandoff {
    pub work: WorkRef,
    pub tmux_name: String,
    pub prior_attempt_id: String,
    pub state: ControllerHandoffState,
}

impl ActivationTargets {
    fn validate(&self) -> Result<()> {
        for path in [&self.cli, &self.daemon] {
            if !path.is_absolute() {
                return Err(anyhow!(
                    "install activation target {} is not absolute",
                    path.display()
                ));
            }
        }
        for path in [self.app.as_deref(), self.legacy_app.as_deref()]
            .into_iter()
            .flatten()
        {
            if !path.is_absolute() {
                return Err(anyhow!(
                    "install activation target {} is not absolute",
                    path.display()
                ));
            }
        }
        if self.legacy_app.is_some() && self.app.is_none() {
            return Err(anyhow!(
                "legacy app activation target requires the current app target"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SwitchReceipt {
    pub schema_version: u32,
    pub id: String,
    pub prior: InstallSelection,
    pub target: InstallSelection,
    pub published_fallback: ArtifactSet,
    pub target_published_fallback: Option<ArtifactSet>,
    pub phase: SwitchPhase,
    pub recovery_owner: RecoveryOwner,
    pub target_store_advance_started: bool,
    pub target_store_advanced: bool,
    pub active_selection_committed: bool,
    pub coordinator: ArtifactIdentity,
    pub candidate: ArtifactIdentity,
    pub activation: ActivationTargets,
    pub app_was_running: bool,
    pub disposable_store_owned: bool,
    pub controller_handoffs: Option<Vec<ControllerHandoff>>,
}

impl SwitchReceipt {
    pub fn validate(&self) -> Result<()> {
        require_schema(self.schema_version)?;
        if self.id.is_empty() {
            return Err(anyhow!("install switch id is empty"));
        }
        self.prior.validate()?;
        self.target.validate()?;
        if self.published_fallback.source != InstallSource::Published {
            return Err(anyhow!("install switch fallback is not published"));
        }
        self.published_fallback.validate_structure()?;
        match (&self.target.source, &self.target_published_fallback) {
            (InstallSource::Published, Some(fallback)) => {
                if fallback.source != InstallSource::Published {
                    return Err(anyhow!(
                        "install switch {} target fallback is not published",
                        self.id
                    ));
                }
                fallback.validate_structure()?;
                if fallback.source_revision != self.target.artifact_set.source_revision
                    || fallback.source_identity != self.target.artifact_set.source_identity
                    || fallback.content_sha256 != self.target.artifact_set.content_sha256
                {
                    return Err(anyhow!(
                        "install switch {} target fallback names different published source bytes",
                        self.id
                    ));
                }
            }
            (InstallSource::Published, None) => {
                return Err(anyhow!(
                    "published install switch {} has no retained target fallback",
                    self.id
                ))
            }
            (InstallSource::Development, Some(_)) => {
                return Err(anyhow!(
                    "development install switch {} carries a published target fallback",
                    self.id
                ))
            }
            (InstallSource::Development, None) => {}
        }
        let prior_cli = self
            .prior
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .ok_or_else(|| anyhow!("install switch {} prior set has no CLI", self.id))?;
        let target_cli = self
            .target
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .ok_or_else(|| anyhow!("install switch {} target set has no CLI", self.id))?;
        let candidate_owned_bootstrap =
            self.target.source == InstallSource::Development && self.coordinator == *target_cli;
        if self.coordinator != *prior_cli && !candidate_owned_bootstrap {
            return Err(anyhow!(
                "install switch {} coordinator matches neither the prior CLI nor its candidate-owned bootstrap",
                self.id
            ));
        }
        if self.candidate != *target_cli {
            return Err(anyhow!(
                "install switch {} candidate does not match the target CLI",
                self.id
            ));
        }
        self.activation.validate()?;
        if self.disposable_store_owned
            && (self.target.source != InstallSource::Development
                || self.target.store == self.prior.store)
        {
            return Err(anyhow!(
                "install switch {} claims a disposable store it did not create",
                self.id
            ));
        }
        let mut works = HashSet::new();
        let mut transports = HashSet::new();
        for handoff in self.controller_handoffs.iter().flatten() {
            if matches!(handoff.work, WorkRef::Wave(_)) {
                return Err(anyhow!(
                    "install switch {} controller handoff cannot target Wave Work",
                    self.id
                ));
            }
            if handoff.tmux_name.is_empty() {
                return Err(anyhow!(
                    "install switch {} controller handoff has no transport name",
                    self.id
                ));
            }
            if !works.insert(handoff.work.clone()) {
                return Err(anyhow!(
                    "install switch {} repeats controller Work {} {}",
                    self.id,
                    handoff.work.kind(),
                    handoff.work.id()
                ));
            }
            if !transports.insert(handoff.tmux_name.as_str()) {
                return Err(anyhow!(
                    "install switch {} repeats controller transport {}",
                    self.id,
                    handoff.tmux_name
                ));
            }
            if handoff.prior_attempt_id.is_empty() {
                return Err(anyhow!(
                    "install switch {} controller handoff has no prior attempt",
                    self.id
                ));
            }
            match &handoff.state {
                ControllerHandoffState::Parked { parked_attempt_id }
                    if parked_attempt_id.is_empty() =>
                {
                    return Err(anyhow!(
                        "install switch {} controller handoff has no parked attempt",
                        self.id
                    ));
                }
                ControllerHandoffState::Restarted { target_attempt_id }
                    if target_attempt_id.is_empty()
                        || target_attempt_id == &handoff.prior_attempt_id =>
                {
                    return Err(anyhow!(
                        "install switch {} controller handoff has no distinct target attempt",
                        self.id
                    ));
                }
                _ => {}
            }
        }
        if self.target_store_advanced && !self.target_store_advance_started {
            return Err(anyhow!(
                "install switch {} records a committed advance that never started",
                self.id
            ));
        }
        if self.target_store_advance_started && self.recovery_owner != RecoveryOwner::Candidate {
            return Err(anyhow!(
                "install switch {} started target advance without candidate recovery ownership",
                self.id
            ));
        }
        if self.phase.order() >= SwitchPhase::Advancing.order()
            && !self.target_store_advance_started
        {
            return Err(anyhow!(
                "install switch {} reached {:?} before target advance started",
                self.id,
                self.phase
            ));
        }
        if self.phase.order() >= SwitchPhase::Activated.order() && !self.target_store_advanced {
            return Err(anyhow!(
                "install switch {} reached {:?} before target advance committed",
                self.id,
                self.phase
            ));
        }
        if self.active_selection_committed && self.phase != SwitchPhase::Settled {
            return Err(anyhow!(
                "install switch {} commits active selection before settlement",
                self.id
            ));
        }
        if self.phase == SwitchPhase::Settled && !self.active_selection_committed {
            return Err(anyhow!(
                "settled install switch {} has no committed active selection",
                self.id
            ));
        }
        Ok(())
    }

    fn validate_transition_from(&self, prior: &Self) -> Result<()> {
        let identity_changed = self.schema_version != prior.schema_version
            || self.id != prior.id
            || self.prior != prior.prior
            || self.coordinator != prior.coordinator
            || self.candidate != prior.candidate
            || self.activation != prior.activation
            || self.target.installation_id != prior.target.installation_id
            || self.target.source != prior.target.source
            || self.target.store != prior.target.store
            || self.target.artifact_set.source != prior.target.artifact_set.source
            || self.target.artifact_set.source_revision
                != prior.target.artifact_set.source_revision
            || self.target.artifact_set.source_identity
                != prior.target.artifact_set.source_identity
            || self.target.artifact_set.content_sha256 != prior.target.artifact_set.content_sha256
            || self.target.artifact_set.artifact(&ArtifactRole::Cli)
                != prior.target.artifact_set.artifact(&ArtifactRole::Cli)
            || self.target.artifact_set.artifact(&ArtifactRole::Daemon)
                != prior.target.artifact_set.artifact(&ArtifactRole::Daemon)
            || self.target_published_fallback != prior.target_published_fallback;
        if identity_changed {
            return Err(anyhow!(
                "install switch {} cannot change its pinned artifact/store identity",
                self.id
            ));
        }
        if prior.app_was_running && !self.app_was_running {
            return Err(anyhow!(
                "install switch {} cannot forget that the app was running",
                self.id
            ));
        }
        if prior.disposable_store_owned && !self.disposable_store_owned {
            return Err(anyhow!(
                "install switch {} cannot forget its disposable store ownership",
                self.id
            ));
        }
        if let Some(previous_handoffs) = &prior.controller_handoffs {
            let handoffs = self.controller_handoffs.as_ref().ok_or_else(|| {
                anyhow!(
                    "install switch {} cannot forget captured controllers",
                    self.id
                )
            })?;
            if handoffs.len() != previous_handoffs.len() {
                return Err(anyhow!(
                    "install switch {} cannot change its captured controller set",
                    self.id
                ));
            }
            for (handoff, previous) in handoffs.iter().zip(previous_handoffs) {
                if handoff.work != previous.work
                    || handoff.tmux_name != previous.tmux_name
                    || handoff.prior_attempt_id != previous.prior_attempt_id
                    || !handoff.state.validate_transition_from(&previous.state)
                {
                    return Err(anyhow!(
                        "install switch {} cannot change captured controller identity or regress its handoff",
                        self.id
                    ));
                }
            }
        }
        if self.published_fallback != prior.published_fallback
            && !(self.phase == SwitchPhase::Settled
                && self.target.source == InstallSource::Published
                && self.target_published_fallback.as_ref() == Some(&self.published_fallback))
        {
            return Err(anyhow!(
                "install switch {} changed its published fallback before settlement",
                self.id
            ));
        }
        if self.phase.order() < prior.phase.order() {
            return Err(anyhow!(
                "install switch {} cannot regress from {:?} to {:?}",
                self.id,
                prior.phase,
                self.phase
            ));
        }
        if prior.recovery_owner == RecoveryOwner::Candidate
            && self.recovery_owner != RecoveryOwner::Candidate
        {
            return Err(anyhow!(
                "install switch {} cannot return recovery ownership to the coordinator",
                self.id
            ));
        }
        for (name, was_set, is_set) in [
            (
                "target_store_advance_started",
                prior.target_store_advance_started,
                self.target_store_advance_started,
            ),
            (
                "target_store_advanced",
                prior.target_store_advanced,
                self.target_store_advanced,
            ),
            (
                "active_selection_committed",
                prior.active_selection_committed,
                self.active_selection_committed,
            ),
        ] {
            if was_set && !is_set {
                return Err(anyhow!(
                    "install switch {} cannot clear committed field {name}",
                    self.id
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MachineInstallState {
    Legacy,
    Settled(Box<ActiveInstall>),
    Switching(Box<SwitchReceipt>),
}

pub fn root_for_home(home: &Path) -> PathBuf {
    home.join(".lf-machine/install")
}

#[cfg(unix)]
pub fn account_home() -> Result<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(home) = std::env::var_os("LF_TEST_ACCOUNT_HOME").filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    // `HOME` is process configuration. Install authority belongs to the OS
    // account and must not move when a child changes its inherited environment.
    // SAFETY: `geteuid` has no preconditions and returns the calling process uid.
    let uid = unsafe { libc::geteuid() };
    let mut buffer = vec![0_u8; 4096];
    loop {
        // SAFETY: a zeroed `passwd` is a valid output destination for
        // `getpwuid_r`; every pointer field is populated before it is read.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result = std::ptr::null_mut();
        // SAFETY: `entry`, `buffer`, and `result` remain live and exclusively
        // borrowed for the call. The buffer length matches its allocation.
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status))
                .context("resolve OS account home directory");
        }
        if result.is_null() || entry.pw_dir.is_null() {
            return Err(anyhow!("OS account {uid} has no home directory"));
        }
        // SAFETY: a successful `getpwuid_r` returns `pw_dir` as a NUL-terminated
        // string backed by `buffer`, which is still live.
        let bytes = unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes();
        if bytes.is_empty() {
            return Err(anyhow!("OS account {uid} has an empty home directory"));
        }
        return Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())));
    }
}

#[cfg(not(unix))]
pub fn account_home() -> Result<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(home) = std::env::var_os("LF_TEST_ACCOUNT_HOME").filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    dirs::home_dir().ok_or_else(|| anyhow!("resolve OS account home directory"))
}

pub fn root() -> Result<PathBuf> {
    Ok(root_for_home(&account_home()?))
}

pub fn entry_gate_path(root: &Path, role: &ArtifactRole) -> Result<PathBuf> {
    let name = match role {
        ArtifactRole::Cli => "lf",
        ArtifactRole::Daemon => "lfd",
        other => return Err(anyhow!("artifact role {other:?} has no machine entry gate")),
    };
    Ok(root.join(GATE_DIRECTORY).join(name))
}

pub fn install_entry_gate(root: &Path, role: &ArtifactRole, source: &Path) -> Result<PathBuf> {
    let source = fs::canonicalize(source)
        .with_context(|| format!("resolve {:?} entry-gate source {}", role, source.display()))?;
    let target = entry_gate_path(root, role)?;
    let parent = target
        .parent()
        .expect("machine entry gate has a versioned parent");
    prepare_directory(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}",
        target
            .file_name()
            .expect("machine entry gate has a file name")
            .to_string_lossy(),
        Uuid::new_v4().simple()
    ));
    let result: Result<()> = (|| {
        fs::copy(&source, &temporary).with_context(|| {
            format!(
                "stage {:?} machine entry gate {} from {}",
                role,
                temporary.display(),
                source.display()
            )
        })?;
        #[cfg(unix)]
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
        File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, &target)
            .with_context(|| format!("commit machine entry gate {}", target.display()))?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(target)
}

fn _switch_capability(role: &ArtifactRole) -> Option<String> {
    if let Some(capability) = std::env::var(INSTALL_SWITCH_ENV)
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Some(capability);
    }
    if role != &ArtifactRole::Daemon {
        return None;
    }
    let mut arguments = std::env::args_os();
    while let Some(argument) = arguments.next() {
        if argument == "--install-switch" {
            return arguments.next().and_then(|value| value.into_string().ok());
        }
    }
    None
}

pub fn dispatch_entry_gate(role: &ArtifactRole) -> Result<()> {
    let root = root()?;
    let gate = entry_gate_path(&root, role)?;
    let current =
        fs::canonicalize(std::env::current_exe().context("resolve running machine entry gate")?)?;
    let gate = match fs::canonicalize(&gate) {
        Ok(gate) => gate,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("resolve machine entry gate {}", gate.display()))
        }
    };
    if current != gate {
        return Ok(());
    }

    let switch_capability = _switch_capability(role);
    let selection = match read_state(&root)? {
        MachineInstallState::Legacy => {
            return Err(anyhow!(
                "machine entry gate {} has no settled install receipt",
                gate.display()
            ))
        }
        MachineInstallState::Switching(receipt)
            if switch_capability.as_deref() == Some(receipt.id.as_str())
                && receipt.phase.order() >= SwitchPhase::Activated.order()
                && receipt.target_store_advanced =>
        {
            receipt.target
        }
        // A switch this process is not driving (typically one that failed or was
        // abandoned mid-flight) must not brick ordinary startup: dispatch through
        // the last good install instead of refusing.
        MachineInstallState::Switching(receipt) => startup_selection_during_switch(&receipt),
        MachineInstallState::Settled(active) => active.selection,
    };
    let artifact = selection
        .artifact_set
        .artifacts
        .iter()
        .find(|artifact| artifact_matches_runtime_role(&artifact.role, role))
        .ok_or_else(|| {
            anyhow!(
                "active installation {} has no {:?} artifact",
                selection.installation_id,
                role
            )
        })?;
    artifact.verify()?;
    if artifact.path == gate {
        return Err(anyhow!(
            "active installation {} routes {:?} back into its entry gate",
            selection.installation_id,
            role
        ));
    }
    let mut command = Command::new(&artifact.path);
    command.args(std::env::args_os().skip(1));
    #[cfg(unix)]
    {
        Err(command.exec()).with_context(|| {
            format!(
                "dispatch {:?} entry gate to {}",
                role,
                artifact.path.display()
            )
        })
    }
    #[cfg(not(unix))]
    {
        let status = command.status().with_context(|| {
            format!(
                "dispatch {:?} entry gate to {}",
                role,
                artifact.path.display()
            )
        })?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

pub fn read_state(root: &Path) -> Result<MachineInstallState> {
    let switch_path = root.join(SWITCH_FILE);
    if path_exists(&switch_path)? {
        let receipt: SwitchReceipt = read_json(&switch_path)?;
        receipt.validate()?;
        return Ok(MachineInstallState::Switching(Box::new(receipt)));
    }
    let active_path = root.join(ACTIVE_FILE);
    if path_exists(&active_path)? {
        let active: ActiveInstall = read_json(&active_path)?;
        active.validate()?;
        return Ok(MachineInstallState::Settled(Box::new(active)));
    }
    Ok(MachineInstallState::Legacy)
}

/// The install selection ordinary startup should use while a switch receipt is
/// present but this process is not the one driving that switch. A committed
/// switch has already made its target the active install; anything earlier —
/// including a switch that failed or was abandoned mid-flight — falls back to
/// the prior settled selection. This keeps ordinary `lf` running the last good
/// install instead of refusing every command until the switch is recovered: a
/// failed promotion must never brick the CLI.
fn startup_selection_during_switch(receipt: &SwitchReceipt) -> InstallSelection {
    if receipt.active_selection_committed {
        receipt.target.clone()
    } else {
        receipt.prior.clone()
    }
}

/// The same fallback expressed as an `ActiveInstall`, for the authorization
/// paths that resolve the running executable against a full install.
fn startup_active_during_switch(receipt: &SwitchReceipt) -> ActiveInstall {
    ActiveInstall {
        schema_version: receipt.schema_version,
        selection: startup_selection_during_switch(receipt),
        published_fallback: receipt.published_fallback.clone(),
        retained_published_sets: vec![receipt.published_fallback.clone()],
    }
}

pub fn write_switch(root: &Path, receipt: &SwitchReceipt) -> Result<()> {
    receipt.validate()?;
    let path = root.join(SWITCH_FILE);
    if path_exists(&path)? {
        let existing: SwitchReceipt = read_json(&path)?;
        existing.validate()?;
        if existing.id != receipt.id {
            return Err(anyhow!(
                "install switch {} is already unsettled; refusing switch {}",
                existing.id,
                receipt.id
            ));
        }
        receipt.validate_transition_from(&existing)?;
    }
    write_atomic_json(root, &path, receipt)
}

pub fn write_active(root: &Path, active: &ActiveInstall) -> Result<()> {
    active.validate()?;
    match read_state(root)? {
        MachineInstallState::Legacy => {}
        MachineInstallState::Settled(existing) if *existing == *active => return Ok(()),
        MachineInstallState::Settled(existing) => {
            return Err(anyhow!(
                "active installation {} may change only through an install switch",
                existing.selection.installation_id
            ))
        }
        MachineInstallState::Switching(_) => {
            return Err(anyhow!(
                "an install switch is unsettled; refusing to replace active selection"
            ))
        }
    }
    write_atomic_json(root, &root.join(ACTIVE_FILE), active)
}

pub fn clear_switch(root: &Path, switch_id: &str) -> Result<()> {
    let path = root.join(SWITCH_FILE);
    let receipt: SwitchReceipt = read_json(&path)?;
    receipt.validate()?;
    if receipt.id != switch_id {
        return Err(anyhow!(
            "install switch {} is active, not {}",
            receipt.id,
            switch_id
        ));
    }
    if receipt.target_store_advance_started {
        return Err(anyhow!(
            "install switch {} handed recovery to the candidate and cannot be cleared",
            receipt.id
        ));
    }
    remove_file_and_sync(root, &path)
}

pub fn settle_switch(root: &Path, receipt: &SwitchReceipt, active: &ActiveInstall) -> Result<()> {
    receipt.validate()?;
    active.validate()?;
    if receipt.phase != SwitchPhase::Settled || !receipt.active_selection_committed {
        return Err(anyhow!("install switch {} is not settled", receipt.id));
    }
    if active.selection != receipt.target {
        return Err(anyhow!(
            "settled active selection does not match install switch {} target",
            receipt.id
        ));
    }
    let switch_path = root.join(SWITCH_FILE);
    if !path_exists(&switch_path)? {
        let active_path = root.join(ACTIVE_FILE);
        let archive_path = root.join("receipts").join(format!("{}.json", receipt.id));
        if !path_exists(&active_path)? || !path_exists(&archive_path)? {
            return Err(anyhow!(
                "install switch {} was never persisted and cannot settle",
                receipt.id
            ));
        }
        let settled: ActiveInstall = read_json(&active_path)?;
        settled.validate()?;
        let archived: SwitchReceipt = read_json(&archive_path)?;
        archived.validate()?;
        if settled == *active && archived == *receipt {
            return Ok(());
        }
        return Err(anyhow!(
            "install switch {} is absent and its settled evidence differs",
            receipt.id
        ));
    }
    let current: SwitchReceipt = read_json(&switch_path)?;
    current.validate()?;
    if current != *receipt {
        return Err(anyhow!(
            "install switch {} changed before settlement",
            receipt.id
        ));
    }
    write_atomic_json(root, &root.join(ACTIVE_FILE), active)?;
    write_immutable_json(
        root,
        &root.join("receipts").join(format!("{}.json", receipt.id)),
        receipt,
    )?;
    remove_file_and_sync(root, &switch_path)
}

pub fn authorize(
    root: &Path,
    executable: &Path,
    role: &ArtifactRole,
) -> Result<Option<InstallSelection>> {
    authorize_for_switch(root, executable, role, None)
}

fn authorize_for_switch(
    root: &Path,
    executable: &Path,
    role: &ArtifactRole,
    switch_id: Option<&str>,
) -> Result<Option<InstallSelection>> {
    let active = match read_state(root)? {
        MachineInstallState::Legacy => return Ok(None),
        MachineInstallState::Switching(receipt) => {
            if switch_id == Some(receipt.id.as_str())
                && receipt.phase.order() >= SwitchPhase::Advancing.order()
                && receipt.target_store_advance_started
            {
                let actual = fs::canonicalize(executable).with_context(|| {
                    format!("resolve switch startup executable {}", executable.display())
                })?;
                let actual_sha256 = file_sha256(&actual)?;
                let expected = receipt
                    .target
                    .artifact_set
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        artifact_matches_runtime_role(&artifact.role, role)
                            && actual_sha256 == artifact.sha256
                    })
                    .ok_or_else(|| {
                        anyhow!(
                            "install switch {} does not authorize {} as {:?}",
                            receipt.id,
                            actual.display(),
                            role
                        )
                    })?;
                expected.verify()?;
                return Ok(Some(receipt.target));
            }
            // Not the switch this process drives: authorize against the last good
            // install so a failed or in-flight switch cannot brick ordinary startup.
            startup_active_during_switch(&receipt)
        }
        MachineInstallState::Settled(active) => *active,
    };
    let actual = fs::canonicalize(executable)
        .with_context(|| format!("resolve running executable {}", executable.display()))?;
    let actual_sha256 = file_sha256(&actual)?;
    let Some(expected) = active
        .selection
        .artifact_set
        .artifacts
        .iter()
        .find(|artifact| {
            artifact_matches_runtime_role(&artifact.role, role) && actual_sha256 == artifact.sha256
        })
    else {
        if active
            .retained_published_sets
            .iter()
            .flat_map(|set| &set.artifacts)
            .chain(&active.published_fallback.artifacts)
            .any(|artifact| {
                artifact_matches_runtime_role(&artifact.role, role)
                    && actual_sha256 == artifact.sha256
            })
        {
            return Err(anyhow!(
                "inactive retained install artifact {} cannot start as {:?}",
                actual.display(),
                role
            ));
        }
        return Ok(None);
    };
    expected.verify()?;
    Ok(Some(active.selection))
}

fn artifact_matches_runtime_role(artifact: &ArtifactRole, runtime: &ArtifactRole) -> bool {
    artifact == runtime
        || matches!(
            (artifact, runtime),
            (ArtifactRole::AppHelper(name), ArtifactRole::Cli) if name == "lf"
        )
        || matches!(
            (artifact, runtime),
            (ArtifactRole::AppHelper(name), ArtifactRole::Daemon) if name == "lfd"
        )
}

pub fn authorize_current(role: &ArtifactRole) -> Result<Option<InstallSelection>> {
    authorize_current_for_switch(role, None)
}

pub fn authorize_current_for_switch(
    role: &ArtifactRole,
    switch_id: Option<&str>,
) -> Result<Option<InstallSelection>> {
    let selection = authorize_for_switch(
        &root()?,
        &std::env::current_exe().context("resolve running install artifact")?,
        role,
        switch_id,
    )?;
    if let Some(existing) = AUTHORIZED_CURRENT.get() {
        if existing != &selection {
            return Err(anyhow!(
                "machine install selection changed after process startup"
            ));
        }
    } else {
        let _ = AUTHORIZED_CURRENT.set(selection.clone());
    }
    Ok(selection)
}

pub fn selection_for_executable(
    root: &Path,
    executable: &Path,
) -> Result<Option<InstallSelection>> {
    let active = match read_state(root)? {
        MachineInstallState::Legacy => return Ok(None),
        // A failed or in-flight switch resolves through the last good install so
        // ordinary startup keeps working instead of refusing every command.
        MachineInstallState::Switching(receipt) => startup_active_during_switch(&receipt),
        MachineInstallState::Settled(active) => *active,
    };
    let actual = fs::canonicalize(executable)
        .with_context(|| format!("resolve running executable {}", executable.display()))?;
    let actual_size = fs::metadata(&actual)
        .with_context(|| format!("inspect running executable {}", actual.display()))?
        .len();
    let plausible_copies = active
        .selection
        .artifact_set
        .artifacts
        .iter()
        .filter(|artifact| {
            fs::metadata(&artifact.path).is_ok_and(|metadata| metadata.len() == actual_size)
        })
        .collect::<Vec<_>>();
    if plausible_copies.is_empty() {
        return Ok(None);
    }
    let actual_sha256 = file_sha256(&actual)?;
    let Some(expected) = plausible_copies
        .into_iter()
        .find(|artifact| artifact.sha256 == actual_sha256)
    else {
        return Ok(None);
    };
    expected.verify()?;
    Ok(Some(active.selection))
}

pub fn selection_for_current_executable() -> Result<Option<InstallSelection>> {
    if let Some(selection) = AUTHORIZED_CURRENT.get() {
        return Ok(selection.clone());
    }
    selection_for_executable(
        &root()?,
        &std::env::current_exe().context("resolve running install artifact")?,
    )
}

fn require_schema(schema_version: u32) -> Result<()> {
    if schema_version != SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported machine install schema {schema_version}; expected {SCHEMA_VERSION}"
        ));
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read artifact {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn app_bundle_for_executable(path: &Path) -> Result<&Path> {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| {
            anyhow!(
                "app artifact {} is not inside an app bundle",
                path.display()
            )
        })
}

fn update_tree_digest(root: &Path, path: &Path, digest: &mut Sha256) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("resolve bundle path {}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect bundle path {}", path.display()))?;
    digest.update(relative.as_os_str().as_encoded_bytes());
    digest.update([0]);
    #[cfg(unix)]
    digest.update(metadata.permissions().mode().to_le_bytes());
    #[cfg(not(unix))]
    digest.update([u8::from(metadata.permissions().readonly())]);
    if metadata.file_type().is_symlink() {
        digest.update(b"symlink\0");
        digest.update(
            fs::read_link(path)
                .with_context(|| format!("read bundle symlink {}", path.display()))?
                .as_os_str()
                .as_encoded_bytes(),
        );
    } else if metadata.is_file() {
        digest.update(b"file\0");
        digest.update(
            fs::read(path).with_context(|| format!("read bundle file {}", path.display()))?,
        );
    } else if metadata.is_dir() {
        digest.update(b"directory\0");
        let mut entries = fs::read_dir(path)
            .with_context(|| format!("read bundle directory {}", path.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort();
        for entry in entries {
            update_tree_digest(root, &entry, digest)?;
        }
    } else {
        return Err(anyhow!("unsupported app entry {}", path.display()));
    }
    digest.update([0xff]);
    Ok(())
}

pub(crate) fn tree_sha256(path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    update_tree_digest(path, path, &mut digest)?;
    Ok(hex::encode(digest.finalize()))
}

pub(crate) fn artifact_set_sha256(cli: &Path, daemon: &Path, app: Option<&Path>) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(file_sha256(cli)?.as_bytes());
    digest.update(file_sha256(daemon)?.as_bytes());
    if let Some(app) = app {
        digest.update(tree_sha256(app)?.as_bytes());
    }
    Ok(hex::encode(digest.finalize()))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn path_exists(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn write_atomic_json<T: Serialize>(root: &Path, path: &Path, value: &T) -> Result<()> {
    prepare_directory(root)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("install receipt {} has no parent", path.display()))?;
    prepare_directory(parent)?;
    let bytes = json_bytes(value)?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    write_private_file(&temporary, &bytes, false)?;
    fs::rename(&temporary, path)
        .with_context(|| format!("commit machine install receipt {}", path.display()))?;
    sync_directory(parent)
}

fn write_immutable_json<T: Serialize>(root: &Path, path: &Path, value: &T) -> Result<()> {
    prepare_directory(root)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("install receipt {} has no parent", path.display()))?;
    prepare_directory(parent)?;
    let bytes = json_bytes(value)?;
    match write_private_file(path, &bytes, true) {
        Ok(()) => sync_directory(parent),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == ErrorKind::AlreadyExists) =>
        {
            let existing = fs::read(path)
                .with_context(|| format!("read immutable receipt {}", path.display()))?;
            if existing == bytes {
                Ok(())
            } else {
                Err(anyhow!(
                    "immutable install receipt {} already has different contents",
                    path.display()
                ))
            }
        }
        Err(error) => Err(error),
    }
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn prepare_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8], create_new: bool) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(!create_new);
    if create_new {
        options.create_new(true);
    }
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .with_context(|| format!("write {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn remove_file_and_sync(root: &Path, path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_directory(
            path.parent()
                .ok_or_else(|| anyhow!("install receipt {} has no parent", path.display()))?,
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if path_exists(root)? {
                sync_directory(root)?;
            }
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
    }
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("sync directory {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_artifact(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    fn artifact_set(directory: &Path, id: &str, source: InstallSource) -> ArtifactSet {
        let cli = directory.join(format!("{id}-lf"));
        let daemon = directory.join(format!("{id}-lfd"));
        write_artifact(&cli, &format!("{id} cli"));
        write_artifact(&daemon, &format!("{id} daemon"));
        ArtifactSet {
            id: id.to_string(),
            source,
            source_revision: format!("revision-{id}"),
            source_identity: format!("identity-{id}"),
            content_sha256: artifact_set_sha256(&cli, &daemon, None).unwrap(),
            artifacts: vec![
                ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap(),
                ArtifactIdentity::capture(ArtifactRole::Daemon, &daemon).unwrap(),
            ],
        }
    }

    fn selection(directory: &Path, id: &str, source: InstallSource) -> InstallSelection {
        let directory = fs::canonicalize(directory).unwrap();
        InstallSelection {
            installation_id: format!("install-{id}"),
            source,
            artifact_set: artifact_set(&directory, id, source),
            store: directory.join(format!("{id}.db")),
        }
    }

    fn active(target: InstallSelection, published_fallback: ArtifactSet) -> ActiveInstall {
        ActiveInstall {
            schema_version: SCHEMA_VERSION,
            selection: target,
            retained_published_sets: vec![published_fallback.clone()],
            published_fallback,
        }
    }

    fn switch(
        prior: InstallSelection,
        target: InstallSelection,
        published_fallback: ArtifactSet,
    ) -> SwitchReceipt {
        let directory = target
            .store
            .parent()
            .expect("test selection store has a parent")
            .to_path_buf();
        SwitchReceipt {
            schema_version: SCHEMA_VERSION,
            id: "switch-test".to_string(),
            prior: prior.clone(),
            target: target.clone(),
            published_fallback,
            target_published_fallback: (target.source == InstallSource::Published)
                .then(|| target.artifact_set.clone()),
            phase: SwitchPhase::Planned,
            recovery_owner: RecoveryOwner::Coordinator,
            target_store_advance_started: false,
            target_store_advanced: false,
            active_selection_committed: false,
            coordinator: prior
                .artifact_set
                .artifact(&ArtifactRole::Cli)
                .unwrap()
                .clone(),
            candidate: target
                .artifact_set
                .artifact(&ArtifactRole::Cli)
                .unwrap()
                .clone(),
            activation: ActivationTargets {
                cli: directory.join("active-lf"),
                daemon: directory.join("active-lfd"),
                app: None,
                legacy_app: None,
            },
            app_was_running: false,
            disposable_store_owned: false,
            controller_handoffs: None,
        }
    }

    #[test]
    fn machine_install_root_is_outside_every_loopflow_home() {
        let account_home = Path::new("/Users/example");
        assert_eq!(
            root_for_home(account_home),
            Path::new("/Users/example/.lf-machine/install")
        );
    }

    #[test]
    fn artifact_identity_detects_replaced_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lf");
        write_artifact(&path, "first");
        let artifact = ArtifactIdentity::capture(ArtifactRole::Cli, &path).unwrap();
        artifact.verify().unwrap();

        write_artifact(&path, "second");
        assert!(artifact
            .verify()
            .unwrap_err()
            .to_string()
            .contains("digest mismatch"));
    }

    #[test]
    fn copied_active_artifact_keeps_its_install_selection() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let active = active(development.clone(), published.artifact_set.clone());
        write_atomic_json(&root, &root.join(ACTIVE_FILE), &active).unwrap();

        let installed_copy = directory.path().join("installed-app-helper-lf");
        fs::copy(
            &development
                .artifact_set
                .artifact(&ArtifactRole::Cli)
                .unwrap()
                .path,
            &installed_copy,
        )
        .unwrap();

        assert_eq!(
            authorize(&root, &installed_copy, &ArtifactRole::Cli).unwrap(),
            Some(development)
        );
    }

    #[test]
    fn copied_retained_artifact_cannot_claim_the_active_store() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let active = active(development, published.artifact_set.clone());
        write_atomic_json(&root, &root.join(ACTIVE_FILE), &active).unwrap();

        let inactive_copy = directory.path().join("inactive-lf");
        fs::copy(
            &published
                .artifact_set
                .artifact(&ArtifactRole::Cli)
                .unwrap()
                .path,
            &inactive_copy,
        )
        .unwrap();

        assert!(authorize(&root, &inactive_copy, &ArtifactRole::Cli)
            .unwrap_err()
            .to_string()
            .contains("inactive retained install artifact"));
    }

    #[test]
    fn artifact_set_detects_changed_app_resources() {
        let directory = tempfile::tempdir().unwrap();
        let directory = fs::canonicalize(directory.path()).unwrap();
        let cli = directory.join("lf");
        let daemon = directory.join("lfd");
        let app = directory.join("Loopflow.app");
        let app_executable = app.join("Contents/MacOS/Loopflow");
        let resource = app.join("Contents/Resources/config.json");
        fs::create_dir_all(resource.parent().unwrap()).unwrap();
        fs::create_dir_all(app_executable.parent().unwrap()).unwrap();
        write_artifact(&cli, "cli");
        write_artifact(&daemon, "daemon");
        write_artifact(&app_executable, "app");
        write_artifact(&resource, "first");
        let set = ArtifactSet {
            id: "complete-app".to_string(),
            source: InstallSource::Published,
            source_revision: "revision".to_string(),
            source_identity: "release".to_string(),
            content_sha256: artifact_set_sha256(&cli, &daemon, Some(&app)).unwrap(),
            artifacts: vec![
                ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap(),
                ArtifactIdentity::capture(ArtifactRole::Daemon, &daemon).unwrap(),
                ArtifactIdentity::capture(ArtifactRole::App, &app_executable).unwrap(),
            ],
        };
        set.verify(&[ArtifactRole::App]).unwrap();

        write_artifact(&resource, "second");
        assert!(set
            .verify(&[ArtifactRole::App])
            .unwrap_err()
            .to_string()
            .contains("content digest mismatch"));
    }

    #[test]
    fn unsettled_switch_falls_back_to_the_prior_install_for_ordinary_startup() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let active = active(published.clone(), published.artifact_set.clone());
        write_atomic_json(&root, &root.join(ACTIVE_FILE), &active).unwrap();
        let receipt = switch(
            published.clone(),
            development,
            active.published_fallback.clone(),
        );
        write_switch(&root, &receipt).unwrap();

        // The switch is still surfaced so install operations can recover it...
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Switching(found) if found.id == receipt.id
        ));
        // ...but a failed or in-flight switch must not brick the CLI: ordinary
        // startup resolves through the prior (last good) install instead of
        // refusing every command. `switch`'s coordinator is the prior CLI.
        let resolved = authorize(&root, &receipt.coordinator.path, &ArtifactRole::Cli).unwrap();
        assert_eq!(
            resolved.map(|selection| selection.installation_id),
            Some(published.installation_id)
        );
    }

    #[test]
    fn every_unsettled_switch_phase_falls_back_to_the_prior_install() {
        // No pre-commit switch phase may brick ordinary startup: each resolves
        // through the prior (last good) install rather than refusing.
        for phase in [
            SwitchPhase::Planned,
            SwitchPhase::Quiesced,
            SwitchPhase::TargetPrepared,
            SwitchPhase::Advancing,
            SwitchPhase::Activated,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().join("authority");
            let published = selection(directory.path(), "published", InstallSource::Published);
            let development =
                selection(directory.path(), "development", InstallSource::Development);
            let mut receipt = switch(
                published.clone(),
                development,
                published.artifact_set.clone(),
            );
            receipt.phase = phase;
            if matches!(phase, SwitchPhase::Advancing | SwitchPhase::Activated) {
                receipt.recovery_owner = RecoveryOwner::Candidate;
                receipt.target_store_advance_started = true;
            }
            if phase == SwitchPhase::Activated {
                receipt.target_store_advanced = true;
            }
            write_switch(&root, &receipt).unwrap();

            let resolved = authorize(&root, &receipt.coordinator.path, &ArtifactRole::Cli)
                .unwrap_or_else(|error| {
                    panic!("phase {phase:?} should not fence startup: {error}")
                });
            assert_eq!(
                resolved.map(|selection| selection.installation_id),
                Some(published.installation_id.clone()),
                "phase {phase:?} should resolve to the prior install"
            );
        }
    }

    #[test]
    fn activated_switch_allows_only_its_target_keeper_capability() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development.clone(),
            published.artifact_set.clone(),
        );
        receipt.phase = SwitchPhase::Activated;
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        receipt.target_store_advanced = true;
        write_switch(&root, &receipt).unwrap();

        let selected = authorize_for_switch(
            &root,
            &receipt.candidate.path,
            &ArtifactRole::Cli,
            Some(&receipt.id),
        )
        .unwrap()
        .unwrap();
        assert_eq!(selected, development);
        // A different switch id no longer fences startup — it falls back to the
        // prior install, where this candidate binary is not a member, so it is
        // simply not authorized (None) rather than granted the target selection.
        assert!(authorize_for_switch(
            &root,
            &receipt.candidate.path,
            &ArtifactRole::Cli,
            Some("different-switch"),
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn advancing_switch_allows_only_its_candidate_capability() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development.clone(),
            published.artifact_set.clone(),
        );
        receipt.phase = SwitchPhase::Advancing;
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        write_switch(&root, &receipt).unwrap();

        assert_eq!(
            authorize_for_switch(
                &root,
                &receipt.candidate.path,
                &ArtifactRole::Cli,
                Some(&receipt.id),
            )
            .unwrap(),
            Some(development)
        );
        assert!(authorize_for_switch(
            &root,
            &receipt.coordinator.path,
            &ArtifactRole::Cli,
            Some(&receipt.id),
        )
        .is_err());
    }

    #[test]
    fn every_local_switch_may_be_coordinated_by_its_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.coordinator = receipt.candidate.clone();

        receipt.validate().unwrap();

        let next = selection(directory.path(), "next", InstallSource::Development);
        let mut replacement = switch(receipt.target, next, published.artifact_set);
        replacement.coordinator = replacement.candidate.clone();
        replacement.validate().unwrap();
    }

    #[test]
    fn inactive_retained_artifact_cannot_start() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        write_active(&root, &active(development, published.artifact_set.clone())).unwrap();

        let retained = published.artifact_set.artifact(&ArtifactRole::Cli).unwrap();
        let error = authorize(&root, &retained.path, &ArtifactRole::Cli).unwrap_err();
        assert!(error.to_string().contains("inactive retained"), "{error}");
    }

    #[test]
    fn a_source_tree_binary_stays_outside_the_active_install() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let active = active(published.clone(), published.artifact_set.clone());
        write_active(&root, &active).unwrap();
        let source_binary = directory.path().join("source-lf");
        write_artifact(&source_binary, "source build");

        assert_eq!(
            selection_for_executable(&root, &source_binary).unwrap(),
            None
        );
        assert_eq!(
            authorize(&root, &source_binary, &ArtifactRole::Cli).unwrap(),
            None
        );
    }

    #[test]
    fn settlement_commits_target_then_archives_immutable_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development.clone(),
            published.artifact_set.clone(),
        );
        write_switch(&root, &receipt).unwrap();
        receipt.phase = SwitchPhase::Settled;
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        receipt.target_store_advanced = true;
        receipt.active_selection_committed = true;
        let active = active(development.clone(), published.artifact_set);

        write_switch(&root, &receipt).unwrap();
        settle_switch(&root, &receipt, &active).unwrap();
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Settled(found) if found.selection == development
        ));
        assert!(!root.join(SWITCH_FILE).exists());
        assert!(root.join("receipts/switch-test.json").is_file());

        settle_switch(&root, &receipt, &active).unwrap();
    }

    #[test]
    fn receipt_invariants_reject_unsafe_recovery_handoffs() {
        let directory = tempfile::tempdir().unwrap();
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.target_store_advance_started = true;

        assert!(receipt
            .validate()
            .unwrap_err()
            .to_string()
            .contains("without candidate recovery ownership"));
    }

    #[test]
    fn controller_handoff_terminal_attempt_cannot_change() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.controller_handoffs = Some(vec![ControllerHandoff {
            work: WorkRef::Task(crate::durable::TaskId::new()),
            tmux_name: "lf-task-controller".to_string(),
            prior_attempt_id: "attempt-prior".to_string(),
            state: ControllerHandoffState::Captured,
        }]);
        write_switch(&root, &receipt).unwrap();

        let handoff = &mut receipt
            .controller_handoffs
            .as_mut()
            .expect("controller handoff exists")[0];
        handoff.state = ControllerHandoffState::Quiesced;
        write_switch(&root, &receipt).unwrap();
        receipt.controller_handoffs.as_mut().unwrap()[0].state =
            ControllerHandoffState::Restarted {
                target_attempt_id: "attempt-target".to_string(),
            };
        write_switch(&root, &receipt).unwrap();

        receipt.controller_handoffs.as_mut().unwrap()[0].state =
            ControllerHandoffState::Restarted {
                target_attempt_id: "attempt-replacement".to_string(),
            };
        assert!(write_switch(&root, &receipt)
            .unwrap_err()
            .to_string()
            .contains("cannot change captured controller identity"));
    }

    #[test]
    fn controller_handoff_requires_a_parked_attempt() {
        let directory = tempfile::tempdir().unwrap();
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.controller_handoffs = Some(vec![ControllerHandoff {
            work: WorkRef::Project(crate::durable::ProjectId::new()),
            tmux_name: "lf-project-controller".to_string(),
            prior_attempt_id: "attempt-prior".to_string(),
            state: ControllerHandoffState::Parked {
                parked_attempt_id: String::new(),
            },
        }]);

        assert!(receipt
            .validate()
            .unwrap_err()
            .to_string()
            .contains("has no parked attempt"));
    }

    #[test]
    fn switch_cannot_be_cleared_after_candidate_handoff() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        receipt.phase = SwitchPhase::Advancing;
        write_switch(&root, &receipt).unwrap();

        let error = clear_switch(&root, &receipt.id).unwrap_err();
        assert!(error.to_string().contains("cannot be cleared"));
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Switching(found) if found.id == receipt.id
        ));
    }

    #[test]
    fn one_unsettled_switch_cannot_be_replaced_by_another() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let first = switch(
            published.clone(),
            development.clone(),
            published.artifact_set.clone(),
        );
        write_switch(&root, &first).unwrap();
        let mut second = switch(published, development, first.published_fallback.clone());
        second.id = "switch-other".to_string();

        assert!(write_switch(&root, &second)
            .unwrap_err()
            .to_string()
            .contains("switch-test is already unsettled"));
    }

    #[test]
    fn persisted_switch_cannot_regress_its_recovery_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development,
            published.artifact_set.clone(),
        );
        receipt.phase = SwitchPhase::Advancing;
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        write_switch(&root, &receipt).unwrap();
        let mut retargeted = receipt.clone();
        retargeted.target.store = fs::canonicalize(directory.path())
            .unwrap()
            .join("different.db");
        assert!(write_switch(&root, &retargeted)
            .unwrap_err()
            .to_string()
            .contains("cannot change its pinned artifact/store identity"));
        let mut regressed = receipt.clone();
        regressed.phase = SwitchPhase::TargetPrepared;
        regressed.recovery_owner = RecoveryOwner::Coordinator;
        regressed.target_store_advance_started = false;

        assert!(write_switch(&root, &regressed)
            .unwrap_err()
            .to_string()
            .contains("cannot regress"));
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Switching(found) if *found == receipt
        ));
    }

    #[test]
    fn settled_active_install_can_change_only_through_a_persisted_switch() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let published_active = active(published.clone(), published.artifact_set.clone());
        write_active(&root, &published_active).unwrap();
        let development_active = active(development, published.artifact_set);

        assert!(write_active(&root, &development_active)
            .unwrap_err()
            .to_string()
            .contains("may change only through an install switch"));
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Settled(found) if *found == published_active
        ));
    }

    #[test]
    fn settlement_requires_the_exact_persisted_switch() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut receipt = switch(
            published.clone(),
            development.clone(),
            published.artifact_set.clone(),
        );
        receipt.phase = SwitchPhase::Settled;
        receipt.recovery_owner = RecoveryOwner::Candidate;
        receipt.target_store_advance_started = true;
        receipt.target_store_advanced = true;
        receipt.active_selection_committed = true;
        let active = active(development, published.artifact_set);

        assert!(settle_switch(&root, &receipt, &active)
            .unwrap_err()
            .to_string()
            .contains("was never persisted"));
        assert!(matches!(
            read_state(&root).unwrap(),
            MachineInstallState::Legacy
        ));
    }

    #[test]
    fn unknown_receipt_schema_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("authority");
        let published = selection(directory.path(), "published", InstallSource::Published);
        let mut active = active(published.clone(), published.artifact_set);
        active.schema_version = 99;
        write_atomic_json(&root, &root.join(ACTIVE_FILE), &active).unwrap();

        assert!(read_state(&root)
            .unwrap_err()
            .to_string()
            .contains("unsupported machine install schema 99"));
    }

    #[test]
    fn active_install_requires_its_exact_published_fallback_to_be_retained() {
        let directory = tempfile::tempdir().unwrap();
        let published = selection(directory.path(), "published", InstallSource::Published);
        let development = selection(directory.path(), "development", InstallSource::Development);
        let mut active = active(development, published.artifact_set.clone());
        let replacement = artifact_set(
            &fs::canonicalize(directory.path()).unwrap(),
            "replacement",
            InstallSource::Published,
        );
        active.retained_published_sets = vec![replacement];

        assert!(active
            .validate()
            .unwrap_err()
            .to_string()
            .contains("is not retained by the active install"));
    }
}
