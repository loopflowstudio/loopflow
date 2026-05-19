use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use qrcode::render::unicode;
use qrcode::QrCode;
use serde::Serialize;

use crate::lfd::token_ledger::TokenLedger;
use crate::ops::error::{OpsError, OpsResult};

pub const PAIRING_TOKEN_TTL: Duration = Duration::from_secs(90 * 24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairOptions {
    pub host: Option<String>,
    pub port: u16,
    pub tls: Option<bool>,
    pub fingerprint: Option<String>,
    pub tls_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairResult {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
}

#[derive(Serialize)]
struct PairQuery<'a> {
    host: &'a str,
    port: u16,
    tls: bool,
    token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fp: Option<&'a str>,
}

pub fn pair_lfd(
    options: &PairOptions,
    progress: &impl crate::ops::Progress,
) -> OpsResult<PairResult> {
    let host = resolve_host(options.host.as_deref())?;
    let use_tls = options.tls.unwrap_or_else(|| !is_tailscale_host(&host));
    if !use_tls && !is_tailscale_host(&host) {
        return Err(OpsError::Message(format!(
            "refusing plaintext pairing URL for non-Tailscale host '{host}'; use TLS or pass a 100.64.0.0/10 host"
        )));
    }

    ensure_studio_auth_config(progress)?;
    let fingerprint =
        resolve_fingerprint(options.fingerprint.as_deref(), options.tls_url.as_deref())?;
    let token = mint_pairing_token()?;
    let url = pairing_url(&host, options.port, use_tls, &token, fingerprint.as_deref())?;

    progress.status("Scan this QR with Loopflow mobile:");
    print_qr(&url, progress);
    progress.status(&url);

    Ok(PairResult {
        url,
        host,
        port: options.port,
        use_tls,
    })
}

pub fn pairing_url(
    host: &str,
    port: u16,
    use_tls: bool,
    token: &str,
    fingerprint: Option<&str>,
) -> OpsResult<String> {
    let normalized_fp = fingerprint.map(normalize_fingerprint).transpose()?;
    let query = serde_urlencoded::to_string(PairQuery {
        host,
        port,
        tls: use_tls,
        token,
        fp: normalized_fp.as_deref(),
    })
    .map_err(|err| OpsError::Message(format!("failed encoding pair URL: {err}")))?;
    Ok(format!("loopflow://pair?{query}"))
}

pub fn resolve_host(explicit: Option<&str>) -> OpsResult<String> {
    if let Some(host) = explicit.map(str::trim).filter(|host| !host.is_empty()) {
        return Ok(host.to_string());
    }

    let output = Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_err(|_| OpsError::Message(
            "pass --host with an address reachable from the phone, or install Tailscale and run `tailscale ip -4`".to_string(),
        ))?;
    if !output.status.success() {
        return Err(OpsError::Message(
            "pass --host with an address reachable from the phone; `tailscale ip -4` failed"
                .to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let candidate = line.trim();
        if is_tailscale_host(candidate) {
            return Ok(candidate.to_string());
        }
    }

    Err(OpsError::Message(
        "pass --host with a reachable address; no Tailscale 100.64.0.0/10 IPv4 address found"
            .to_string(),
    ))
}

fn ensure_studio_auth_config(progress: &impl crate::ops::Progress) -> OpsResult<()> {
    if std::env::var("LFD_AUTH_MODE")
        .ok()
        .is_some_and(|value| value.trim() == "studio")
    {
        return Ok(());
    }

    let path = lfd_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            OpsError::Message(format!("failed creating lfd config directory: {err}"))
        })?;
    }

    let mut value = match std::fs::read_to_string(&path) {
        Ok(content) => {
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content).map_err(|err| {
                OpsError::Message(format!("invalid lfd config at {}: {err}", path.display()))
            })?
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
        }
        Err(err) => {
            return Err(OpsError::Message(format!(
                "failed reading lfd config at {}: {err}",
                path.display()
            )));
        }
    };

    let root = value
        .as_mapping_mut()
        .ok_or_else(|| OpsError::Message("lfd config must be a YAML mapping".to_string()))?;
    let auth_key = serde_yaml_ng::Value::String("auth".to_string());
    let mode_key = serde_yaml_ng::Value::String("mode".to_string());
    let auth = root
        .entry(auth_key)
        .or_insert_with(|| serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()));
    let auth_map = auth.as_mapping_mut().ok_or_else(|| {
        OpsError::Message("lfd config auth section must be a YAML mapping".to_string())
    })?;

    if auth_map
        .get(&mode_key)
        .and_then(serde_yaml_ng::Value::as_str)
        == Some("studio")
    {
        return Ok(());
    }

    auth_map.insert(mode_key, serde_yaml_ng::Value::String("studio".to_string()));
    let rendered = serde_yaml_ng::to_string(&value)
        .map_err(|err| OpsError::Message(format!("failed rendering lfd config: {err}")))?;
    std::fs::write(&path, rendered)
        .map_err(|err| OpsError::Message(format!("failed writing {}: {err}", path.display())))?;
    progress.warning(&format!(
        "set auth.mode=studio in {}; restart lfd if it is already running",
        path.display()
    ));
    Ok(())
}

