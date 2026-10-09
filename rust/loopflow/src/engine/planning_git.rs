//! Custom-ref transport for an exported planning document. The common planning
//! writer owns transactional import; the exchange layer supplies the portable
//! format and semantic reconciliation.
//! This module never opens a store or uses the source branch, index or worktree.

use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::process::ProcessGroupGuard;
use sha2::{Digest, Sha256};

const MAX_DOCUMENT: usize = 16 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningRevision(String);

impl PlanningRevision {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningDocument {
    pub revision: PlanningRevision,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningPublication {
    /// The remote contains this revision, possibly followed by newer changes.
    Confirmed,
    /// Fresh remote evidence does not contain this revision. Reconcile before retry.
    Pending,
    /// Remote readback failed; the push may already have succeeded.
    Unconfirmed,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanningGitError {
    #[error("planning Git {operation} failed (exit {code:?}); local planning is retained")]
    Command {
        operation: &'static str,
        code: Option<i32>,
    },
    #[error("planning Git {0} timed out; publication may need readback")]
    Timeout(&'static str),
    #[error("planning Git I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("local planning changed while saving; reconcile with its retained revision")]
    ConcurrentWrite,
}

type Result<T> = std::result::Result<T, PlanningGitError>;

/// A pinned endpoint and ref. Persist this value, not the remote alias used to
/// select it. Ref separation is not access control.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanningDestination {
    endpoint: String,
    reference: String,
}

impl PlanningDestination {
    pub fn new(endpoint: &str, reference: &str) -> Result<Self> {
        if endpoint.is_empty() || endpoint.starts_with('-') || endpoint.contains(['\n', '\r']) {
            return Err(PlanningGitError::Invalid("select a planning Git endpoint"));
        }
        let suffix =
            reference
                .strip_prefix("refs/loopflow/planning/")
                .ok_or(PlanningGitError::Invalid(
                    "select a user-keyed or shared planning ref",
                ))?;
        let valid = if let Some(key) = suffix.strip_prefix("users/") {
            uuid::Uuid::parse_str(key).is_ok_and(|id| id.to_string() == key)
        } else if let Some(key) = suffix.strip_prefix("shared/") {
            !key.is_empty()
                && key
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        } else {
            false
        };
        if !valid {
            return Err(PlanningGitError::Invalid(
                "invalid user-keyed or shared planning ref",
            ));
        }

        // Local destinations must not change meaning when another checkout opens.
        if !endpoint.contains(':') && !Path::new(endpoint).is_absolute() {
            return Err(PlanningGitError::Invalid(
                "planning local endpoint must be absolute",
            ));
        }
        Ok(Self {
            endpoint: endpoint.into(),
            reference: reference.into(),
        })
    }

    /// Resolve an explicitly chosen alias once, without contacting its remote.
    /// Separate fetch/push endpoints would make readback an invalid receipt.
    pub fn resolve(repo: &Path, remote: &str, reference: &str) -> Result<Self> {
        if remote.is_empty() || remote.starts_with('-') {
            return Err(PlanningGitError::Invalid("select a planning Git remote"));
        }
        let read = |push: bool| -> Result<String> {
            let mut args = vec!["remote", "get-url", "--all"];
            if push {
                args.push("--push");
            }
            args.push(remote);
            let bytes =
                git(repo, "resolve destination", &args, &[])?.success("resolve destination")?;
            let text = String::from_utf8(bytes)
                .map_err(|_| PlanningGitError::Invalid("invalid planning endpoint"))?;
            let lines: Vec<_> = text.lines().collect();
            if lines.len() != 1 {
                return Err(PlanningGitError::Invalid(
                    "select one planning fetch/push endpoint",
                ));
            }
            Ok(lines[0].into())
        };
        let mut endpoint = read(false)?;
        if endpoint != read(true)? {
            return Err(PlanningGitError::Invalid(
                "planning fetch and push endpoints differ",
            ));
        }
        if !endpoint.contains(':') && !Path::new(&endpoint).is_absolute() {
            endpoint = repo
                .join(endpoint)
                .canonicalize()?
                .to_string_lossy()
                .into_owned();
        }
        Self::new(&endpoint, reference)
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn reference(&self) -> &str {
        &self.reference
    }
    pub fn id(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(format!("{}\0{}", self.endpoint, self.reference))
        )
    }
}

/// Construction uses only the saved binding, never the current remote alias.
#[derive(Debug)]
pub struct PlanningGit {
    repo: PathBuf,
    remote: String,
    reference: String,
    local_ref: String,
    observed_ref: String,
}

impl PlanningGit {
    pub fn new(repo: &Path, destination: &PlanningDestination) -> Result<Self> {
        PlanningDestination::new(destination.endpoint(), destination.reference())?;
        let namespace = destination.id();
        Ok(Self {
            reference: destination.reference().into(),
            local_ref: format!("refs/loopflow/planning-local/{namespace}"),
            observed_ref: format!("refs/loopflow/planning-observed/{namespace}"),
            repo: repo.to_path_buf(),
            remote: destination.endpoint().into(),
        })
    }

    pub fn local(&self) -> Result<Option<PlanningDocument>> {
        self.read_revision(&self.local_ref)?
            .map(|revision| self.read_document(revision))
            .transpose()
    }

    /// Fetch only the planning ref. Absence is explicit and is not a deletion.
    pub fn fetch(&self) -> Result<Option<PlanningDocument>> {
        let refs = self.checked(
            "discover",
            &["ls-remote", "--refs", &self.remote, &self.reference],
            &[],
        )?;
        if refs.is_empty() {
            return Ok(None);
        }
        // Isolate concurrent fetches. A failed transfer cannot read another
        // invocation's stale FETCH_HEAD or overwrite its temporary reference.
        let temporary = format!("refs/loopflow/planning-fetch-{}", uuid::Uuid::new_v4());
        let refspec = format!("{}:{temporary}", self.reference);
        let fetched: Result<PlanningDocument> = (|| {
            self.checked(
                "fetch",
                &[
                    "fetch",
                    "--no-tags",
                    "--no-write-fetch-head",
                    "--no-recurse-submodules",
                    &self.remote,
                    &refspec,
                ],
                &[],
            )?;
            let revision = self
                .read_revision(&temporary)?
                .ok_or(PlanningGitError::Invalid("fetched planning ref is missing"))?;
            let document = self.read_document(revision)?;
            self.checked(
                "retain observation",
                &[
                    "update-ref",
                    "--create-reflog",
                    &self.observed_ref,
                    document.revision.as_str(),
                ],
                &[],
            )?;
            Ok(document)
        })();
        let cleanup = self.checked("release fetch ref", &["update-ref", "-d", &temporary], &[]);
        let document = fetched?;
        cleanup?;
        Ok(Some(document))
    }

    /// Save the exchange layer's reconciled bytes. Parents are causal inputs, not
    /// permission to decide conflicts. The local ref update compares its old tip.
    pub fn save(
        &self,
        bytes: &[u8],
        expected: Option<&PlanningRevision>,
        incoming: Option<&PlanningRevision>,
    ) -> Result<PlanningDocument> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(PlanningGitError::Invalid(
                "planning document exceeds 16 MiB",
            ));
        }
        let blob = self.checked("write document", &["hash-object", "-w", "--stdin"], bytes)?;
        let blob = object_id(&blob)?;
        let tree = self.checked(
            "write tree",
            &["mktree"],
            format!("100644 blob {blob}\tplanning.json\n").as_bytes(),
        )?;
        let tree = object_id(&tree)?;
        let mut args = vec!["commit-tree", tree.as_str()];
        for parent in expected
            .into_iter()
            .chain(incoming.filter(|parent| Some(*parent) != expected))
        {
            args.extend(["-p", parent.as_str()]);
        }
        let commit = self.checked("write revision", &args, b"Synchronize planning\n")?;
        let revision = PlanningRevision(object_id(&commit)?);
        // Git accepts an empty expected value to require an absent reference.
        let updated = self.git(
            "save revision",
            &[
                "update-ref",
                "--create-reflog",
                &self.local_ref,
                revision.as_str(),
                expected.map(PlanningRevision::as_str).unwrap_or(""),
            ],
            &[],
        )?;
        if !updated.status.success() {
            if self.read_revision(&self.local_ref)?.as_ref() != expected {
                return Err(PlanningGitError::ConcurrentWrite);
            }
            updated.success("save revision")?;
        }
        Ok(PlanningDocument {
            revision,
            bytes: bytes.to_vec(),
        })
    }

