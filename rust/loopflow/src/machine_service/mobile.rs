//! Host setup owns only its recorded Serve route. The existing Machine service
//! owns the listener, and the host's Tailscale installation owns networking.
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::wire::{Operation, Reply};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Configuration {
    pub enabled: bool,
    pub port: u16,
    pub endpoint: String,
}

pub(super) fn configured() -> Result<Option<Configuration>> {
    let value =
        crate::config::read_global_config_at(&crate::store::lf_home_dir().join("config.yaml"))?;
    value
        .get("mobile")
        .cloned()
        .map(serde_yaml_ng::from_value)
        .transpose()
        .context("Read mobile setup")
}

fn save(configuration: &Configuration) -> Result<()> {
    crate::config::update_global_config_at(&crate::store::lf_home_dir(), |value| {
        let mapping = value
            .as_mapping_mut()
            .context("Machine configuration is not a mapping")?;
        let key = serde_yaml_ng::Value::String("mobile".into());
        mapping.insert(key, serde_yaml_ng::to_value(configuration)?);
        Ok(())
    })
}

fn lock() -> Result<fs::File> {
    let home = crate::store::lf_home_dir();
    fs::create_dir_all(&home)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(home.join("mobile-setup.lock"))?;
    lock.lock_exclusive()?;
    Ok(lock)
}

fn configure(port: Option<u16>) -> Result<Option<u16>> {
    let (_, response) = super::client::connect(Operation::ConfigureRemote { port })?;
    match response.result {
        Reply::Remote { port } => Ok(port),
        Reply::Error { message, .. } => anyhow::bail!(message),
        _ => anyhow::bail!("Update the Machine service before enabling mobile access"),
    }
}

pub fn enable() -> Result<Configuration> {
    let _lock = lock()?;
    let host = Host::discover()?;
    let mut before = host.json(&["serve", "status", "--json"])?;
    let previous = configured()?;
    let https_port = select_port(&before, previous.as_ref(), &host.dns)?;
    let endpoint = format!("https://{}:{https_port}", host.dns);
    // Retire a still-owned old address before replacing its ownership record.
    // An independently edited route belongs to its editor and is left alone.
    if let Some(old) = previous
        .as_ref()
        .filter(|old| old.endpoint != endpoint && owns_route(&before, old))
    {
        host.remove(old)?;
        before = host.json(&["serve", "status", "--json"])?;
    }
    super::installation::ensure()?;
    // An edited or publicly forwarded former route must not keep reaching the
    // carrier through its old loopback port. Move the listener, leave that route.
    let desired = previous
        .as_ref()
        .filter(|config| owns_route(&before, config))
        .map_or(0, |config| config.port);
    let port = match configure(Some(desired)) {
        Ok(port) => port,
        Err(_) if desired != 0 => configure(Some(0))?,
        Err(error) => return Err(error),
    }
    .context("The Machine did not start its mobile listener")?;
    let mut selected = Configuration {
        enabled: false,
        port,
        endpoint,
    };
    // Record a disabled, retryable setup before publishing a route. A lost CLI
    // response or failed persistence must not orphan a route or enable it at boot.
    if let Err(error) = save(&selected) {
        let _ = configure(
            previous
                .as_ref()
                .filter(|config| config.enabled)
                .map(|config| config.port),
        );
        return Err(error);
    }
    let result = (|| {
        if !owns_route(&before, &selected) {
            host.run(&[
                "serve",
                "--bg",
                &format!("--https={https_port}"),
                "--yes",
                &format!("http://127.0.0.1:{port}"),
            ])?;
        }
        let after = host.json(&["serve", "status", "--json"])?;
        anyhow::ensure!(
            owns_route(&after, &selected),
            "Tailscale did not establish the private Loopflow endpoint"
        );
        anyhow::ensure!(
            other_routes(&before, &selected.endpoint) == other_routes(&after, &selected.endpoint),
            "Tailscale changed another route; inspect Serve configuration before continuing"
        );
        selected.enabled = true;
        save(&selected)?;
        Ok(())
    })();
    if let Err(error) = result {
        // Keep the disabled record so a retry recognizes any route a timed-out
        // Tailscale command may have created. Stop admission immediately.
        let _ = configure(None);
        return Err(error);
    }
    Ok(selected)
}