fn lfd_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("lfd.yaml")
}

fn mint_pairing_token() -> OpsResult<String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed creating runtime: {err}")))?;
    runtime.block_on(async {
        let storage_config = crate::lfd::storage_config_from_env()
            .map_err(|err| OpsError::Message(format!("failed resolving lfd storage: {err}")))?;
        let ledger_path = crate::lfd::connection_token_ledger_path(&storage_config);
        let ledger = TokenLedger::new(ledger_path)
            .await
            .map_err(|err| OpsError::Message(format!("failed opening token ledger: {err}")))?;
        ledger
            .mint_with_ttl(1, PAIRING_TOKEN_TTL)
            .await
            .map_err(|err| OpsError::Message(format!("failed minting pairing token: {err}")))?
            .pop()
            .ok_or_else(|| OpsError::Message("failed minting pairing token".to_string()))
    })
}

fn resolve_fingerprint(
    fingerprint: Option<&str>,
    tls_url: Option<&str>,
) -> OpsResult<Option<String>> {
    match (fingerprint, tls_url) {
        (Some(_), Some(_)) => Err(OpsError::Message(
            "pass either --fingerprint or --tls-url, not both".to_string(),
        )),
        (Some(value), None) => normalize_fingerprint(value).map(Some),
        (None, Some(url)) => fetch_tls_fingerprint(url).map(Some),
        (None, None) => Ok(None),
    }
}

fn normalize_fingerprint(value: &str) -> OpsResult<String> {
    let normalized: String = value
        .chars()
        .filter(|ch| !matches!(ch, ':' | ' ' | '-' | '\n' | '\t'))
        .flat_map(char::to_lowercase)
        .collect();
    if normalized.len() == 64 && normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(normalized)
    } else {
        Err(OpsError::Message(
            "certificate fingerprint must be a SHA-256 hex digest".to_string(),
        ))
    }
}

fn fetch_tls_fingerprint(url: &str) -> OpsResult<String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| OpsError::Message(format!("invalid --tls-url: {err}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| OpsError::Message("--tls-url requires a host".to_string()))?;
    let port = parsed.port_or_known_default().unwrap_or(443);
    let command = format!(
        "echo | openssl s_client -servername {host} -connect {host}:{port} 2>/dev/null | openssl x509 -outform DER | openssl dgst -sha256 -binary | xxd -p -c 256"
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|err| OpsError::Message(format!("failed running openssl for --tls-url: {err}")))?;
    if !output.status.success() {
        return Err(OpsError::Message(
            "failed fetching TLS certificate fingerprint".to_string(),
        ));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    normalize_fingerprint(raw.trim())
}

fn print_qr(url: &str, progress: &impl crate::ops::Progress) {
    match QrCode::new(url.as_bytes()) {
        Ok(code) => progress.status(&code.render::<unicode::Dense1x2>().quiet_zone(false).build()),
        Err(err) => progress.warning(&format!("failed rendering QR: {err}")),
    }
}

fn is_tailscale_host(host: &str) -> bool {
    host.parse::<Ipv4Addr>()
        .map(|ip| {
            let octets = ip.octets();
            octets[0] == 100 && (64..=127).contains(&octets[1])
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{is_tailscale_host, pairing_url};

    #[test]
    fn pairing_url_encodes_expected_scheme_and_query() {
        let url = pairing_url("100.64.1.2", 2486, false, "tok+en", Some("AA:bb")).unwrap_err();
        assert!(url.to_string().contains("fingerprint"));

        let url = pairing_url(
            "100.64.1.2",
            2486,
            false,
            "tok+en",
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        )
        .expect("url");
        assert!(url.starts_with("loopflow://pair?"));
        assert!(url.contains("host=100.64.1.2"));
        assert!(url.contains("tls=false"));
        assert!(url.contains("token=tok%2Ben"));
    }

    #[test]
    fn plaintext_requires_tailscale_host() {
        assert!(is_tailscale_host("100.64.1.2"));
        assert!(is_tailscale_host("100.127.255.254"));
        assert!(!is_tailscale_host("100.128.0.1"));
        assert!(!is_tailscale_host("192.168.1.2"));
    }
}
