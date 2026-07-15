mod support;

use std::ffi::OsString;
use std::path::Path;

use loopflow::store::{
    open_store, storage_config_from_env, StorageConfig, CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV,
};

struct AmbientStorage {
    previous_lf_home: Option<OsString>,
    previous_db_path: Option<OsString>,
    previous_control_home: Option<OsString>,
    previous_control_db_path: Option<OsString>,
}

impl AmbientStorage {
    fn seed(home: &Path, db_path: &Path) -> Self {
        let previous_lf_home = std::env::var_os("LF_HOME");
        let previous_db_path = std::env::var_os("LF_DB_PATH");
        let previous_control_home = std::env::var_os(CONTROL_HOME_ENV);
        let previous_control_db_path = std::env::var_os(CONTROL_DB_PATH_ENV);
        std::env::set_var("LF_HOME", home);
        std::env::set_var("LF_DB_PATH", db_path);
        std::env::set_var(CONTROL_HOME_ENV, home);
        std::env::set_var(CONTROL_DB_PATH_ENV, db_path);
        Self {
            previous_lf_home,
            previous_db_path,
            previous_control_home,
            previous_control_db_path,
        }
    }
}

impl Drop for AmbientStorage {
    fn drop(&mut self) {
        match &self.previous_lf_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
        match &self.previous_db_path {
            Some(value) => std::env::set_var("LF_DB_PATH", value),
            None => std::env::remove_var("LF_DB_PATH"),
        }
        match &self.previous_control_home {
            Some(value) => std::env::set_var(CONTROL_HOME_ENV, value),
            None => std::env::remove_var(CONTROL_HOME_ENV),
        }
        match &self.previous_control_db_path {
            Some(value) => std::env::set_var(CONTROL_DB_PATH_ENV, value),
            None => std::env::remove_var(CONTROL_DB_PATH_ENV),
        }
    }
}

fn open_test_store() {
    assert!(std::env::var_os("LF_HOME").is_some());
    assert!(std::env::var_os("LF_DB_PATH").is_none());
    assert!(std::env::var_os(CONTROL_HOME_ENV).is_none());
    assert!(std::env::var_os(CONTROL_DB_PATH_ENV).is_none());
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
    let ambient_db_dir = tempfile::tempdir().expect("ambient db dir");
    let ambient_db = ambient_db_dir.path().join("production.db");
    let _ambient = AmbientStorage::seed(ambient_home.path(), &ambient_db);

    support::with_clean_home(open_test_store);
    assert_eq!(
        std::env::var_os("LF_HOME").as_deref(),
        Some(ambient_home.path().as_os_str())
    );
    assert_eq!(
        std::env::var_os("LF_DB_PATH").as_deref(),
        Some(ambient_db.as_os_str())
    );
    assert_eq!(
        std::env::var_os(CONTROL_HOME_ENV).as_deref(),
        Some(ambient_home.path().as_os_str())
    );
    assert_eq!(
        std::env::var_os(CONTROL_DB_PATH_ENV).as_deref(),
        Some(ambient_db.as_os_str())
    );

    {
        let _guard = support::EnvGuard::new(&[]);
        open_test_store();
    }
    assert_eq!(
        std::env::var_os("LF_HOME").as_deref(),
        Some(ambient_home.path().as_os_str())
    );
    assert_eq!(
        std::env::var_os("LF_DB_PATH").as_deref(),
        Some(ambient_db.as_os_str())
    );
    assert_eq!(
        std::env::var_os(CONTROL_HOME_ENV).as_deref(),
        Some(ambient_home.path().as_os_str())
    );
    assert_eq!(
        std::env::var_os(CONTROL_DB_PATH_ENV).as_deref(),
        Some(ambient_db.as_os_str())
    );

    assert!(!ambient_home.path().join("loopflow.db").exists());
    assert!(!ambient_db.exists());
}