    /// Publish once, without forcing or retrying a write, then inspect the remote.
    /// The caller retains its common-writer pending effect until confirmation.
    pub fn publish(&self, revision: &PlanningRevision) -> Result<PlanningPublication> {
        let retained = self
            .read_revision(&self.local_ref)?
            .ok_or(PlanningGitError::Invalid(
                "no local revision for this planning destination",
            ))?;
        if !self.is_ancestor(revision, &retained)? {
            return Err(PlanningGitError::Invalid(
                "revision belongs to another planning destination",
            ));
        }
        let refspec = format!("{}:{}", revision.as_str(), self.reference);
        // Any failed response is ambiguous until readback, including a timeout.
        let _push = self.git(
            "publish",
            &[
                "push",
                "--porcelain",
                "--no-follow-tags",
                "--recurse-submodules=no",
                &self.remote,
                &refspec,
            ],
            &[],
        );
        self.confirm(revision)
    }

    /// Readback also recovers a lost successful push response without republishing.
    pub fn confirm(&self, revision: &PlanningRevision) -> Result<PlanningPublication> {
        let remote = match self.fetch() {
            Ok(remote) => remote,
            Err(_) => return Ok(PlanningPublication::Unconfirmed),
        };
        if let Some(document) = &remote {
            if self.is_ancestor(revision, &document.revision)? {
                return Ok(PlanningPublication::Confirmed);
            }
        }
        Ok(PlanningPublication::Pending)
    }

