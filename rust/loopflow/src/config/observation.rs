//! File dependencies of a bounded settings read. Watches probe only these
//! inputs, including absent files, rather than rebuilding Work on every tick.
//! Content fingerprints survive editor replacement and timestamp preservation.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

thread_local! {
    // Readings run on the pulling thread; bounded workers inherit this scope.
    static READING: RefCell<Option<Files>> = const { RefCell::new(None) };
}

#[derive(Debug, PartialEq, Eq)]
enum Fingerprint {
    Content(Result<[u8; 32], io::ErrorKind>),
    Directory(Result<SystemTime, io::ErrorKind>),
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Files(Arc<Mutex<BTreeMap<PathBuf, Fingerprint>>>);

impl Files {
    /// Capture on the calling thread, including synchronous config loads while
    /// it drives an async reading with `Runtime::block_on`.
    pub(crate) fn capture<T>(read: impl FnOnce() -> T) -> (T, Self) {
        let files = Self::default();
        let result = files.observe(read);
        (result, files)
    }

    fn observe<T>(&self, read: impl FnOnce() -> T) -> T {
        struct Restore(Option<Files>);
        impl Drop for Restore {
            fn drop(&mut self) {
                READING.with(|reading| *reading.borrow_mut() = self.0.take());
            }
        }
        let previous = Restore(READING.replace(Some(self.clone())));
        let result = read();
        drop(previous);
        result
    }

    pub(crate) fn changed(&self) -> bool {
        self.0
            .lock()
            .expect("configuration observations poisoned")
            .iter()
            .any(|(path, before)| {
                let after = match before {
                    Fingerprint::Directory(_) => directory_fingerprint(path),
                    Fingerprint::Content(_) => content_fingerprint(&std::fs::read_to_string(path)),
                };
                before != &after
            })
    }
}

/// Carry this reading's dependencies into a bounded worker without observing
/// unrelated watches or retaining a process-wide list of files.
pub(crate) fn inherit<T>(read: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let files = READING.with(|reading| reading.borrow().clone());
    move || match files {
        Some(files) => files.observe(read),
        None => read(),
    }
}

fn record(path: &Path, fingerprint: impl FnOnce() -> Fingerprint) {
    READING.with(|reading| {
        if let Some(files) = reading.borrow_mut().as_mut() {
            let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
            // Keep the first observation so an edit during a read is not lost.
            files
                .0
                .lock()
                .expect("configuration observations poisoned")
                .entry(path)
                .or_insert_with(fingerprint);
        }
    });
}

fn content_fingerprint(content: &io::Result<String>) -> Fingerprint {
    Fingerprint::Content(
        content
            .as_ref()
            .map(|content| Sha256::digest(content.as_bytes()).into())
            .map_err(|error| error.kind()),
    )
}

pub(crate) fn read_to_string(path: &Path) -> io::Result<String> {
    let content = std::fs::read_to_string(path);
    record(path, || content_fingerprint(&content));
    content
}

fn directory_fingerprint(path: &Path) -> Fingerprint {
    Fingerprint::Directory(
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .map_err(|error| error.kind()),
    )
}

/// Definition discovery also depends on directory entries: adding an override
/// must invalidate a read that previously selected a builtin or another scope.
pub(crate) fn directory(path: &Path) {
    record(path, || directory_fingerprint(path));
}