pub fn disable() -> Result<()> {
    let _lock = lock()?;
    let previous = configured()?;
    if let Some(previous) = &previous {
        let mut disabled = previous.clone();
        disabled.enabled = false;
        // Keep route ownership even if Tailscale cannot remove it right now,
        // while preventing a supervised restart from re-enabling admission.
        save(&disabled)?;
    }
    // Stop local admission first, even if Tailscale is currently offline.
    if let Err(error) = configure(None) {
        let stopped = error.downcast_ref::<std::io::Error>().is_some_and(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            )
        });
        if !stopped {
            return Err(error);
        }
    }
    if let Some(previous) = previous {
        let cleanup = (|| -> Result<()> {
            let host = Host::discover()?;
            if owns_route(&host.json(&["serve", "status", "--json"])?, &previous) {
                host.remove(&previous)?;
            }
            Ok(())
        })();
        if let Err(error) = cleanup {
            eprintln!("Mobile access is off; Tailscale route cleanup is pending ({error}). Run `lf machine service mobile --off` again when Tailscale is available.");
        }
    }
    Ok(())
}

struct Host {
    binary: PathBuf,
    dns: String,
}

impl Host {
    fn discover() -> Result<Self> {
        let mut candidates = Vec::new();
        if let Some(binary) = crate::os_process::which_on_path(Path::new("tailscale")) {
            candidates.push(binary);
        }
        #[cfg(target_os = "macos")]
        for directory in [
            PathBuf::from("/Applications"),
            crate::installation::account_home()?.join("Applications"),
        ] {
            let Ok(entries) = fs::read_dir(directory) else {
                continue;
            };
            let mut bundles: Vec<_> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("Tailscale") && name.ends_with(".app"))
                })
                .collect();
            bundles.sort();
            for bundle in bundles {
                for executable in ["Tailscale", "tailscale"] {
                    let binary = bundle.join("Contents/MacOS").join(executable);
                    if binary.is_file() {
                        candidates.push(binary);
                    }
                }
            }
        }
        let mut installed = false;
        for binary in candidates {
            let probe = Self {
                binary,
                dns: String::new(),
            };
            let Ok(status) = probe.json(&["status", "--json"]) else {
                continue;
            };
            installed = true;
            if status["BackendState"] != "Running" {
                continue;
            }
            let dns = status["Self"]["DNSName"]
                .as_str()
                .unwrap_or("")
                .trim_end_matches('.');
            anyhow::ensure!(
                !dns.is_empty() && dns.ends_with(".ts.net"),
                "Enable Tailscale MagicDNS and HTTPS for this Machine"
            );
            return Ok(Self {
                binary: probe.binary,
                dns: dns.into(),
            });
        }
        anyhow::bail!(if installed {
            "Open Tailscale and sign in on this Machine, then try again"
        } else {
            "Install and open Tailscale on this Machine, then try again"
        })
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = crate::os_process::bounded_output(
            Command::new(&self.binary).args(args),
            Duration::from_secs(15),
        )
        .context("Tailscale did not respond; open it and check its connection")?;
        anyhow::ensure!(output.status.success(), "Tailscale could not complete setup. Check that it is signed in and private HTTPS/Serve is enabled for this tailnet");
        Ok(output.stdout)
    }

    fn json(&self, args: &[&str]) -> Result<Value> {
        serde_json::from_slice(&self.run(args)?).context("Tailscale returned an unsupported status")
    }

    fn remove(&self, config: &Configuration) -> Result<()> {
        let url = reqwest::Url::parse(&config.endpoint)?;
        self.run(&[
            "serve",
            &format!(
                "--https={}",
                url.port_or_known_default().context("Missing HTTPS port")?
            ),
            "off",
        ])?;
        Ok(())
    }
}

fn endpoint_key(endpoint: &str) -> Result<String> {
    let url = reqwest::Url::parse(endpoint)?;
    Ok(format!(
        "{}:{}",
        url.host_str().context("Missing Machine hostname")?,
        url.port_or_known_default().context("Missing HTTPS port")?
    ))
}

fn owns_route(status: &Value, config: &Configuration) -> bool {
    let Ok(key) = endpoint_key(&config.endpoint) else {
        return false;
    };
    let Some((_, port)) = key.rsplit_once(':') else {
        return false;
    };
    status["TCP"][port] == json!({"HTTPS":true})
        && status["Web"][&key]
            == json!({"Handlers":{"/":{"Proxy":format!("http://127.0.0.1:{}",config.port)}}})
        && status["AllowFunnel"][&key] != true
        && !status["Foreground"]
            .as_object()
            .is_some_and(|sessions| sessions.values().any(|config| port_used(config, port)))
}

