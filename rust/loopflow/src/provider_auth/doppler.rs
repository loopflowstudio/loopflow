//! Narrow online secret resolution. Never use the SSH credential bundle for reporting.
use std::process::Stdio;
use std::time::Duration;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DopplerReference {
    pub project: String,
    pub config: String,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DopplerError {
    #[error("invalid Doppler secret reference")]
    InvalidReference,
    #[error("Doppler online lookup unavailable or denied")]
    Unavailable,
    #[error("Doppler returned an empty, invalid or oversized secret")]
    InvalidValue,
}

pub async fn resolve(reference: &DopplerReference) -> Result<SecretString, DopplerError> {
    if reference.project.trim().is_empty() || reference.config.trim().is_empty() {
        return Err(DopplerError::InvalidReference);
    }
    resolve_secret(
        &reference.name,
        Some((&reference.project, &reference.config)),
    )
    .await
}

pub(crate) async fn resolve_secret(
    name: &str,
    scope: Option<(&str, &str)>,
) -> Result<SecretString, DopplerError> {
    if !valid_name(name) {
        return Err(DopplerError::InvalidReference);
    }
    let mut command = Command::new("doppler");
    // `secrets get` reads the API; unlike run/download it has no fallback cache.
    command.args([
        "secrets",
        "get",
        name,
        "--plain",
        "--no-check-version",
        "--attempts",
        "1",
        "--timeout",
        "10s",
    ]);
    if let Some((project, config)) = scope {
        command.args(["--project", project, "--config", config]);
    }
    read_secret(&mut command).await
}

async fn read_secret(command: &mut Command) -> Result<SecretString, DopplerError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| DopplerError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(DopplerError::Unavailable)?;
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut bytes = Vec::new();
        stdout
            .take(65537)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| DopplerError::Unavailable)?;
        if bytes.len() > 65536 {
            return Err(DopplerError::InvalidValue);
        }
        if !child
            .wait()
            .await
            .map_err(|_| DopplerError::Unavailable)?
            .success()
        {
            return Err(DopplerError::Unavailable);
        }
        let value = String::from_utf8(bytes).map_err(|_| DopplerError::InvalidValue)?;
        let value = value.trim_end_matches(['\n', '\r']);
        if value.is_empty() {
            return Err(DopplerError::InvalidValue);
        }
        Ok(SecretString::new(value.to_owned()))
    })
    .await
    .map_err(|_| DopplerError::Unavailable)?
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::{read_secret, valid_name};
    use secrecy::ExposeSecret;
    use tokio::process::Command;

    #[test]
    fn names_cannot_inject_arguments() {
        for name in ["", "--plain", "1BAD", "A B", "A=B; rm -rf ~"] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("_KEY1"));
    }

    #[tokio::test]
    async fn captures_values_and_sanitizes_failures() {
        let value = uuid::Uuid::new_v4().to_string();
        let secret =
            read_secret(Command::new("sh").args(["-c", "printf '%s\\n' \"$1\"", "probe", &value]))
                .await
                .unwrap();
        assert!(secret.expose_secret() == &value);
        assert!(!format!("{secret:?}").contains(&value));
        let error = read_secret(Command::new("sh").args([
            "-c",
            "printf '%s' \"$1\"; printf '%s' \"$1\" >&2; exit 1",
            "probe",
            &value,
        ]))
        .await
        .unwrap_err();
        assert!(!error.to_string().contains(&value));
    }
}
