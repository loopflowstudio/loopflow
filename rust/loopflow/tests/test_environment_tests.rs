mod support;

use std::ffi::OsString;
use std::path::Path;

use loopflow::store::{open_store, storage_config_from_env, StorageConfig};

struct AmbientStorage {
    previous_lf_home: Option<OsString>,
}

impl AmbientStorage {
    fn seed(home: &Path) -> Self {
        let previous_lf_home = std::env::var_os("LF_HOME");
        std::env::set_var("LF_HOME", home);
        Self { previous_lf_home }
    }
}

impl Drop for AmbientStorage {
    fn drop(&mut self) {
        match &self.previous_lf_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
    }
}

fn open_test_store() {
    let config = storage_config_from_env().expect("test storage config");
    let StorageConfig::Sqlite { path } = &config;
    let home = std::env::var_os("LF_HOME").expect("isolated LF_HOME");
    assert!(path.starts_with(home));
    tokio::runtime::Runtime::new()
        .expect("test runtime")
        .block_on(open_store(&config))
        .expect("open isolated test store");
}

#[test]
fn test_guards_keep_store_writes_out_of_ambient_paths() {
    let ambient_home = tempfile::tempdir().expect("ambient home");
    let _ambient = AmbientStorage::seed(ambient_home.path());

    support::with_clean_home(open_test_store);
    assert_eq!(
        std::env::var_os("LF_HOME").as_deref(),
        Some(ambient_home.path().as_os_str())
    );

    {
        let _guard = support::EnvGuard::new(&[]);
        open_test_store();
    }
    assert_eq!(
        std::env::var_os("LF_HOME").as_deref(),
        Some(ambient_home.path().as_os_str())
    );

    assert!(!ambient_home.path().join("loopflow.db").exists());
}