fn port_used(status: &Value, port: &str) -> bool {
    let suffix = format!(":{port}");
    status["TCP"].get(port).is_some()
        || ["Web", "AllowFunnel"].iter().any(|section| {
            status[section]
                .as_object()
                .is_some_and(|entries| entries.keys().any(|key| key.ends_with(&suffix)))
        })
        || status["Foreground"]
            .as_object()
            .is_some_and(|sessions| sessions.values().any(|config| port_used(config, port)))
}

fn select_port(status: &Value, previous: Option<&Configuration>, dns: &str) -> Result<u16> {
    if let Some(previous) = previous {
        if let Ok(url) = reqwest::Url::parse(&previous.endpoint) {
            if let Some(port) = url.port_or_known_default() {
                if url.host_str() == Some(dns)
                    && (owns_route(status, previous) || !port_used(status, &port.to_string()))
                {
                    return Ok(port);
                }
            }
        }
    }
    for port in [443, 8443]
        .into_iter()
        .chain(10000..=u16::MAX)
        .chain(1024..10000)
    {
        if !port_used(status, &port.to_string()) {
            return Ok(port);
        }
    }
    anyhow::bail!("No unused private HTTPS port is available in Tailscale Serve")
}

fn other_routes(status: &Value, endpoint: &str) -> Value {
    let mut other = status.clone();
    if let Ok(key) = endpoint_key(endpoint) {
        if let Some((_, port)) = key.rsplit_once(':') {
            for (section, entry) in [
                ("TCP", port),
                ("Web", key.as_str()),
                ("AllowFunnel", key.as_str()),
            ] {
                if let Some(map) = other.get_mut(section).and_then(Value::as_object_mut) {
                    map.remove(entry);
                }
                if other[section].as_object().is_some_and(|map| map.is_empty()) {
                    if let Some(map) = other.as_object_mut() {
                        map.remove(section);
                    }
                }
            }
        }
    }
    other
}

#[cfg(test)]
mod tests {
    use super::{other_routes, owns_route, select_port, Configuration};
    use serde_json::json;

    #[test]
    fn setup_preserves_other_routes_and_never_selects_funnel() {
        let config = Configuration {
            enabled: true,
            port: 49250,
            endpoint: "https://host.tail.ts.net:8443".into(),
        };
        assert_eq!(
            select_port(&json!({}), Some(&config), "host.tail.ts.net").unwrap(),
            8443,
            "Re-enabling keeps the phone's recorded endpoint when its port is free"
        );
        let before = json!({
            "TCP":{"443":{"HTTPS":true}},
            "Web":{"host.tail.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3000"}}}},
            "AllowFunnel":{"host.tail.ts.net:10000":true}
        });
        assert_eq!(
            select_port(&before, None, "host.tail.ts.net").unwrap(),
            8443
        );
        let mut after = before.clone();
        after["TCP"]["8443"] = json!({"HTTPS":true});
        after["Web"]["host.tail.ts.net:8443"] =
            json!({"Handlers":{"/":{"Proxy":"http://127.0.0.1:49250"}}});
        assert!(owns_route(&after, &config));
        assert_eq!(
            select_port(&after, Some(&config), "host.tail.ts.net").unwrap(),
            8443
        );
        assert_eq!(
            other_routes(&before, &config.endpoint),
            other_routes(&after, &config.endpoint)
        );
        after["AllowFunnel"]["host.tail.ts.net:8443"] = json!(true);
        assert!(!owns_route(&after, &config));
        assert_eq!(
            select_port(&after, Some(&config), "host.tail.ts.net").unwrap(),
            10001
        );
        after["AllowFunnel"]["host.tail.ts.net:8443"] = json!(false);
        after["Foreground"] = json!({"another-session":{"TCP":{"8443":{"HTTPS":true}}}});
        assert!(!owns_route(&after, &config));
        assert_eq!(
            select_port(&after, Some(&config), "host.tail.ts.net").unwrap(),
            10001
        );
        after.as_object_mut().unwrap().remove("Foreground");
        after["Web"]["host.tail.ts.net:8443"]["Handlers"]["/other"] = json!({"Text":"keep me"});
        assert!(
            !owns_route(&after, &config),
            "An edited route must never be removed as ours"
        );
        for port in 10000..10100 {
            after["TCP"][port.to_string()] = json!({"HTTPS":true});
        }
        assert_eq!(
            select_port(&after, None, "host.tail.ts.net").unwrap(),
            10100
        );
    }
}
