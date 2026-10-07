//! Transient filesystem invalidation. Notifications never establish ownership.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_PATHS: usize = 4096;
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct Changes {
    pub paths: BTreeSet<PathBuf>,
    pub rescan: bool,
    bytes: usize,
}

impl Changes {
    pub(super) fn insert(&mut self, path: PathBuf) {
        if self.rescan || self.paths.contains(&path) {
            return;
        }
        self.bytes += path.as_os_str().len();
        if self.paths.len() >= MAX_PATHS || self.bytes > MAX_BYTES {
            self.invalidate();
        } else {
            self.paths.insert(path);
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.paths.clear();
        self.bytes = 0;
        self.rescan = true;
    }
}

#[cfg(test)]
mod tests {
    use super::Changes;
    use std::path::PathBuf;

    #[test]
    fn overflow_invalidates_coverage_instead_of_evicting_paths() {
        let mut changes = Changes::default();
        for index in 0..5000 {
            changes.insert(PathBuf::from(format!("runs/{index}")));
        }
        assert!(changes.rescan);
        assert!(changes.paths.is_empty());
        assert_eq!(changes.bytes, 0);
    }
}

// Keep directory events: an older input can acquire its first native client.
#[cfg(target_os = "macos")]
fn relevant(home: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(home) else {
        return home.starts_with(path);
    };
    let parts: Vec<_> = relative.iter().collect();
    let published = || {
        path.extension().is_some_and(|ext| ext == "json")
            && path
                .file_name()
                .is_some_and(|name| !name.as_encoded_bytes().starts_with(b"."))
    };
    match parts.first().and_then(|value| value.to_str()) {
        None => true,
        Some("runs") => {
            parts.len() <= 3
                || (parts.len() == 4 && parts[3] == "provider-clients")
                || (parts.len() == 5 && parts[3] == "provider-clients" && published())
        }
        Some("runtime") => {
            parts.len() == 1
                || (parts.len() == 2
                    && (parts[1] == "exec-processes" || parts[1] == "opencode-servers.json"))
                || (parts.len() == 3 && parts[1] == "exec-processes" && published())
        }
        _ => false,
    }
}

#[cfg(target_os = "macos")]
pub(super) use macos::Subscription;

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub(super) struct Subscription;

#[cfg(not(target_os = "macos"))]
impl Subscription {
    pub fn start(_: &Path) -> anyhow::Result<Self> {
        anyhow::bail!("continuous active Session discovery requires macOS FSEvents")
    }

    pub fn changes(&self) -> Changes {
        unreachable!("unsupported platforms cannot start a subscription")
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::{relevant, Changes};
    use crate::engine::fs_events::Stream;

    #[derive(Debug)]
    pub(crate) struct Subscription {
        stream: Stream,
        changes: Arc<Mutex<Changes>>,
    }

    impl Subscription {
        pub fn start(home: &Path) -> Result<Self> {
            let root = home
                .ancestors()
                .find(|path| path.is_dir())
                .ok_or_else(|| anyhow::anyhow!("no existing Machine ancestor to observe"))?;
            let changes = Arc::new(Mutex::new(Changes::default()));
            let (sink, home) = (changes.clone(), home.to_owned());
            let stream = Stream::start(&[root], 0.05, true, move |path| {
                let mut changes = sink.lock().unwrap_or_else(|error| error.into_inner());
                match path {
                    // Interpret lost coverage before filtering.
                    None => changes.invalidate(),
                    Some(path) if relevant(&home, path) => changes.insert(path.to_owned()),
                    Some(_) => {}
                }
            })?;
            Ok(Self { stream, changes })
        }

        pub fn changes(&self) -> Changes {
            self.stream.flush();
            std::mem::take(
                &mut *self
                    .changes
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()),
            )
        }
    }
}
