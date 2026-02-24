//! Annotation layer for mechanistic interpretability.
//!
//! Records one canonical envelope per agent run as a sidecar file, then
//! appends outcome signals after execution. Env vars propagate trace
//! identity to agent subprocesses.
//!
//! Sidecar files live at `.lf/annotation/<trace_id>/envelope.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Env var names ────────────────────────────────────────────────────────────

pub const LF_TRACE_ID: &str = "LF_TRACE_ID";
pub const LF_SPAN_ID: &str = "LF_SPAN_ID";
pub const LF_ANNOTATION_FILE: &str = "LF_ANNOTATION_FILE";
pub const LF_STEP_TYPE: &str = "LF_STEP_TYPE";
pub const LF_FLOW_POSITION: &str = "LF_FLOW_POSITION";

// ── Schema types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnnotationEnvelopeV1 {
    pub schema_version: u32,
    pub trace: TraceContext,
    pub spawn: SpawnMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpawnMetadata {
    pub step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    pub direction: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<String>,
    pub model: String,
    pub run_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_position: Option<FlowPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowPosition {
    pub step_index: u32,
    pub total_steps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts_produced: Option<Vec<String>>,
}

// ── ID generation ────────────────────────────────────────────────────────────

pub fn new_trace_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn new_span_id() -> String {
    Uuid::new_v4().to_string()
}

// ── Sidecar lifecycle ────────────────────────────────────────────────────────

/// Annotation directory for a given trace within the repo.
pub fn annotation_dir(repo_root: &Path, trace_id: &str) -> PathBuf {
    repo_root.join(".lf/annotation").join(trace_id)
}

/// Write the spawn envelope to disk. Returns the path to `envelope.json`.
pub fn write_envelope(
    repo_root: &Path,
    envelope: &AnnotationEnvelopeV1,
) -> std::io::Result<PathBuf> {
    let dir = annotation_dir(repo_root, &envelope.trace.trace_id);
    fs::create_dir_all(&dir)?;
    let path = dir.join("envelope.json");
    let json = serde_json::to_string_pretty(envelope).map_err(std::io::Error::other)?;
    fs::write(&path, json)?;
    Ok(path)
}

/// Read an envelope from disk.
pub fn read_envelope(repo_root: &Path, trace_id: &str) -> std::io::Result<AnnotationEnvelopeV1> {
    let path = annotation_dir(repo_root, trace_id).join("envelope.json");
    let json = fs::read_to_string(&path)?;
    serde_json::from_str(&json).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Append outcome to an existing envelope. Reads, merges, writes atomically.
pub fn append_outcome(repo_root: &Path, trace_id: &str, outcome: Outcome) -> std::io::Result<()> {
    let mut envelope = read_envelope(repo_root, trace_id)?;
    envelope.outcome = Some(outcome);
    write_envelope(repo_root, &envelope)?;
    Ok(())
}

// ── Convenience builders ─────────────────────────────────────────────────────

/// Builds an `AnnotationEnvelopeV1` for a local `lf` run.
pub fn build_lf_envelope(
    step: &str,
    model: &str,
    directions: &[String],
    area: Option<&str>,
    wave: Option<&str>,
    run_mode: &str,
) -> AnnotationEnvelopeV1 {
    AnnotationEnvelopeV1 {
        schema_version: 1,
        trace: TraceContext {
            trace_id: new_trace_id(),
            span_id: new_span_id(),
            parent_span_id: std::env::var(LF_SPAN_ID).ok(),
        },
        spawn: SpawnMetadata {
            step: step.to_string(),
            flow: None,
            direction: directions.to_vec(),
            area: area.map(str::to_string),
            wave: wave.map(str::to_string),
            model: model.to_string(),
            run_mode: run_mode.to_string(),
            flow_position: None,
        },
        outcome: None,
    }
}

/// Parameters for building a wave annotation envelope.
pub struct WaveEnvelopeParams<'a> {
    pub step: &'a str,
    pub flow: &'a str,
    pub model: &'a str,
    pub directions: &'a [String],
    pub area: Option<&'a str>,
    pub wave: &'a str,
    pub step_index: u32,
    pub total_steps: u32,
    pub parent_span_id: Option<&'a str>,
}

/// Builds an `AnnotationEnvelopeV1` for a daemon wave step.
pub fn build_wave_envelope(params: &WaveEnvelopeParams<'_>) -> AnnotationEnvelopeV1 {
    AnnotationEnvelopeV1 {
        schema_version: 1,
        trace: TraceContext {
            trace_id: new_trace_id(),
            span_id: new_span_id(),
            parent_span_id: params.parent_span_id.map(str::to_string),
        },
        spawn: SpawnMetadata {
            step: params.step.to_string(),
            flow: Some(params.flow.to_string()),
            direction: params.directions.to_vec(),
            area: params.area.map(str::to_string),
            wave: Some(params.wave.to_string()),
            model: params.model.to_string(),
            run_mode: "auto".to_string(),
            flow_position: Some(FlowPosition {
                step_index: params.step_index,
                total_steps: params.total_steps,
            }),
        },
        outcome: None,
    }
}

// ── Env var propagation ──────────────────────────────────────────────────────

/// Set annotation env vars on a `std::process::Command` (used by `lf` local runs).
pub fn set_annotation_env(
    cmd: &mut Command,
    envelope: &AnnotationEnvelopeV1,
    envelope_path: &Path,
) {
    cmd.env(LF_TRACE_ID, &envelope.trace.trace_id);
    cmd.env(LF_SPAN_ID, &envelope.trace.span_id);
    cmd.env(LF_ANNOTATION_FILE, envelope_path.to_string_lossy().as_ref());
    cmd.env(LF_STEP_TYPE, &envelope.spawn.step);
    if let Some(ref pos) = envelope.spawn.flow_position {
        cmd.env(
            LF_FLOW_POSITION,
            format!("{}/{}", pos.step_index, pos.total_steps),
        );
    }
}

/// Set annotation env vars on a `tokio::process::Command` (used by `lfd` daemon runs).
pub fn set_annotation_env_async(
    cmd: &mut tokio::process::Command,
    envelope: &AnnotationEnvelopeV1,
    envelope_path: &Path,
) {
    cmd.env(LF_TRACE_ID, &envelope.trace.trace_id);
    cmd.env(LF_SPAN_ID, &envelope.trace.span_id);
    cmd.env(LF_ANNOTATION_FILE, envelope_path.to_string_lossy().as_ref());
    cmd.env(LF_STEP_TYPE, &envelope.spawn.step);
    if let Some(ref pos) = envelope.spawn.flow_position {
        cmd.env(
            LF_FLOW_POSITION,
            format!("{}/{}", pos.step_index, pos.total_steps),
        );
    }
}

/// Collect annotation env vars as key-value pairs (for Docker or other executors).
pub fn annotation_env_pairs(
    envelope: &AnnotationEnvelopeV1,
    envelope_path: &Path,
) -> Vec<(String, String)> {
    let mut pairs = vec![
        (LF_TRACE_ID.to_string(), envelope.trace.trace_id.clone()),
        (LF_SPAN_ID.to_string(), envelope.trace.span_id.clone()),
        (
            LF_ANNOTATION_FILE.to_string(),
            envelope_path.to_string_lossy().to_string(),
        ),
        (LF_STEP_TYPE.to_string(), envelope.spawn.step.clone()),
    ];
    if let Some(ref pos) = envelope.spawn.flow_position {
        pairs.push((
            LF_FLOW_POSITION.to_string(),
            format!("{}/{}", pos.step_index, pos.total_steps),
        ));
    }
    pairs
}

// ── Outcome helpers ──────────────────────────────────────────────────────────

/// Build an `Outcome` from agent exit results.
pub fn build_outcome(exit_code: i32, start: Instant, repo_root: Option<&Path>) -> Outcome {
    let duration_ms = start.elapsed().as_millis() as u64;
    let artifacts = repo_root.and_then(|root| list_changed_files(root).ok());
    Outcome {
        exit_code,
        duration_ms,
        verdict: None,
        artifacts_produced: artifacts,
    }
}

fn list_changed_files(repo_root: &Path) -> std::io::Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["diff", "--name-only", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    Ok(files)
}

/// Ensure `.lf/annotation/` is in `.gitignore`.
pub fn ensure_annotation_gitignored(repo_root: &Path) -> std::io::Result<()> {
    let gitignore = repo_root.join(".gitignore");
    let entry = ".lf/annotation/";

    if gitignore.exists() {
        let content = fs::read_to_string(&gitignore)?;
        if content.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
        let mut new_content = content;
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(entry);
        new_content.push('\n');
        fs::write(&gitignore, new_content)?;
    } else {
        fs::write(&gitignore, format!("{entry}\n"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn annotation_envelope_roundtrip() {
        let envelope = build_lf_envelope(
            "implement",
            "claude:opus",
            &["designer".to_string()],
            Some("src/"),
            None,
            "auto",
        );

        let tmp = tempdir().expect("tempdir");
        let path = write_envelope(tmp.path(), &envelope).expect("write");
        assert!(path.exists());

        let read_back = read_envelope(tmp.path(), &envelope.trace.trace_id).expect("read");
        assert_eq!(read_back.schema_version, 1);
        assert_eq!(read_back.spawn.step, "implement");
        assert_eq!(read_back.spawn.model, "claude:opus");
        assert_eq!(read_back.spawn.direction, vec!["designer".to_string()]);
        assert!(read_back.outcome.is_none());
    }

    #[test]
    fn annotation_outcome_append() {
        let envelope = build_lf_envelope("gate", "claude:sonnet", &[], None, None, "auto");
        let tmp = tempdir().expect("tempdir");
        write_envelope(tmp.path(), &envelope).expect("write");

        let outcome = Outcome {
            exit_code: 0,
            duration_ms: 1500,
            verdict: Some("SHIP".to_string()),
            artifacts_produced: Some(vec!["src/main.rs".to_string()]),
        };
        append_outcome(tmp.path(), &envelope.trace.trace_id, outcome.clone()).expect("append");

        let read_back = read_envelope(tmp.path(), &envelope.trace.trace_id).expect("read");
        let read_outcome = read_back.outcome.expect("outcome present");
        assert_eq!(read_outcome.exit_code, 0);
        assert_eq!(read_outcome.duration_ms, 1500);
        assert_eq!(read_outcome.verdict, Some("SHIP".to_string()));
        assert_eq!(
            read_outcome.artifacts_produced,
            Some(vec!["src/main.rs".to_string()])
        );
    }

    #[test]
    fn annotation_wave_envelope_has_flow_position() {
        let envelope = build_wave_envelope(&WaveEnvelopeParams {
            step: "implement",
            flow: "ship",
            model: "claude:opus",
            directions: &["product-engineer".to_string()],
            area: None,
            wave: "engbot",
            step_index: 1,
            total_steps: 4,
            parent_span_id: Some("parent-span-123"),
        });

        assert_eq!(envelope.spawn.flow, Some("ship".to_string()));
        assert_eq!(envelope.spawn.wave, Some("engbot".to_string()));
        let pos = envelope.spawn.flow_position.expect("flow_position present");
        assert_eq!(pos.step_index, 1);
        assert_eq!(pos.total_steps, 4);
        assert_eq!(
            envelope.trace.parent_span_id,
            Some("parent-span-123".to_string())
        );
    }

    #[test]
    fn annotation_env_pairs_includes_all_vars() {
        let envelope = build_wave_envelope(&WaveEnvelopeParams {
            step: "gate",
            flow: "ship",
            model: "claude:sonnet",
            directions: &[],
            area: None,
            wave: "engbot",
            step_index: 2,
            total_steps: 5,
            parent_span_id: None,
        });
        let path = PathBuf::from("/tmp/.lf/annotation/abc/envelope.json");
        let pairs = annotation_env_pairs(&envelope, &path);

        let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&LF_TRACE_ID));
        assert!(keys.contains(&LF_SPAN_ID));
        assert!(keys.contains(&LF_ANNOTATION_FILE));
        assert!(keys.contains(&LF_STEP_TYPE));
        assert!(keys.contains(&LF_FLOW_POSITION));

        let flow_pos = pairs
            .iter()
            .find(|(k, _)| k == LF_FLOW_POSITION)
            .map(|(_, v)| v.as_str())
            .expect("flow position present");
        assert_eq!(flow_pos, "2/5");
    }

    #[test]
    fn annotation_spawn_metadata_preserved_after_outcome() {
        let envelope = build_lf_envelope(
            "debug",
            "claude:sonnet",
            &["infra-engineer".to_string()],
            Some("src/api/"),
            Some("scan-wave"),
            "auto",
        );
        let tmp = tempdir().expect("tempdir");
        write_envelope(tmp.path(), &envelope).expect("write");

        let outcome = Outcome {
            exit_code: 1,
            duration_ms: 3000,
            verdict: None,
            artifacts_produced: None,
        };
        append_outcome(tmp.path(), &envelope.trace.trace_id, outcome).expect("append");

        let read_back = read_envelope(tmp.path(), &envelope.trace.trace_id).expect("read");
        assert_eq!(read_back.spawn.step, "debug");
        assert_eq!(read_back.spawn.wave, Some("scan-wave".to_string()));
        assert_eq!(read_back.spawn.area, Some("src/api/".to_string()));
        assert_eq!(read_back.outcome.expect("outcome").exit_code, 1);
    }

    #[test]
    fn annotation_ensure_gitignore_creates_and_deduplicates() {
        let tmp = tempdir().expect("tempdir");

        ensure_annotation_gitignored(tmp.path()).expect("first call");
        let content = fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        assert!(content.contains(".lf/annotation/"));

        ensure_annotation_gitignored(tmp.path()).expect("second call (idempotent)");
        let content = fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        let count = content
            .lines()
            .filter(|l| l.trim() == ".lf/annotation/")
            .count();
        assert_eq!(count, 1, "should not duplicate gitignore entry");
    }

    #[test]
    fn annotation_sidecar_directory_structure() {
        let envelope = build_lf_envelope("implement", "claude", &[], None, None, "interactive");
        let tmp = tempdir().expect("tempdir");
        let path = write_envelope(tmp.path(), &envelope).expect("write");

        assert!(path.ends_with("envelope.json"));
        assert!(
            path.parent()
                .expect("parent")
                .file_name()
                .expect("dir name")
                .to_string_lossy()
                == envelope.trace.trace_id
        );
    }
}
