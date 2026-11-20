//! Shared conversation fixtures for runtime and adapter checks.
use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
use std::ffi::OsString;
#[derive(Debug)]
pub struct SessionHome {
    pub home: tempfile::TempDir,
    previous: Vec<(&'static str, Option<OsString>)>,
    _ambient: crate::test_ambient::EnvGuard,
}

impl Default for SessionHome {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionHome {
    pub fn new() -> Self {
        let ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let previous = ["LF_HOME", "LF_BIN"]
            .into_iter()
            .map(|name| {
                let value = std::env::var_os(name);
                std::env::remove_var(name);
                (name, value)
            })
            .collect();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            home,
            previous,
            _ambient: ambient,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.home.path()
    }

    pub async fn store(&self) -> SharedStore {
        std::sync::Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(self.home.path().join("loopflow.db")))
                .await
                .unwrap(),
        )
    }
}

impl Drop for SessionHome {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
