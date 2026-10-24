//! RunPod's REST API, reduced to the facts renting needs (Etude's adapter).
//!
//! The account key stays in this process. Replies are narrowed to a few
//! checked fields and never logged whole. A create is sent once and never
//! retried; any failure leaves it unknown whether a pod exists.

use std::time::Duration;

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::{cpu_quote, CpuFlavor, Hardware, Quote, Template, Usd};

const BASE: &str = "https://api.runpod.io/v2/";
const MAX_REPLY_BYTES: usize = 8 * 1024 * 1024;

/// Sends one request and returns the status and body; `Err` when no reply came.
pub type Send = Box<dyn Fn(&str, &str, Option<Vec<u8>>) -> std::io::Result<(u16, Vec<u8>)>>;

/// No usable answer. This never means the thing asked about is absent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("RunPod: {0}")]
pub struct Unavailable(pub String);

type Result<T> = std::result::Result<T, Unavailable>;

/// One pod, as checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pod {
    pub id: String,
    /// Our identity, set at create.
    pub name: String,
    pub usd_per_hour: Usd,
    pub created_at: OffsetDateTime,
    pub status: String,
    /// Where SSH answers, once the pod has a public address and a port for 22.
    pub ssh: Option<(String, u16)>,
}

impl Pod {
    pub fn is_gone(&self) -> bool {
        matches!(self.status.as_str(), "EXITED" | "TERMINATED")
    }
}

pub struct RunPod {
    send: Send,
}

impl std::fmt::Debug for RunPod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RunPod")
    }
}