    pub fn is_ancestor(
        &self,
        ancestor: &PlanningRevision,
        descendant: &PlanningRevision,
    ) -> Result<bool> {
        let output = self.git(
            "read ancestry",
            &[
                "merge-base",
                "--is-ancestor",
                ancestor.as_str(),
                descendant.as_str(),
            ],
            &[],
        )?;
        if output.status.code() == Some(1) {
            return Ok(false);
        }
        output.success("read ancestry").map(|_| true)
    }

    fn read_revision(&self, reference: &str) -> Result<Option<PlanningRevision>> {
        let output = self.git(
            "read revision",
            &["rev-parse", "--verify", "--quiet", reference],
            &[],
        )?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        let bytes = output.success("read revision")?;
        Ok(Some(PlanningRevision(object_id(&bytes)?)))
    }

    fn read_document(&self, revision: PlanningRevision) -> Result<PlanningDocument> {
        let tree = self.checked("read tree", &["ls-tree", revision.as_str()], &[])?;
        let tree = String::from_utf8(tree)
            .map_err(|_| PlanningGitError::Invalid("invalid planning tree"))?;
        let blob = tree
            .strip_prefix("100644 blob ")
            .and_then(|line| line.strip_suffix("\tplanning.json\n"))
            .ok_or(PlanningGitError::Invalid(
                "planning revision must contain only planning.json",
            ))?;
        let blob = object_id(blob.as_bytes())?;
        let size = self.checked("read document size", &["cat-file", "-s", &blob], &[])?;
        let size = String::from_utf8_lossy(&size)
            .trim()
            .parse::<usize>()
            .map_err(|_| PlanningGitError::Invalid("invalid planning document size"))?;
        if size > MAX_DOCUMENT {
            return Err(PlanningGitError::Invalid(
                "planning document exceeds 16 MiB",
            ));
        }
        let bytes = self.checked("read document", &["cat-file", "blob", &blob], &[])?;
        Ok(PlanningDocument { revision, bytes })
    }

    fn checked(&self, operation: &'static str, args: &[&str], input: &[u8]) -> Result<Vec<u8>> {
        self.git(operation, args, input)?.success(operation)
    }

    fn git(&self, operation: &'static str, args: &[&str], input: &[u8]) -> Result<GitOutput> {
        git(&self.repo, operation, args, input)
    }
}

fn git(repo: &Path, operation: &'static str, args: &[&str], input: &[u8]) -> Result<GitOutput> {
    let mut stdin = tempfile::tempfile()?;
    stdin.write_all(input)?;
    stdin.rewind()?;
    let mut stdout = tempfile::tempfile()?;
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(["-c", "core.hooksPath=/dev/null", "-c", "gc.auto=0"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Loopflow planning")
        .env("GIT_AUTHOR_EMAIL", "planning@loopflow.invalid")
        .env("GIT_COMMITTER_NAME", "Loopflow planning")
        .env("GIT_COMMITTER_EMAIL", "planning@loopflow.invalid")
        .stdin(stdin)
        .stdout(stdout.try_clone()?)
        .stderr(Stdio::null());
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let group = ProcessGroupGuard::new(child.id());
    let deadline = Instant::now() + DEADLINE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            result => {
                group.terminate();
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return match result {
                    Err(error) => Err(error.into()),
                    _ => Err(PlanningGitError::Timeout(operation)),
                };
            }
        }
    };
    group.terminate();
    Ok(GitOutput {
        status,
        stdout: read_output(&mut stdout)?,
    })
}
struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

impl GitOutput {
    fn success(self, operation: &'static str) -> Result<Vec<u8>> {
        if self.status.success() {
            Ok(self.stdout)
        } else {
            // Git diagnostics may contain credential-bearing URLs.
            Err(PlanningGitError::Command {
                operation,
                code: self.status.code(),
            })
        }
    }
}

fn read_output(file: &mut File) -> Result<Vec<u8>> {
    file.rewind()?;
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOCUMENT {
        return Err(PlanningGitError::Invalid(
            "planning Git output exceeds 16 MiB",
        ));
    }
    Ok(bytes)
}

fn object_id(bytes: &[u8]) -> Result<String> {
    let text = String::from_utf8_lossy(bytes);
    let id = text.trim();
    if ![40, 64].contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PlanningGitError::Invalid("invalid planning Git object ID"));
    }
    Ok(id.into())
}
