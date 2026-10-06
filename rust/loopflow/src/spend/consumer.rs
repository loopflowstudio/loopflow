//! Designated-tool receipts come from an isolated child over a local pipe or SSH.
//! The administrator trusts each Home, its Docker daemon and preinstalled image. No
//! uploaded receipt or caller-supplied command can establish a successful observation.
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::{Consumer, ReportExport, RequirementId};
use crate::durable::{Home, HomeId};
use crate::engine::wave_home::HomeRoute;

const IMAGE: &str = "rust:bookworm";
const LIMIT: u64 = 8 * 1024 * 1024;
const PROBE: &str = include_str!("consumer.py");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumptionReceipt {
    pub invocation: String,
    pub binding: Option<ConsumptionBinding>,
    pub consumer: Consumer,
    pub period: String,
    pub export_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumptionBinding {
    pub home: HomeId,
    pub requirement: RequirementId,
    pub revision: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConsumptionError {
    #[error("export recipient, period or shape does not match designated access")]
    Denied,
    #[error("isolated export consumer unavailable; no authenticated receipt collected")]
    Unavailable,
}

pub async fn consume(
    path: &Path,
    consumer: &Consumer,
    period: &str,
) -> Result<(ReportExport, ConsumptionReceipt), ConsumptionError> {
    consume_bound(path, consumer, period, None, None).await
}

pub async fn consume_bound(
    path: &Path,
    consumer: &Consumer,
    period: &str,
    binding: Option<ConsumptionBinding>,
    remote: Option<&Home>,
) -> Result<(ReportExport, ConsumptionReceipt), ConsumptionError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| ConsumptionError::Unavailable)?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ConsumptionError::Unavailable)?;
    if bytes.len() as u64 > LIMIT {
        return Err(ConsumptionError::Denied);
    }
    let report: ReportExport =
        serde_json::from_slice(&bytes).map_err(|_| ConsumptionError::Denied)?;
    if report.repo != consumer.repo
        || report.wave_id != consumer.wave_id
        || report.period != period
        || report
            .amounts
            .iter()
            .any(|amount| consumer.wave_id.is_some() && amount.wave_id != consumer.wave_id)
    {
        return Err(ConsumptionError::Denied);
    }
    let receipt = ConsumptionReceipt {
        invocation: uuid::Uuid::new_v4().to_string(),
        binding,
        consumer: consumer.clone(),
        period: period.into(),
        export_sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    // Snapshot bytes travel over stdin: no host file, credential or Store mount.
    let request = serde_json::to_vec(&serde_json::json!({
        "receipt": receipt,
        "report": String::from_utf8(bytes).map_err(|_| ConsumptionError::Denied)?,
    }))
    .map_err(|_| ConsumptionError::Denied)?;
    if let Some(home) = remote {
        let result = deliver_remote(home, &receipt, &request).await?;
        validate_receipt(&result, &receipt)?;
        return Ok((report, receipt));
    }
    let docker = [
        "/usr/bin/docker",
        "/usr/local/bin/docker",
        "/opt/homebrew/bin/docker",
    ]
    .into_iter()
    .find(|path| Path::new(path).is_file())
    .ok_or(ConsumptionError::Unavailable)?;
    let mut context = Command::new(docker);
    context.env_clear().args([
        "context",
        "inspect",
        "--format",
        "{{.Endpoints.docker.Host}}",
    ]);
    let endpoint = capture(&mut context, &[], 4096, 5, None).await?;
    let endpoint = std::str::from_utf8(&endpoint)
        .map_err(|_| ConsumptionError::Unavailable)?
        .trim();
    // A remote TCP/SSH context cannot certify an environment bound to this Home.
    if !endpoint.starts_with("unix:///") {
        return Err(ConsumptionError::Unavailable);
    }
    let name = format!("lf-spend-{}", receipt.invocation);
    let result = run_probe(docker, endpoint, &name, &request).await;
    // Killing the client on timeout need not kill its container. Always remove it.
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        docker_command(docker, endpoint)
            .args(["rm", "--force", &name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status(),
    )
    .await;
    validate_receipt(&result?, &receipt)?;
    Ok((report, receipt))
}

fn docker_command(executable: &str, endpoint: &str) -> Command {
    let mut command = Command::new(executable);
    command.env_clear().args(["--host", endpoint]);
    command
}

async fn run_probe(
    docker: &str,
    endpoint: &str,
    name: &str,
    request: &[u8],
) -> Result<Vec<u8>, ConsumptionError> {
    let mut command = docker_command(docker, endpoint);
    command.args(probe_args(name));
    capture(&mut command, request, 65536, 60, None).await
}

async fn capture(
    command: &mut Command,
    request: &[u8],
    limit: u64,
    seconds: u64,
    expected_home: Option<&HomeId>,
) -> Result<Vec<u8>, ConsumptionError> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| ConsumptionError::Unavailable)?;
    let mut stdin = child.stdin.take().ok_or(ConsumptionError::Unavailable)?;
    let mut stdout = child.stdout.take().ok_or(ConsumptionError::Unavailable)?;
    tokio::time::timeout(Duration::from_secs(seconds), async {
        if let Some(expected) = expected_home {
            let mut identity = Vec::new();
            loop {
                let byte = stdout
                    .read_u8()
                    .await
                    .map_err(|_| ConsumptionError::Unavailable)?;
                if byte == b'\n' {
                    break;
                }
                identity.push(byte);
                if identity.len() > 4096 {
                    return Err(ConsumptionError::Unavailable);
                }
            }
            let observed: HomeId =
                serde_json::from_slice(&identity).map_err(|_| ConsumptionError::Unavailable)?;
            if &observed != expected {
                return Err(ConsumptionError::Unavailable);
            }
        }
        let send = async {
            stdin.write_all(request).await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let receive = async {
            let mut bytes = Vec::new();
            stdout.take(limit + 1).read_to_end(&mut bytes).await?;
            Ok::<_, std::io::Error>(bytes)
        };
        let (_, bytes) =
            tokio::try_join!(send, receive).map_err(|_| ConsumptionError::Unavailable)?;
        if bytes.len() as u64 > limit
            || !child
                .wait()
                .await
                .map_err(|_| ConsumptionError::Unavailable)?
                .success()
        {
            return Err(ConsumptionError::Unavailable);
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| ConsumptionError::Unavailable)?
}

fn probe_args(name: &str) -> Vec<String> {
    [
        "run",
        "--rm",
        "--pull",
        "never",
        "--name",
        name,
        "-i",
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--user",
        "65534:65534",
        "--pids-limit",
        "32",
        "--memory",
        "128m",
        "--entrypoint",
        "/usr/bin/python3",
        IMAGE,
        "-I",
        "-c",
        PROBE,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn validate_receipt(bytes: &[u8], expected: &ConsumptionReceipt) -> Result<(), ConsumptionError> {
    let received: ConsumptionReceipt =
        serde_json::from_slice(bytes).map_err(|_| ConsumptionError::Unavailable)?;
    if &received != expected {
        return Err(ConsumptionError::Unavailable);
    }
    Ok(())
}

async fn deliver_remote(
    home: &Home,
    receipt: &ConsumptionReceipt,
    request: &[u8],
) -> Result<Vec<u8>, ConsumptionError> {
    if receipt.binding.as_ref().map(|b| &b.home) != Some(&home.id) {
        return Err(ConsumptionError::Unavailable);
    }
    let route = HomeRoute::parse(&home.route).ok_or(ConsumptionError::Unavailable)?;
    let destination = route
        .ssh_destination()
        .ok_or(ConsumptionError::Unavailable)?;
    let name = format!("lf-spend-{}", receipt.invocation);
    let payload = serde_json::to_vec(&serde_json::json!({
        "home": home.id,
        "name": name,
        "args": probe_args(&name),
        "request": String::from_utf8_lossy(request),
    }))
    .map_err(|_| ConsumptionError::Unavailable)?;
    let script = include_str!("remote_consumer.py");
    let quoted = format!("'{}'", script.replace('\'', "'\"'\"'"));
    let mut command = Command::new("/usr/bin/ssh");
    command.env_clear().args([
        "-F",
        "/dev/null",
        "-T",
        "-a",
        "-x",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ClearAllForwardings=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=10",
        "-o",
        "ServerAliveCountMax=3",
    ]);
    if let Some(port) = route.ssh_port() {
        command.args(["-p", &port.to_string()]);
    }
    command
        .arg(destination)
        .arg(format!("/usr/bin/python3 -I -c {quoted}"));
    capture(&mut command, &payload, 65536, 100, Some(&home.id)).await
}

#[cfg(test)]
mod tests {
    use super::{capture, validate_receipt, ConsumptionBinding, ConsumptionReceipt, PROBE};
    use crate::durable::HomeId;
    use crate::spend::{Consumer, RequirementId};
    use sha2::{Digest, Sha256};
    use tokio::process::Command;

    fn receipt() -> ConsumptionReceipt {
        ConsumptionReceipt {
            invocation: uuid::Uuid::new_v4().to_string(),
            binding: Some(ConsumptionBinding {
                home: HomeId::new(),
                requirement: RequirementId("report-reader".into()),
                revision: "r1".into(),
            }),
            consumer: Consumer {
                repo: "example/two".into(),
                wave_id: None,
            },
            period: "2026-09".into(),
            export_sha256: "digest".into(),
        }
    }

    #[test]
    fn remote_receipts_reject_replay_and_every_changed_binding() {
        let expected = receipt();
        let original = serde_json::to_value(&expected).unwrap();
        validate_receipt(&serde_json::to_vec(&original).unwrap(), &expected).unwrap();
        for pointer in [
            "/invocation",
            "/binding/home",
            "/binding/requirement",
            "/binding/revision",
            "/consumer/repo",
            "/period",
            "/export_sha256",
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(pointer).unwrap() = serde_json::json!("different");
            assert!(
                validate_receipt(&serde_json::to_vec(&changed).unwrap(), &expected).is_err(),
                "{pointer}"
            );
        }
        let mut next = expected.clone();
        next.invocation = uuid::Uuid::new_v4().to_string();
        assert!(validate_receipt(&serde_json::to_vec(&expected).unwrap(), &next).is_err());
    }

    #[tokio::test]
    async fn remote_channel_fixture_consumes_only_after_matching_home() {
        let mut expected = receipt();
        let raw = serde_json::json!({
            "period": "2026-09", "repo": "example/two", "wave_id": null,
            "generated_at": 0, "amounts": [], "totals": [], "coverage": []
        })
        .to_string();
        expected.export_sha256 = format!("{:x}", Sha256::digest(raw.as_bytes()));
        let home = &expected.binding.as_ref().unwrap().home;
        let request =
            serde_json::to_vec(&serde_json::json!({"receipt": expected, "report": raw})).unwrap();
        // This pipe fixture exercises the real reader and handshake; Docker and
        // SSH authentication retain their independent integration boundary.
        let script = format!(
            "print({}, flush=True)\n{}\nprint(json.dumps(receipt))",
            serde_json::to_string(&serde_json::to_string(home).unwrap()).unwrap(),
            PROBE.split("assert not any").next().unwrap()
        );
        let mut command = Command::new("/usr/bin/python3");
        command.env_clear().args(["-I", "-c", &script]);
        let output = capture(&mut command, &request, 65536, 5, Some(home))
            .await
            .unwrap();
        validate_receipt(&output, &expected).unwrap();

        let marker = tempfile::NamedTempFile::new().unwrap();
        let script = format!("import sys\nprint({}, flush=True)\ndata = sys.stdin.buffer.read()\nopen({}, 'wb').write(data)", serde_json::to_string(&serde_json::to_string(&HomeId::new()).unwrap()).unwrap(), serde_json::to_string(&marker.path().to_str().unwrap()).unwrap());
        let mut wrong = Command::new("/usr/bin/python3");
        wrong.env_clear().args(["-I", "-c", &script]);
        assert!(capture(&mut wrong, &request, 65536, 5, Some(home))
            .await
            .is_err());
        assert!(std::fs::read(marker.path()).unwrap().is_empty());
        let mut unavailable = Command::new("/usr/bin/false");
        assert!(capture(&mut unavailable, &request, 65536, 5, Some(home))
            .await
            .is_err());
    }
}