impl RunPod {
    /// With the account key from the environment of the lf process that rents.
    pub fn from_env() -> anyhow::Result<Self> {
        let key = std::env::var("RUNPOD_API_KEY")
            .ok()
            .filter(|key| !key.is_empty() && !key.chars().any(char::is_whitespace))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "RUNPOD_API_KEY is not set; run with `doppler run -p etude -c prd -- lf …`"
                )
            })?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client builds with static options");
        Ok(Self::with_send(Box::new(move |method, url, body| {
            let method = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let mut request = client
                .request(method, url)
                .bearer_auth(&key)
                .header("Content-Type", "application/json")
                // RunPod's front door refuses default agent names with a 403.
                .header("User-Agent", "loopflow");
            if let Some(body) = body {
                request = request.body(body);
            }
            let reply = request
                .send()
                .map_err(|error| std::io::Error::other(error.without_url().to_string()))?;
            let status = reply.status().as_u16();
            let bytes = reply
                .bytes()
                .map_err(|error| std::io::Error::other(error.without_url().to_string()))?;
            Ok((status, bytes.to_vec()))
        })))
    }

    pub(crate) fn with_send(send: Send) -> Self {
        Self { send }
    }

    fn ask(
        &self,
        method: &str,
        path: &str,
        params: &[(&str, String)],
        payload: Option<Value>,
        absent_ok: bool,
    ) -> Result<Option<Value>> {
        let query = if params.is_empty() {
            String::new()
        } else {
            format!(
                "?{}",
                serde_urlencoded::to_string(params).expect("string pairs encode")
            )
        };
        let body = payload.map(|payload| payload.to_string().into_bytes());
        let (status, raw) = (self.send)(method, &format!("{BASE}{path}{query}"), body)
            .map_err(|_| Unavailable(format!("{method} {path}: no reply")))?;
        if absent_ok && status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&status) || raw.len() > MAX_REPLY_BYTES {
            return Err(Unavailable(format!("{method} {path}: status {status}")));
        }
        if raw.is_empty() {
            return Ok(Some(Value::Null));
        }
        serde_json::from_slice(&raw)
            .map(Some)
            .map_err(|_| Unavailable(format!("{method} {path}: unreadable reply")))
    }

    fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        Ok(self
            .ask("GET", path, params, None, false)?
            .unwrap_or(Value::Null))
    }

    /// The secure-cloud hourly price for one machine of this template, and
    /// whether any can be had.
    pub fn quote(&self, template: &Template) -> anyhow::Result<Quote> {
        match &template.gpu {
            None => self.cpu_quote(template),
            Some(gpu) => Ok(self.gpu_quote(gpu, template)?),
        }
    }

    fn cpu_quote(&self, template: &Template) -> anyhow::Result<Quote> {
        let catalog = self.get(
            "catalog/cpus",
            &[
                ("include", "AVAILABILITY".into()),
                ("product", "POD".into()),
                (
                    "vcpuCount",
                    super::catalog_vcpus(template.vcpus).to_string(),
                ),
            ],
        )?;
        let unreadable = || Unavailable("the CPU catalog was unreadable".into());
        let flavors = catalog["cpus"]
            .as_array()
            .ok_or_else(unreadable)?
            .iter()
            .map(|row| {
                Some(CpuFlavor {
                    id: row["id"].as_str()?.to_string(),
                    min_vcpus: u32::try_from(row["vcpu"]["min"].as_u64()?).ok()?,
                    max_vcpus: u32::try_from(row["vcpu"]["max"].as_u64()?).ok()?,
                    ram_gb_per_vcpu: row["ramGbPerVcpu"].as_f64()?,
                    usd_per_vcpu_hour: usd(&row["price"]["securePerVcpu"])?,
                    available: available(row, template.data_center.as_deref()),
                    data_centers: stocked(row, template.data_center.as_deref()),
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(unreadable)?;
        cpu_quote(&flavors, template.vcpus, template.memory_gb).ok_or_else(|| {
            anyhow::anyhow!(
                "no RunPod CPU pod has {} vCPUs and {} GB",
                template.vcpus,
                template.memory_gb
            )
        })
    }

    fn gpu_quote(&self, gpu: &str, template: &Template) -> Result<Quote> {
        let catalog = self.get(
            "catalog/gpus",
            &[
                ("include", "AVAILABILITY".into()),
                ("product", "POD".into()),
                ("cloud", "SECURE".into()),
            ],
        )?;
        let row = catalog["gpus"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"] == gpu))
            .ok_or_else(|| Unavailable(format!("no price for {gpu}")))?;
        let usd_per_hour = usd(&row["price"]["secure"])
            .filter(|price| *price > Usd::ZERO)
            .ok_or_else(|| Unavailable(format!("no price for {gpu}")))?;
        Ok(Quote {
            hardware: Hardware::Gpu {
                id: gpu.to_string(),
                min_vcpus: template.vcpus,
                min_memory_gb: template.memory_gb,
            },
            usd_per_hour,
            available: available(row, template.data_center.as_deref()),
            data_centers: stocked(row, template.data_center.as_deref()),
        })
    }

    /// One pod by id; `None` when the provider says it does not exist.
    pub fn pod(&self, id: &str) -> Result<Option<Pod>> {
        check_id(id)?;
        self.ask("GET", &format!("pods/{id}"), &[], None, true)?
            .map(|raw| read_pod(&raw))
            .transpose()
    }

    /// The pod carrying this name, from the whole inventory. `None` means every
    /// page was read and none has it.
    pub fn named(&self, name: &str) -> Result<Option<Pod>> {
        let mut params = vec![("limit", "1000".to_string())];
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let page = self.get("pods", &params)?;
            let pods = page["pods"]
                .as_array()
                .ok_or_else(|| Unavailable("the pod inventory was unreadable".into()))?
                .iter()
                .map(read_pod)
                .collect::<Result<Vec<_>>>()?;
            if let Some(found) = pods.into_iter().find(|pod| pod.name == name) {
                return Ok(Some(found));
            }
            let more = page["pagination"]["hasNextPage"].as_bool().unwrap_or(false);
            let cursor = page["pagination"]["nextCursor"].as_str().unwrap_or("");
            if !more {
                return Ok(None);
            }
            if cursor.is_empty() || !seen.insert(cursor.to_string()) {
                break;
            }
            params.retain(|(key, _)| *key != "cursor");
            params.push(("cursor", cursor.to_string()));
        }
        Err(Unavailable("the pod inventory did not end".into()))
    }

    /// Ask for one pod, once. Any error leaves it unknown whether one exists.
    pub fn create(&self, request: &CreatePod) -> Result<Pod> {
        if !request.image.contains("@sha256:")
            || request.env.iter().any(|(k, _)| k == "RUNPOD_API_KEY")
        {
            return Err(Unavailable(
                "the image must be pinned by digest and the account key kept out".into(),
            ));
        }
        let env: serde_json::Map<String, Value> = request
            .env
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect();
        let mut payload = json!({
            "name": request.name,
            "image": request.image,
            "cloud": "SECURE",
            "disk": request.disk_gb,
            "ports": ["22/tcp"],
            "entrypoint": request.entrypoint,
            "env": env,
        });
        match &request.hardware {
            Hardware::Cpu { flavor, vcpus } => {
                payload["cpu"] = json!({"id": flavor, "vcpuCount": vcpus});
            }
            Hardware::Gpu {
                id,
                min_vcpus,
                min_memory_gb,
            } => {
                payload["gpu"] = json!({
                    "id": id,
                    "count": 1,
                    "minRamPerGpu": min_memory_gb,
                    "minVcpuCountPerGpu": min_vcpus,
                });
            }
        }
        if !request.data_centers.is_empty() {
            payload["dataCenterIds"] = json!(request.data_centers);
        }
        let raw = self
            .ask("POST", "pods", &[], Some(payload), false)?
            .unwrap_or(Value::Null);
        let pod = read_pod(&raw)?;
        if pod.name != request.name || pod.usd_per_hour <= Usd::ZERO || pod.is_gone() {
            return Err(Unavailable(
                "the created pod is not the one asked for".into(),
            ));
        }
        Ok(pod)
    }

    /// Delete exactly this pod and return it as last seen. Refuses when the id
    /// now names another pod; an already absent pod is already given back.
    pub fn delete(&self, id: &str, name: &str) -> Result<Option<Pod>> {
        let Some(current) = self.pod(id)? else {
            return Ok(None);
        };
        if current.name != name {
            return Err(Unavailable("the pod id now names a different pod".into()));
        }
        self.ask("DELETE", &format!("pods/{id}"), &[], None, true)?;
        Ok(Some(current))
    }

    /// What RunPod has billed this pod, counting only contiguous whole hours
    /// from its creation: never estimate across a missing hour.
    pub fn billed(&self, pod: &Pod, now: OffsetDateTime) -> Result<Usd> {
        let first = hour_floor(pod.created_at);
        let end = hour_floor(now);
        if end <= first {
            return Ok(Usd::ZERO);
        }
        let raw = self.get(
            "billing/pods",
            &[
                ("podId", pod.id.clone()),
                ("bucketSize", "hour".into()),
                ("startTime", first.format(&Rfc3339).expect("UTC formats")),
                ("endTime", end.format(&Rfc3339).expect("UTC formats")),
            ],
        )?;
        let incomplete = || Unavailable("the billing records were incomplete".into());
        let mut hours = std::collections::BTreeMap::new();
        for row in raw["records"].as_array().ok_or_else(incomplete)? {
            let start = timestamp(&row["startTime"]).ok_or_else(incomplete)?;
            let finish = timestamp(&row["endTime"]).ok_or_else(incomplete)?;
            if row["podId"] != pod.id.as_str()
                || finish != start + time::Duration::HOUR
                || start < first
                || start >= end
                || hours
                    .insert(start, usd(&row["totalAmount"]).ok_or_else(incomplete)?)
                    .is_some()
            {
                return Err(incomplete());
            }
        }
        let mut through = first;
        let mut total = Usd::ZERO;
        for (hour, amount) in hours {
            if hour != through {
                break;
            }
            total = total + amount;
            through += time::Duration::HOUR;
        }
        Ok(total)
    }
}

/// One create, exactly as admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePod {
    pub name: String,
    pub hardware: Hardware,
    pub image: String,
    pub disk_gb: u32,
    /// Preferred placement, from the quote; empty lets RunPod choose.
    pub data_centers: Vec<String>,
    pub env: Vec<(String, String)>,
    pub entrypoint: Vec<String>,
}

fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 200
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then_some(())
        .ok_or_else(|| Unavailable("not a provider id".into()))
}

fn usd(value: &Value) -> Option<Usd> {
    let price = value.as_f64()?;
    (price.is_finite() && price >= 0.0).then(|| Usd::from_micros((price * 1e6).round() as i64))
}

fn timestamp(value: &Value) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value.as_str()?, &Rfc3339).ok()
}

