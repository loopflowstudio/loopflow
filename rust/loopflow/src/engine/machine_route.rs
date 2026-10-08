//! SSH destinations are OpenSSH inputs, including config aliases and URIs.
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::Path;

use sha2::{Digest, Sha256};

pub(crate) const SSH_CONNECT_TIMEOUT_SECS: u32 = 10;

pub(crate) fn bounded_ssh_args(dest: &str, forward_agent: bool) -> std::io::Result<Vec<String>> {
    // Keep sun_path short even for custom data directories. The per-store directory
    // belongs only to this OS user; OpenSSH supplies the host/port/user socket key.
    let home = crate::store::lf_home_dir();
    let mut namespace = Sha256::new();
    namespace.update(home.as_os_str().as_encoded_bytes());
    if forward_agent {
        namespace.update(b"agent");
        namespace.update(
            std::env::var_os("SSH_AUTH_SOCK")
                .unwrap_or_default()
                .as_encoded_bytes(),
        );
    }
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    let directory = std::path::PathBuf::from(format!(
        "/tmp/lf-ssh-{uid}-{}",
        &hex::encode(namespace.finalize())[..16]
    ));
    match DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(std::io::Error::other(format!(
            "SSH control directory {} must be owned by this user with mode 0700",
            directory.display()
        )));
    }
    Ok(vec![
        if forward_agent { "-A" } else { "-a" }.into(),
        "-x".into(),
        "-T".into(),
        "-o".into(),
        "ControlMaster=auto".into(),
        "-o".into(),
        "ControlPersist=60".into(),
        "-o".into(),
        format!("ControlPath={}/%C", directory.display()),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "StrictHostKeyChecking=yes".into(),
        "-o".into(),
        format!("ConnectTimeout={SSH_CONNECT_TIMEOUT_SECS}"),
        "-o".into(),
        "ServerAliveInterval=10".into(),
        "-o".into(),
        "ServerAliveCountMax=3".into(),
        "--".into(),
        dest.into(),
    ])
}

pub(crate) fn resolve_home_relative_repo(repo: &Path) -> Result<String, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    repo.strip_prefix(&home)
        .map_err(|_| {
            format!(
                "repo {} is outside {}; remote Machine routing needs a home-relative path",
                repo.display(),
                home.display()
            )
        })?
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("repo path {} is not UTF-8", repo.display()))
}
