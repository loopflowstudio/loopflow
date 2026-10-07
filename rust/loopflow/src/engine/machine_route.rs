//! SSH destinations are OpenSSH inputs, including config aliases and URIs.
use std::path::Path;

pub(crate) const SSH_CONNECT_TIMEOUT_SECS: u32 = 10;

pub(crate) fn bounded_ssh_args(dest: &str) -> Vec<String> {
    vec![
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
    ]
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