fn hour_floor(at: OffsetDateTime) -> OffsetDateTime {
    let at = at.to_offset(time::UtcOffset::UTC);
    at.replace_time(time::Time::from_hms(at.hour(), 0, 0).expect("valid hour"))
}

fn port(value: &Value) -> Option<u16> {
    value.as_u64().and_then(|port| u16::try_from(port).ok())
}

fn read_pod(raw: &Value) -> Result<Pod> {
    let fields = || -> Option<Pod> {
        let id = raw["id"].as_str()?.to_string();
        check_id(&id).ok()?;
        // v2 reports the published 22/tcp port under `ssh.direct` once the pod
        // has a public address; `runtime.ports` carries the same mapping. The
        // proxy endpoint is PTY-only and cannot carry `lf --machine`.
        let direct = &raw["ssh"]["direct"];
        let ssh = direct["host"]
            .as_str()
            .filter(|host| !host.is_empty())
            .zip(port(&direct["port"]))
            .map(|(host, port)| (host.to_string(), port))
            .or_else(|| {
                raw["runtime"]["ports"]
                    .as_array()?
                    .iter()
                    .find_map(|mapping| {
                        (mapping["private"] == 22 && mapping["type"] == "tcp")
                            .then(|| {
                                mapping["ip"]
                                    .as_str()
                                    .filter(|ip| !ip.is_empty())
                                    .zip(port(&mapping["public"]))
                                    .map(|(ip, port)| (ip.to_string(), port))
                            })
                            .flatten()
                    })
            });
        Some(Pod {
            id,
            name: raw["name"].as_str()?.to_string(),
            usd_per_hour: usd(&raw["cost"])?,
            created_at: timestamp(&raw["createdAt"])?,
            status: raw["status"].as_str()?.to_string(),
            ssh,
        })
    };
    fields().ok_or_else(|| Unavailable("a pod record was missing its expected fields".into()))
}

