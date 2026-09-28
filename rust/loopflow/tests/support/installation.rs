//! Real CLI installation authority, only inside the disposable proof account.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use loopflow::machine_install::{
    self, write_active, ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet,
    InstallSelection, InstallSource,
};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct Installation {
    pub cli: PathBuf,
    root: PathBuf,
    _artifacts: tempfile::TempDir,
}

impl Installation {
    pub fn new(home: &Path) -> Self {
        // HOME overrides cannot isolate machine installation authority.
        assert!(Path::new("/.dockerenv").is_file());
        assert_eq!(
            machine_install::account_home().unwrap(),
            Path::new("/home/lf-task-proof")
        );
        let root = machine_install::root().unwrap();
        assert!(!root.exists(), "each proof owns an empty installation");
        let artifacts = tempfile::tempdir().unwrap();
        let cli = artifacts.path().join("lf");
        let daemon = artifacts.path().join("lfd");
        fs::copy(env!("CARGO_BIN_EXE_lf"), &cli).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_lfd"), &daemon).unwrap();
        // Installation recognizes byte-identical copies, regardless of path.
        // An ELF trailer distinguishes this fixture's installed identity while
        // executing the real CLI code. This does not simulate an older schema.
        fs::OpenOptions::new()
            .append(true)
            .open(&cli)
            .unwrap()
            .write_all(b"\nloopflow disposable installed fixture\n")
            .unwrap();
        let artifacts_identity = vec![
            ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap(),
            ArtifactIdentity::capture(ArtifactRole::Daemon, &daemon).unwrap(),
        ];
        let content_sha256 = hex::encode(Sha256::digest(
            artifacts_identity
                .iter()
                .map(|artifact| artifact.sha256.as_str())
                .collect::<String>(),
        ));
        let set = ArtifactSet {
            id: "task-proof".into(),
            source: InstallSource::Development,
            source_revision: loopflow::build_info::source_revision().into(),
            source_identity: loopflow::build_info::source_identity(),
            content_sha256,
            artifacts: artifacts_identity,
        };
        set.verify(&[ArtifactRole::Cli, ArtifactRole::Daemon])
            .unwrap();
        let mut fallback = set.clone();
        fallback.source = InstallSource::Published;
        write_active(
            &root,
            &ActiveInstall {
                schema_version: 1,
                selection: InstallSelection {
                    installation_id: "task-proof".into(),
                    source: InstallSource::Development,
                    artifact_set: set,
                    store: home.join("loopflow.db"),
                },
                published_fallback: fallback.clone(),
                retained_published_sets: vec![fallback],
            },
        )
        .unwrap();
        Self {
            cli,
            root,
            _artifacts: artifacts,
        }
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove disposable installation receipt");
    }
}
