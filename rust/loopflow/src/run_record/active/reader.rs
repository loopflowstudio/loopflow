use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use crate::harness::opencode_runtime::{registered_opencode_servers_at, OpenCodeServerEntry};
use crate::journal::{ExecProcessReceipt, EXEC_PROCESS_ROOT};
use crate::lf::commands::top::{sample_processes, OsProcess, ProcessSnapshot};
use crate::run_record::{read_manifest, ProviderClientRef};

use super::events::{Changes, Subscription};
use super::{ActiveRunsSnapshot, DiscoveryState, RunBinding};

type Observation = (
    BTreeMap<PathBuf, RunBinding>,
    ProcessSnapshot,
    Vec<(crate::durable::RunId, ProviderClientRef, String)>,
    Vec<String>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Receipt {
    Native(ProviderClientRef),
    Capture {
        binding: RunBinding,
        // Namespace presence resolves ownerless capture attribution, not liveness.
        // None means an ownership event invalidated this cached observation.
        native_history: Option<bool>,
    },
    Exec(ExecProcessReceipt),
}

#[cfg(all(test, target_os = "macos"))]
mod tests;

impl Receipt {
    fn live(&self, processes: &HashMap<u32, &OsProcess>) -> bool {
        let (pid, start, tolerance) = match self {
            Self::Native(client) => (client.pid, client.started_at.unix_timestamp(), 5),
            Self::Exec(exec) => (exec.pid, exec.started_at, 3),
            Self::Capture { binding, .. } => match &binding.owner {
                Some(owner) => (owner.pid, owner.started_at, 3),
                None => return true, // Unknown ownership must remain visible.
            },
        };
        processes
            .get(&pid)
            .is_some_and(|p| p.matches_start(pid, start, tolerance))
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub(super) struct DiscoveryCost {
    pub directories: usize,
    pub receipts: usize,
    pub manifests: usize,
    pub bytes: usize,
    pub process_samples: usize,
    pub rescans: usize,
    pub retained: usize,
    pub dirty_paths: usize,
}

impl DiscoveryCost {
    pub(super) fn manifest(
        &mut self,
        dir: &Path,
    ) -> std::io::Result<crate::run_record::RunManifest> {
        self.manifests += 1;
        self.bytes += fs::metadata(dir.join("manifest.json"))
            .map(|m| m.len() as usize)
            .unwrap_or(0);
        read_manifest(dir)
    }
}

#[derive(Debug)]
pub(crate) struct ActiveRunReader {
    home: PathBuf,
    home_identity: Option<(u64, u64)>,
    subscription: Option<Subscription>,
    candidates: BTreeMap<PathBuf, Receipt>,
    errors: BTreeMap<PathBuf, String>,
    servers: Vec<OpenCodeServerEntry>,
    rescan: bool,
    pending: Changes,
    cancel: CancellationToken,
    #[cfg(test)]
    scan_started: Option<std::sync::mpsc::SyncSender<()>>,
    pub(super) cost: DiscoveryCost,
}

impl ActiveRunReader {
    pub(crate) fn start(home: &Path, continuous: bool, cancel: CancellationToken) -> Result<Self> {
        // Resolve aliases once: FSEvents returns canonical paths (not /tmp aliases).
        let ancestor = home
            .ancestors()
            .find(|p| p.exists())
            .context("Home has no existing ancestor")?;
        let home = ancestor.canonicalize()?.join(home.strip_prefix(ancestor)?);
        let subscription = if continuous {
            Some(Subscription::start(&home)?)
        } else {
            None
        };
        let home_identity = fs::metadata(&home).ok().map(|m| (m.dev(), m.ino()));
        Ok(Self {
            home,
            home_identity,
            subscription,
            candidates: BTreeMap::new(),
            errors: BTreeMap::new(),
            servers: Vec::new(),
            rescan: true,
            pending: Changes::default(),
            cancel,
            #[cfg(test)]
            scan_started: None,
            cost: DiscoveryCost::default(),
        })
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn invalidate(&mut self) {
        self.rescan = true;
    }

    fn changes(&self) -> Changes {
        self.subscription
            .as_ref()
            .map(Subscription::changes)
            .unwrap_or_default()
    }

    fn check_cancelled(&self) -> Result<()> {
        anyhow::ensure!(
            !self.cancel.is_cancelled(),
            "active Run observation cancelled"
        );
        Ok(())
    }

    fn read<T: DeserializeOwned>(&mut self, path: &Path) -> Result<Option<T>> {
        self.check_cancelled()?;
        self.cost.receipts += 1;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        self.cost.bytes += bytes.len();
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    fn record_error(&mut self, path: &Path, error: anyhow::Error) -> Result<()> {
        self.errors.insert(path.to_owned(), error.to_string());
        self.check_unknown_limit()
    }

    fn check_unknown_limit(&self) -> Result<()> {
        let unresolved = self.candidates.iter().filter(|(_, candidate)| {
            matches!(
                candidate,
                Receipt::Capture {
                    binding: RunBinding { owner: None, .. },
                    ..
                }
            )
        });
        let (count, bytes) = unresolved.fold(
            (
                self.errors.len(),
                self.errors
                    .iter()
                    .map(|(path, error)| path.as_os_str().len() + error.len())
                    .sum::<usize>(),
            ),
            |(count, bytes), (path, _)| (count + 1, bytes + path.as_os_str().len()),
        );
        anyhow::ensure!(
            count <= 4096 && bytes <= 4 * 1024 * 1024,
            "too much unresolved Run ownership; repair receipts before retrying"
        );
        Ok(())
    }

    fn read_receipt(&mut self, path: &Path, processes: &HashMap<u32, &OsProcess>) -> Result<()> {
        let relative = path.strip_prefix(&self.home)?;
        let parts: Vec<_> = relative.iter().collect();
        if path
            .file_name()
            .is_some_and(|name| name.as_encoded_bytes().starts_with(b"."))
            || path.extension().is_none_or(|ext| ext != "json")
        {
            return Ok(());
        }
        let read = (|| -> Result<Option<Receipt>> {
            if parts.first().is_some_and(|p| *p == "run-bindings") && parts.len() == 2 {
                let value: Option<RunBinding> = self.read(path)?;
                if let Some(value) = &value {
                    crate::durable::RunId::parse(value.run_id.as_str())?;
                }
                let Some(binding) = value else {
                    return Ok(None);
                };
                let native_history = if binding.owner.is_none() {
                    let cached = match self.candidates.get(path) {
                        Some(Receipt::Capture {
                            binding: previous,
                            native_history,
                        }) if previous == &binding && self.subscription.is_some() => {
                            *native_history
                        }
                        _ => None,
                    };
                    Some(match cached {
                        Some(present) => present,
                        None => self.has_native_receipt(&binding.run_id)?,
                    })
                } else {
                    None
                };
                return Ok(Some(Receipt::Capture {
                    binding,
                    native_history,
                }));
            }
            if relative.parent() == Some(Path::new(EXEC_PROCESS_ROOT)) {
                let value: Option<ExecProcessReceipt> = self.read(path)?;
                if let Some(value) = &value {
                    anyhow::ensure!(
                        value.schema_version == 1 && value.pid > 1,
                        "invalid Exec receipt"
                    );
                }
                return Ok(value.map(Receipt::Exec));
            }
            if parts.len() == 5 && parts[0] == "runs" && parts[3] == "provider-clients" {
                let value: Option<ProviderClientRef> = self.read(path)?;
                if let Some(value) = &value {
                    anyhow::ensure!(
                        value.schema_version == 1 && value.pid > 1,
                        "invalid native client receipt"
                    );
                }
                return Ok(value.map(Receipt::Native));
            }
            Ok(None)
        })();
        match read {
            Ok(value) => {
                self.errors.remove(path);
                match value.filter(|value| value.live(processes)) {
                    Some(value) => {
                        self.candidates.insert(path.to_owned(), value);
                    }
                    None => {
                        self.candidates.remove(path);
                    }
                }
            }
            Err(error) => {
                self.candidates.remove(path);
                self.record_error(path, error)?;
            }
        }
        if matches!(
            self.candidates.get(path),
            Some(Receipt::Capture {
                binding: RunBinding { owner: None, .. },
                ..
            })
        ) {
            self.check_unknown_limit()?;
        }
        Ok(())
    }

    fn receipt_path(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.home) else {
            return false;
        };
        let parts: Vec<_> = relative.iter().collect();
        path.extension().is_some_and(|ext| ext == "json")
            && ((parts.len() == 2 && parts[0] == "run-bindings")
                || relative.parent() == Some(Path::new(EXEC_PROCESS_ROOT))
                || (parts.len() == 5 && parts[0] == "runs" && parts[3] == "provider-clients"))
    }

    fn has_native_receipt(&mut self, run: &crate::durable::RunId) -> Result<bool> {
        let dir = crate::run_record::record_dir(&self.home, run).context("invalid Run ID")?;
        let entries = match fs::read_dir(dir.join("provider-clients")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        self.cost.directories += 1;
        for entry in entries {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(client) = self.read::<ProviderClientRef>(&path)? {
                    anyhow::ensure!(
                        client.schema_version == 1 && client.pid > 1,
                        "invalid native receipt"
                    );
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn scan(&mut self, path: &Path, processes: &HashMap<u32, &OsProcess>) -> Result<()> {
        self.check_cancelled()?;
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.candidates.retain(|p, _| !p.starts_with(path));
                self.errors.retain(|p, _| !p.starts_with(path));
                return Ok(());
            }
            Err(error) => return self.record_error(path, error.into()),
        };
        self.cost.directories += 1;
        #[cfg(test)]
        if let Some(started) = self.scan_started.take() {
            let _ = started.send(());
        }
        self.errors.remove(path);
        for entry in entries {
            self.check_cancelled()?;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.record_error(path, error.into())?;
                    continue;
                }
            };
            if entry.file_name().as_encoded_bytes().starts_with(b".") {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(&self.home)?;
            let parts: Vec<_> = relative.iter().collect();
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    self.record_error(&path, error.into())?;
                    continue;
                }
            };
            if kind.is_dir()
                && parts[0] == "runs"
                && (parts.len() <= 3 || (parts.len() == 4 && parts[3] == "provider-clients"))
            {
                self.scan(&path, processes)?;
            } else if kind.is_file() {
                self.read_receipt(&path, processes)?;
            }
        }
        Ok(())
    }

    fn read_servers(&mut self, processes: &HashMap<u32, &OsProcess>) -> Result<()> {
        let path = self.home.join("runtime/opencode-servers.json");
        self.cost.receipts += 1;
        self.cost.bytes += fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
        match registered_opencode_servers_at(&self.home) {
            Ok(servers) => {
                self.errors.remove(&path);
                self.servers = servers
                    .into_iter()
                    .filter(|s| processes.contains_key(&s.opencode_pid))
                    .collect();
            }
            Err(error) => self.record_error(&path, error)?,
        }
        Ok(())
    }

    fn reconcile(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        processes: &HashMap<u32, &OsProcess>,
    ) -> Result<()> {
        let paths: Vec<_> = paths.into_iter().collect();
        let native_changes = paths
            .iter()
            .filter_map(|path| {
                let relative = path.strip_prefix(&self.home).ok()?;
                let parts: Vec<_> = relative.iter().collect();
                if parts.len() >= 3
                    && parts[0] == "runs"
                    && (parts.len() == 3 || parts[3] == "provider-clients")
                {
                    crate::durable::RunId::parse(parts[2].to_str()?)
                        .ok()
                        .map(|id| id.to_string())
                } else {
                    None
                }
            })
            .collect::<std::collections::BTreeSet<_>>();
        for candidate in self.candidates.values_mut() {
            if let Receipt::Capture {
                binding,
                native_history,
            } = candidate
            {
                if binding.owner.is_none() && native_changes.contains(binding.run_id.as_str()) {
                    *native_history = None;
                }
            }
        }
        for path in paths {
            self.check_cancelled()?;
            let relative = match path.strip_prefix(&self.home) {
                Ok(relative) => relative,
                Err(_) => {
                    self.rescan = true;
                    continue;
                }
            };
            let parts: Vec<_> = relative.iter().collect();
            if parts.len() <= 1
                || relative == Path::new("runtime/exec-processes")
                || (parts[0] == "runs" && parts.len() == 2)
            {
                self.rescan = true;
            } else if relative == Path::new("runtime/opencode-servers.json") {
                self.read_servers(processes)?;
            } else if path.extension().is_some_and(|ext| ext == "json") {
                // Manifest changes are read when projecting retained live candidates.
                if path.file_name().is_none_or(|name| name != "manifest.json") {
                    self.read_receipt(&path, processes)?;
                }
            } else if path
                .file_name()
                .is_none_or(|name| !name.as_encoded_bytes().starts_with(b"."))
            {
                self.scan(&path, processes)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn observe(
        &mut self,
        store: &crate::store::SharedStore,
        task: Option<crate::durable::WorkRef>,
    ) -> ActiveRunsSnapshot {
        self.cost = DiscoveryCost::default();
        let mut result = ActiveRunsSnapshot {
            home: self.home.clone(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Ready,
            runs: Vec::new(),
            gaps: Vec::new(),
        };
        let observation = self.collect();
        match observation {
            Ok((bindings, processes, clients, gaps)) => {
                result.gaps.extend(gaps);
                result.gaps.extend(
                    self.errors
                        .iter()
                        .map(|(p, e)| format!("{}: {e}", p.display())),
                );
                super::project(
                    &self.home,
                    store,
                    &bindings,
                    &processes,
                    &clients,
                    &mut result,
                    &mut self.cost,
                )
                .await;
                // Revalidate known ownership after the join as well. This protects
                // one-shot reads, which have no notification subscription.
                let before = self.candidates.clone();
                let by_pid: HashMap<_, _> =
                    processes.processes.iter().map(|p| (p.pid(), p)).collect();
                let mut changed = false;
                for path in before.keys() {
                    if let Err(error) = self.read_receipt(path, &by_pid) {
                        result.gaps.push(error.to_string());
                        changed = true;
                    }
                }
                changed |= before != self.candidates;
                let after = self.changes();
                if changed || after.rescan || !after.paths.is_empty() {
                    self.rescan |= after.rescan;
                    // These paths must survive until the next observation.
                    for path in after.paths {
                        self.pending.insert(path);
                    }
                    result.runs.clear();
                    result.discovery = DiscoveryState::Scanning;
                    result
                        .gaps
                        .push("Run ownership changed during observation; reading again".into());
                } else if self.rescan {
                    result.discovery = DiscoveryState::Scanning;
                    result
                        .gaps
                        .push("Run discovery is catching up with filesystem changes".into());
                    result.runs.clear();
                }
            }
            Err(error) => {
                result.discovery = DiscoveryState::Unavailable;
                result.gaps.push(error.to_string());
                self.rescan = true;
            }
        }
        self.cost.retained = self.candidates.len();
        result.observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        result
    }

    fn collect(&mut self) -> Result<Observation> {
        self.check_cancelled()?;
        let identity = match fs::metadata(&self.home) {
            Ok(m) => Some((m.dev(), m.ino())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(
            self.home_identity.is_none() || self.home_identity == identity,
            "Home directory was replaced or removed; restart the reader to resolve its authority"
        );
        self.home_identity = identity;
        let changes = self.changes();
        self.rescan |= changes.rescan || self.pending.rescan;
        for path in changes.paths {
            self.pending.insert(path);
        }
        self.rescan |= self.pending.rescan;
        self.cost.dirty_paths = self.pending.paths.len();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.cost.process_samples += 1;
        let mut processes = sample_processes(now)?;
        let by_pid: HashMap<_, _> = processes.iter().map(|p| (p.pid(), p)).collect();
        let cold = self.rescan;
        if self.rescan {
            self.cost.rescans += 1;
            self.candidates.clear();
            self.errors.clear();
            self.pending = Changes::default();
            for root in ["runs", "run-bindings", EXEC_PROCESS_ROOT] {
                self.scan(&self.home.join(root), &by_pid)?;
            }
            self.read_servers(&by_pid)?;
            self.rescan = false;
        }
        let pending = std::mem::take(&mut self.pending);
        self.reconcile(pending.paths, &by_pid)?;
        // A long cold scan must not publish its old process observation as current.
        // Publications after the first sample remain queued in FSEvents and force
        // another observation, including clients pruned by that initial sample.
        if cold {
            self.cost.process_samples += 1;
            processes = sample_processes(time::OffsetDateTime::now_utc().unix_timestamp())?;
        }
        let by_pid: HashMap<_, _> = processes.iter().map(|p| (p.pid(), p)).collect();
        // Recheck live/unknown receipts themselves, never all retained history.
        let paths: std::collections::BTreeSet<_> = self
            .candidates
            .keys()
            .chain(self.errors.keys().filter(|path| self.receipt_path(path)))
            .cloned()
            .collect();
        for path in paths {
            self.read_receipt(&path, &by_pid)?;
        }
        let mut bindings = BTreeMap::new();
        let mut receipts = Vec::new();
        let mut clients = Vec::new();
        let mut gaps = Vec::new();
        for (path, candidate) in &self.candidates {
            match candidate {
                Receipt::Capture {
                    binding,
                    native_history,
                } => {
                    if *native_history != Some(true) {
                        bindings.insert(path.clone(), binding.clone());
                    }
                }
                Receipt::Exec(receipt) => receipts.push(receipt.clone()),
                Receipt::Native(client) => {
                    let dir = path
                        .parent()
                        .and_then(Path::parent)
                        .context("native receipt has no Run directory")?;
                    match self.cost.manifest(dir) {
                        Ok(manifest) => {
                            clients.push((manifest.run_id, client.clone(), manifest.harness));
                        }
                        Err(error) => {
                            // Manifest evidence belongs to this observation's live
                            // clients, not to the retained receipt-discovery cache.
                            gaps.push(format!("{}: {error}", dir.join("manifest.json").display()));
                        }
                    }
                }
            }
        }
        Ok((
            bindings,
            ProcessSnapshot {
                processes,
                receipts,
                opencode_servers: self.servers.clone(),
            },
            clients,
            gaps,
        ))
    }
}