/// False only when a catalog row positively reports none: anywhere when
/// unpinned, or in every data center the pin names.
fn available(row: &Value, pin: Option<&str>) -> bool {
    match pin {
        None => row["availability"] != "NONE",
        Some(_) => !stocked(row, pin).is_empty(),
    }
}

/// The pinned data centers with stock. A pin is a full id (`US-IL-1`) or a
/// region prefix (`US`); unpinned, the list is empty and the scheduler chooses.
fn stocked(row: &Value, pin: Option<&str>) -> Vec<String> {
    let Some(pin) = pin else {
        return Vec::new();
    };
    row["dataCenters"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|place| place["availability"] != "NONE")
        .filter_map(|place| place["id"].as_str())
        .filter(|id| id.starts_with(pin))
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// The pod
// ---------------------------------------------------------------------------

const GUARDIAN: &str = include_str!("guardian.py");

/// Starts, in order: the guardian (nothing else starts until it is ready),
/// key-only sshd with a non-root `lf` user, the tools, then sleeps. The
/// laptop drives everything after this over SSH.
const BOOT: &str = r#"set -u
mkdir -p /opt/lf /run/lf
echo "$LF_GUARDIAN" | base64 -d > /opt/lf/guardian.py
unset LF_GUARDIAN
python3 /opt/lf/guardian.py > /var/log/lf-guardian.log 2>&1 &
while [ ! -e /run/lf/guardian-ready ]; do sleep 1; done
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq && apt-get install -y -qq openssh-server git curl ca-certificates sudo build-essential pkg-config libssl-dev >/var/log/lf-apt.log 2>&1
id lf >/dev/null 2>&1 || useradd -m -s /bin/bash lf
install -d -m 700 -o lf -g lf /home/lf/.ssh
printf '%s\n' "${PUBLIC_KEY:-}" > /home/lf/.ssh/authorized_keys
chown lf:lf /home/lf/.ssh/authorized_keys && chmod 600 /home/lf/.ssh/authorized_keys
mkdir -p /run/sshd
cat > /etc/ssh/sshd_config.d/lf.conf <<'EOF'
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
AllowUsers lf
EOF
ssh-keygen -A >/dev/null 2>&1
/usr/sbin/sshd
(curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y -qq nodejs gh) >/var/log/lf-tools.log 2>&1
su - lf -c 'mkdir -p ~/.local && npm config set prefix ~/.local && npm install -g @anthropic-ai/claude-code' >>/var/log/lf-tools.log 2>&1
if [ "${LF_INSTALL:-published}" = upload ]; then
  install -d -o lf -g lf /home/lf/.local /home/lf/.local/bin
  while [ ! -x /home/lf/.local/bin/lf ]; do sleep 2; done
else
  su - lf -c 'curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh' >>/var/log/lf-tools.log 2>&1
fi
touch /run/lf/ready
exec sleep infinity
"#;

/// The pod's environment and entrypoint. The account key never enters it:
/// the guardian uses the pod-scoped key RunPod injects, which can only delete
/// this pod. With `upload_lf`, boot waits for the laptop to upload an `lf`
/// binary instead of running the published installer, so a development build
/// rents pods that run its own Linux build.
pub fn boot(
    deadline: OffsetDateTime,
    cap_usd: Usd,
    public_key: &str,
    upload_lf: bool,
) -> (Vec<(String, String)>, Vec<String>) {
    use base64::Engine;
    let mut env = vec![
        (
            "LF_GUARDIAN".to_string(),
            base64::engine::general_purpose::STANDARD.encode(GUARDIAN),
        ),
        (
            "LF_DEADLINE".to_string(),
            deadline
                .to_offset(time::UtcOffset::UTC)
                .format(&Rfc3339)
                .expect("UTC formats"),
        ),
        ("LF_MAX_USD".to_string(), cap_usd.to_decimal()),
        ("PUBLIC_KEY".to_string(), public_key.trim().to_string()),
    ];
    if upload_lf {
        env.push(("LF_INSTALL".to_string(), "upload".to_string()));
    }
    let entrypoint = vec!["bash".to_string(), "-c".to_string(), BOOT.to_string()];
    (env, entrypoint)
}

#[cfg(test)]
mod tests;
