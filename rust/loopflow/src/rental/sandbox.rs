//! Rented machines on every provider, reduced to the facts renting needs.
//!
//! Each call runs the embedded `sandbox.py` under `uv`, with the provider's
//! pinned SDK where it has one. The account key stays in the bridge. A create
//! is sent once and never retried; any failure leaves it unknown whether a
//! machine exists, and its name finds it later. Each create carries the
//! embedded `guardian.py`, which holds the deadline on providers without a
//! lifetime of their own.

use std::io::Write;
use std::process::{Command, Stdio};

use serde::Deserialize;
use serde_json::{json, Value};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::{Provider, Rental, Usd};

const BRIDGE: &str = include_str!("sandbox.py");
const GUARDIAN: &str = include_str!("guardian.py");

/// Runs one bridge operation and returns its stdout; `Err` when it could not run.
pub type Call = Box<dyn Fn(&str, Value) -> std::io::Result<Vec<u8>>>;

/// No usable answer. This never means the thing asked about is absent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{provider}: {message}")]
pub struct Unavailable {
    pub provider: Provider,
    pub message: String,
}

type Result<T> = std::result::Result<T, Unavailable>;

/// One sandbox, as the provider reports it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Sandbox {
    pub id: String,
    /// Ended, deleted or failed; it runs nothing more, and deleting it frees
    /// whatever remains (a Fly app's IPv4, an exited pod's disk).
    pub gone: bool,
    /// The SSH destination, reported only by create.
    pub route: Option<String>,
    /// The price the provider charges, where it reports one (RunPod).
    pub usd_per_hour: Option<Usd>,
}

pub struct Sandboxes {
    provider: Provider,
    call: Call,
}

impl std::fmt::Debug for Sandboxes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sandboxes({})", self.provider)
    }
}

impl Sandboxes {
    /// With the account this lf process can reach; refuses when it has none.
    pub fn from_env(provider: Provider) -> anyhow::Result<Self> {
        if let Some(missing) = provider.missing_credentials() {
            return Err(anyhow::anyhow!(missing));
        }
        Ok(Self::with_call(
            provider,
            Box::new(move |op, request| {
                let mut child = Command::new("uv")
                    .args(["run", "--quiet", "--no-project"])
                    .args(provider.sdk().map(|sdk| ["--with", sdk]).into_iter().flatten())
                    .args(["python", "-c", BRIDGE, provider.as_str(), op])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|error| match error.kind() {
                        std::io::ErrorKind::NotFound => std::io::Error::other(
                            "uv is not installed; `curl -LsSf https://astral.sh/uv/install.sh | sh`",
                        ),
                        _ => error,
                    })?;
                child
                    .stdin
                    .take()
                    .expect("stdin is piped")
                    .write_all(request.to_string().as_bytes())?;
                let output = child.wait_with_output()?;
                if output.stdout.is_empty() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let last = stderr.lines().last().unwrap_or("no output");
                    return Err(std::io::Error::other(last.to_string()));
                }
                Ok(output.stdout)
            }),
        ))
    }

    pub(crate) fn with_call(provider: Provider, call: Call) -> Self {
        Self { provider, call }
    }

    fn unavailable(&self, message: impl Into<String>) -> Unavailable {
        Unavailable {
            provider: self.provider,
            message: message.into(),
        }
    }

    fn ask(&self, op: &str, request: Value) -> Result<Option<Sandbox>> {
        let raw =
            (self.call)(op, request).map_err(|error| self.unavailable(format!("{op}: {error}")))?;
        let reply: Value = serde_json::from_slice(&raw)
            .map_err(|_| self.unavailable(format!("{op}: unreadable reply")))?;
        if let Some(error) = reply.get("error").and_then(Value::as_str) {
            return Err(self.unavailable(format!("{op}: {error}")));
        }
        if reply.is_null() {
            return Ok(None);
        }
        serde_json::from_value(reply)
            .map(Some)
            .map_err(|_| self.unavailable(format!("{op}: a sandbox was missing its fields")))
    }

    /// Ask once for the admitted rental's machine, authorizing `public_key`
    /// for `lf`, and wait until it has an SSH route. The provider deletes it at
    /// the deadline whatever else fails; the guardian may only shorten that,
    /// by what the cap buys from `created_at`.
    pub fn create(&self, rental: &Rental, public_key: &str) -> Result<Sandbox> {
        let offer = self
            .provider
            .offer(&rental.hardware)
            .map_err(|reason| self.unavailable(format!("create: {reason}")))?;
        let utc = |at: OffsetDateTime| {
            at.to_offset(time::UtcOffset::UTC)
                .format(&Rfc3339)
                .expect("UTC formats")
        };
        let lifetime_minutes = (rental.deadline - OffsetDateTime::now_utc())
            .whole_minutes()
            .clamp(1, i64::from(u32::MAX));
        let sandbox = self
            .ask(
                "create",
                json!({
                    "name": rental.provider_name,
                    "vcpus": rental.hardware.vcpus,
                    "memory_gb": rental.hardware.memory_gb,
                    "disk_gb": rental.hardware.disk_gb,
                    "gpu": rental.hardware.gpu,
                    "spot": rental.hardware.spot,
                    "type": offer.machine_type,
                    // The most a spot machine may be charged.
                    "usd_per_hour": rental.usd_per_hour.to_decimal(),
                    "lifetime_minutes": lifetime_minutes,
                    "deadline": utc(rental.deadline),
                    "created_at": utc(rental.created_at),
                    "max_usd": rental.cap_usd.to_decimal(),
                    "guardian": GUARDIAN,
                    "public_key": public_key.trim(),
                }),
            )?
            .ok_or_else(|| self.unavailable("create: no sandbox in the reply"))?;
        if sandbox.gone || sandbox.route.is_none() {
            return Err(self.unavailable("create: the sandbox has no SSH route"));
        }
        Ok(sandbox)
    }

    /// One sandbox by id; `None` when the provider says it does not exist.
    pub fn get(&self, id: &str) -> Result<Option<Sandbox>> {
        self.ask("get", json!({ "id": id }))
    }

    /// The sandbox carrying this name; `None` when none does.
    pub fn named(&self, name: &str) -> Result<Option<Sandbox>> {
        self.ask("get", json!({ "name": name }))
    }

    /// Delete it; an already absent sandbox is already given back.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.ask("delete", json!({ "id": id })).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replying(reply: &'static str) -> Sandboxes {
        Sandboxes::with_call(
            Provider::Daytona,
            Box::new(move |_, _| Ok(reply.as_bytes().to_vec())),
        )
    }

    #[test]
    fn absent_and_failed_replies_differ() {
        assert_eq!(replying("null").get("sb-1").unwrap(), None);
        let error = replying(r#"{"error": "DaytonaError: no region"}"#)
            .get("sb-1")
            .unwrap_err();
        assert!(error.to_string().contains("no region"), "{error}");
    }

    #[test]
    fn a_reported_price_is_read_exactly() {
        let pod =
            replying(r#"{"id": "p1", "gone": false, "route": null, "usd_per_hour": "0.590000"}"#)
                .get("p1")
                .unwrap()
                .unwrap();
        assert_eq!(pod.usd_per_hour, Some(Usd::parse("0.59").unwrap()));
    }

    #[test]
    fn a_create_without_a_route_is_not_a_machine() {
        let created = replying(r#"{"id": "sb-1", "gone": false, "route": null}"#)
            .create(&crate::rental::tests::rented(false), "ssh-ed25519 AAAA");
        assert!(created.is_err());
    }
}
