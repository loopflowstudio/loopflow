use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use crate::harness::opencode_runtime::{registered_opencode_servers_at, OpenCodeServerEntry};
use crate::journal::{ProcessReceipt, PROCESS_RECEIPT_ROOT};
use crate::lf::commands::top::{sample_processes, OsProcess, ProcessSnapshot};
use crate::session_record::ProviderClientRef;

use super::events::{Changes, Subscription};
use super::{ActiveSessionsSnapshot, DiscoveryState};

type Observation = (
    ProcessSnapshot,
    Vec<(String, ProviderClientRef)>,
    Vec<String>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Receipt {
    Native(ProviderClientRef),
    Process(ProcessReceipt),
}

#[cfg(all(test, target_os = "macos"))]
mod tests;

impl Receipt {
    fn live(&self, processes: &HashMap<u32, &OsProcess>) -> bool {
        let (pid, start, tolerance) = match self {
            Self::Native(client) => (client.pid, client.started_at.unix_timestamp(), 5),
            Self::Process(process) => (process.pid, process.started_at, 3),
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
    pub bytes: usize,
    pub process_samples: usize,
    pub rescans: usize,
    pub retained: usize,
    pub dirty_paths: usize,
}

#[derive(Debug)]
pub(crate) struct ActiveSessionReader {
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

impl ActiveSessionReader {
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
            "active Session observation cancelled"
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
        let count = self.errors.len();
        let bytes = self
            .errors
            .iter()
            .map(|(path, error)| path.as_os_str().len() + error.len())
            .sum::<usize>();
        anyhow::ensure!(
            count <= 4096 && bytes <= 4 * 1024 * 1024,
            "too much unresolved Session ownership; repair receipts before retrying"
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
            if relative.parent() == Some(Path::new(PROCESS_RECEIPT_ROOT)) {
                let value: Option<ProcessReceipt> = self.read(path)?;
                if let Some(value) = &value {
                    anyhow::ensure!(
                        value.schema_version == 1 && value.pid > 1,
                        "invalid Process receipt"
                    );
                }
                return Ok(value.map(Receipt::Process));
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
        Ok(())
    }

    fn receipt_path(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.home) else {
            return false;
        };
        let parts: Vec<_> = relative.iter().collect();
        path.extension().is_some_and(|ext| ext == "json")
            && (relative.parent() == Some(Path::new(PROCESS_RECEIPT_ROOT))
                || (parts.len() == 5 && parts[0] == "runs" && parts[3] == "provider-clients"))
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
                || relative == Path::new(PROCESS_RECEIPT_ROOT)
                || (parts[0] == "runs" && parts.len() == 2)
            {
                self.rescan = true;
            } else if relative == Path::new("runtime/opencode-servers.json") {
                self.read_servers(processes)?;
            } else if path.extension().is_some_and(|ext| ext == "json") {
                // Input metadata comes from SQL; only process receipts affect discovery.
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
    ) -> ActiveSessionsSnapshot {
        self.cost = DiscoveryCost::default();
        let mut result = ActiveSessionsSnapshot {
            home: self.home.clone(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Ready,
            sessions: Vec::new(),
            gaps: Vec::new(),
        };
        let observation = self.collect();
        match observation {
            Ok((processes, clients, gaps)) => {
                result.gaps.extend(gaps);
                result.gaps.extend(
                    self.errors
                        .iter()
                        .map(|(p, e)| format!("{}: {e}", p.display())),
                );
                let inputs = clients
                    .iter()
                    .map(|(input, _)| input.clone())
                    .collect::<Vec<_>>();
                let process_lfids = processes
                    .receipts
                    .iter()
                    .map(|receipt| receipt.process_lfid.clone())
                    .collect::<Vec<_>>();
                let pids = processes
                    .processes
                    .iter()
                    .map(|process| process.pid())
                    .collect::<Vec<_>>();
                let ownership =
                    match store
                        .sqlite
                        .session_process_ownership(&inputs, &process_lfids, &pids)
                    {
                        Ok(ownership) => ownership,
                        Err(error) => {
                            result.discovery = DiscoveryState::Unavailable;
                            result.gaps.push(error.to_string());
                            return result;
                        }
                    };
                super::project(&ownership, &processes, &clients, &mut result);
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
                // SQL is sampled on every tick, including when its database is
                // outside Home. Filesystem notifications are not its invalidation.
                match store
                    .sqlite
                    .session_process_ownership(&inputs, &process_lfids, &pids)
                {
                    Ok(after) => changed |= ownership != after,
                    Err(error) => {
                        result.discovery = DiscoveryState::Unavailable;
                        result.sessions.clear();
                        result.gaps.push(error.to_string());
                        return result;
                    }
                }
                let after = self.changes();
                if changed || after.rescan || !after.paths.is_empty() {
                    self.rescan |= after.rescan;
                    // These paths must survive until the next observation.
                    for path in after.paths {
                        self.pending.insert(path);
                    }
                    result.sessions.clear();
                    result.discovery = DiscoveryState::Scanning;
                    result
                        .gaps
                        .push("Session ownership changed during observation; reading again".into());
                } else if self.rescan {
                    result.discovery = DiscoveryState::Scanning;
                    result
                        .gaps
                        .push("Session discovery is catching up with filesystem changes".into());
                    result.sessions.clear();
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
            for root in ["runs", PROCESS_RECEIPT_ROOT] {
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
        let mut receipts = Vec::new();
        let mut clients = Vec::new();
        let mut gaps = Vec::new();
        for (path, candidate) in &self.candidates {
            match candidate {
                Receipt::Process(receipt) => receipts.push(receipt.clone()),
                Receipt::Native(client) => {
                    let dir = path
                        .parent()
                        .and_then(Path::parent)
                        .context("native receipt has no input directory")?;
                    match crate::session_record::input_id_from_dir(dir) {
                        Ok(input) => clients.push((input, client.clone())),
                        Err(error) => gaps.push(format!("{}: {error}", dir.display())),
                    }
                }
            }
        }
        Ok((
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
