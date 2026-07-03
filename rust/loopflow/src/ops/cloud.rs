//! `lf op cloud <vendor>` — scaffold a per-repo looping session in a vendor's
//! cloud (claude, codex) that re-runs a wave's Goal prompt on the vendor's own
//! recurring schedule.
//!
//! This is the **A2** shape: lfd assembles the launch scaffold — the rendered
//! Goal prompt, the flows-as-Skills, a loop instruction, and an Asana `.mcp.json`
//! — and the human presses go in the vendor UI. lfd does not drive the vendor
//! API or own the runtime; we rent the vendor's persistence and recurrence.
//!
//! Once launched, `--session-url` records the vendor session URL back onto the
//! wave (`cloud_session_url` in `wave/<name>/GOAL.md`) so Concerto can deep-link
//! out to it with `NSWorkspace.open`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::{
    available_flow_names, load_goal, render_goal, sync_skills, GoalRenderContext, SkillSyncOptions,
};
use crate::lfd::http::routes::wave_config::{read_wave_config, update_wave_cloud_session_url};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;

/// Asana's hosted remote MCP server (OAuth via bearer token). A fresh cloud
/// clone has no `lf` binary and no local Asana OAuth, so the cloud session
/// reaches the roadmap over MCP instead of `lf op pm`.
const ASANA_MCP_URL: &str = "https://mcp.asana.com/sse";

/// Where the human creates the recurring routine. There is no create-routine
/// API today (A1), so we deep-link to the vendor UI and let the human press go.
const CLAUDE_CLOUD_URL: &str = "https://claude.ai/new";

/// Appended to the rendered Goal prompt: tells the vendor session it is a loop
/// and how to reach the roadmap over MCP on each scheduled run.
const CLOUD_LOOP_INSTRUCTION: &str = "\n\n<lf:cloud-loop>\n\
This prompt is a recurring loop. Register it as a scheduled routine in your \
vendor's cloud so it re-runs on a cadence. On each scheduled run:\n\
1. Read this wave's roadmap from Asana via the `asana` MCP server.\n\
2. Pick the next open roadmap item.\n\
3. Do the work in this repo, open a PR, and record progress on the roadmap.\n\
Then stop until the next scheduled run.\n\
</lf:cloud-loop>\n";

/// The cloud vendors a wave can be scaffolded for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Claude,
}

impl Vendor {
    fn parse(raw: &str) -> OpsResult<Self> {
        match raw.trim().to_lowercase().as_str() {
            "claude" => Ok(Vendor::Claude),
            "codex" => Err(OpsError::Message(
                "codex cloud scaffolding is a follow-on increment; start with `lf op cloud claude`"
                    .to_string(),
            )),
            other => Err(OpsError::Message(format!(
                "unknown cloud vendor '{other}' (expected: claude)"
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Vendor::Claude => "claude",
        }
    }

    fn deep_link(self) -> &'static str {
        match self {
            Vendor::Claude => CLAUDE_CLOUD_URL,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CloudLaunchOptions {
    pub vendor: String,
    pub wave: Option<String>,
    /// When set, record this already-launched vendor session URL onto the wave
    /// instead of scaffolding.
    pub session_url: Option<String>,
}

/// What `cloud_launch` did — scaffold a launch, or record a session URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudMode {
    Scaffold,
    Record,
}

#[derive(Debug, Clone)]
pub struct CloudLaunchResult {
    pub wave: String,
    pub vendor: String,
    pub mode: CloudMode,
    pub prompt_path: Option<PathBuf>,
    pub mcp_path: Option<PathBuf>,
    pub skills_written: usize,
    pub deep_link: Option<String>,
    pub session_url: Option<String>,
}

/// Scaffold (or record) a vendor-cloud looping session for a wave.
pub fn cloud_launch(
    repo: &Path,
    options: &CloudLaunchOptions,
    progress: &impl Progress,
) -> OpsResult<CloudLaunchResult> {
    let vendor = Vendor::parse(&options.vendor)?;
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let wave_dir = repo.join("wave").join(&wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{wave}/"
        )));
    }

    // Record mode: persist a launched session URL so Concerto can reopen it.
    if let Some(url) = options.session_url.as_deref() {
        let url = url.trim();
        if url.is_empty() {
            return Err(OpsError::Message(
                "--session-url must not be empty".to_string(),
            ));
        }
        update_wave_cloud_session_url(repo, &wave, Some(url.to_string()))
            .map_err(OpsError::Message)?;
        progress.status(&format!(
            "recorded {} cloud session for wave/{wave}",
            vendor.as_str()
        ));
        return Ok(CloudLaunchResult {
            wave,
            vendor: vendor.as_str().to_string(),
            mode: CloudMode::Record,
            prompt_path: None,
            mcp_path: None,
            skills_written: 0,
            deep_link: None,
            session_url: Some(url.to_string()),
        });
    }

    // Scaffold mode: rendered Goal prompt + flows-as-Skills + Asana MCP.
    progress.status(&format!(
        "scaffolding {} cloud loop for wave/{wave}",
        vendor.as_str()
    ));

    let prompt = render_cloud_prompt(repo, &wave)?;
    let prompt_path = repo.join(".lf").join("cloud").join(&wave).join("PROMPT.md");
    write_file(&prompt_path, &prompt)?;

    let report = sync_skills(repo, &SkillSyncOptions::default())
        .map_err(|err| OpsError::Message(format!("failed to sync skills: {err}")))?;

    let token = crate::ops::pm::asana_access_token()?;
    let mcp = build_asana_mcp_json(&token)?;
    let mcp_path = repo.join(".mcp.json");
    write_file(&mcp_path, &mcp)?;

    // The prompt scaffold and the token-bearing .mcp.json are generated, local,
    // and secret — keep them out of git the same way skill sync does.
    ensure_git_excluded(repo, &[".mcp.json", ".lf/cloud/"])?;

    Ok(CloudLaunchResult {
        wave,
        vendor: vendor.as_str().to_string(),
        mode: CloudMode::Scaffold,
        prompt_path: Some(prompt_path),
        mcp_path: Some(mcp_path),
        skills_written: report.written.len(),
        deep_link: Some(vendor.deep_link().to_string()),
        session_url: None,
    })
}

/// Render the wave's Goal into a vendor-launchable loop prompt: the same
/// context `lf goal` assembles, plus a loop instruction. In-flight dispatches
/// are omitted — a cloud clone has no local `lfd` to query.
fn render_cloud_prompt(repo: &Path, wave: &str) -> OpsResult<String> {
    let goal = load_goal(wave, repo).map_err(|err| {
        OpsError::Message(format!("failed to load goal for wave '{wave}': {err}"))
    })?;
    let wave_config = read_wave_config(repo, wave).unwrap_or_default();
    let memory =
        fs::read_to_string(repo.join("wave").join(wave).join("MEMORY.md")).unwrap_or_default();

    let ctx = GoalRenderContext {
        flows: available_flow_names(repo),
        roadmap: wave_config.roadmap.unwrap_or_default(),
        memory,
        metrics: wave_config.metrics.unwrap_or_default(),
        in_flight: Vec::new(),
    };

    let mut message = render_goal(&goal, &ctx);
    message.push_str(CLOUD_LOOP_INSTRUCTION);
    Ok(message)
}

/// Wire Asana over MCP: the hosted remote server authenticated with the wave's
/// stored OAuth token. The token grants the cloud session read/write on the
/// workspace; the roadmap project GID travels in the Goal prompt.
fn build_asana_mcp_json(token: &str) -> OpsResult<String> {
    let value = serde_json::json!({
        "mcpServers": {
            "asana": {
                "type": "sse",
                "url": ASANA_MCP_URL,
                "headers": {
                    "Authorization": format!("Bearer {token}"),
                },
            },
        },
    });
    serde_json::to_string_pretty(&value)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .map_err(|err| OpsError::Message(format!("failed to render .mcp.json: {err}")))
}

fn write_file(path: &Path, contents: &str) -> OpsResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            OpsError::Message(format!("failed to create {}: {err}", parent.display()))
        })?;
    }
    fs::write(path, contents)
        .map_err(|err| OpsError::Message(format!("failed to write {}: {err}", path.display())))
}

/// Append paths to `.git/info/exclude` if not already present. No-op when the
/// file is absent (e.g. a bare or freshly-initialized clone).
fn ensure_git_excluded(repo: &Path, paths: &[&str]) -> OpsResult<()> {
    let exclude_path = repo.join(".git/info/exclude");
    if !exclude_path.exists() {
        return Ok(());
    }
    let mut content = fs::read_to_string(&exclude_path)
        .map_err(|err| OpsError::Message(format!("failed to read git exclude: {err}")))?;
    let mut changed = false;
    for line in paths {
        if !content.lines().any(|existing| existing.trim() == *line) {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(line);
            content.push('\n');
            changed = true;
        }
    }
    if changed {
        fs::write(&exclude_path, content)
            .map_err(|err| OpsError::Message(format!("failed to write git exclude: {err}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn wave_fixture() -> TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wave_dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&wave_dir).expect("wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nroadmap: wave/ship\nmetrics:\n  - tests pass\n---\nDrive the ship wave.",
        )
        .expect("write goal");
        std::fs::write(wave_dir.join("MEMORY.md"), "Last loop shipped auth.")
            .expect("write memory");
        tmp
    }

    #[test]
    fn vendor_parse_accepts_claude_rejects_others() {
        assert_eq!(Vendor::parse("claude").unwrap(), Vendor::Claude);
        assert_eq!(Vendor::parse("Claude").unwrap(), Vendor::Claude);
        assert!(Vendor::parse("codex").is_err());
        assert!(Vendor::parse("gemini").is_err());
    }

    #[test]
    fn asana_mcp_json_carries_bearer_token_and_url() {
        let json = build_asana_mcp_json("tok_123").expect("render mcp json");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let asana = &value["mcpServers"]["asana"];
        assert_eq!(asana["url"], ASANA_MCP_URL);
        assert_eq!(asana["headers"]["Authorization"], "Bearer tok_123");
    }

    #[test]
    fn cloud_prompt_renders_goal_context_and_loop_instruction() {
        let tmp = wave_fixture();
        let prompt = render_cloud_prompt(tmp.path(), "ship").expect("render prompt");
        assert!(prompt.contains("Drive the ship wave."));
        assert!(prompt.contains("<lf:goal-context>"));
        assert!(prompt.contains("wave/ship"));
        assert!(prompt.contains("- tests pass"));
        assert!(prompt.contains("<lf:cloud-loop>"));
        assert!(prompt.contains("asana` MCP server"));
    }

    #[test]
    fn git_exclude_appends_paths_idempotently() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let info = tmp.path().join(".git/info");
        std::fs::create_dir_all(&info).expect("git info dir");
        std::fs::write(info.join("exclude"), "existing\n").expect("seed exclude");

        ensure_git_excluded(tmp.path(), &[".mcp.json", ".lf/cloud/"]).expect("exclude once");
        ensure_git_excluded(tmp.path(), &[".mcp.json", ".lf/cloud/"]).expect("exclude twice");

        let content = std::fs::read_to_string(info.join("exclude")).expect("read exclude");
        assert_eq!(content.matches(".mcp.json").count(), 1);
        assert_eq!(content.matches(".lf/cloud/").count(), 1);
        assert!(content.contains("existing"));
    }

    #[test]
    fn record_mode_requires_wave_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = cloud_launch(
            tmp.path(),
            &CloudLaunchOptions {
                vendor: "claude".to_string(),
                wave: Some("ghost".to_string()),
                session_url: Some("https://claude.ai/session/abc".to_string()),
            },
            &crate::ops::progress::NullProgress,
        )
        .expect_err("missing wave should fail");
        assert!(err.to_string().contains("wave directory not found"));
    }

    #[test]
    fn record_mode_persists_session_url_onto_wave() {
        let tmp = wave_fixture();
        let result = cloud_launch(
            tmp.path(),
            &CloudLaunchOptions {
                vendor: "claude".to_string(),
                wave: Some("ship".to_string()),
                session_url: Some("https://claude.ai/session/abc".to_string()),
            },
            &crate::ops::progress::NullProgress,
        )
        .expect("record session url");

        assert_eq!(result.mode, CloudMode::Record);
        let config = read_wave_config(tmp.path(), "ship").expect("config");
        assert_eq!(
            config.cloud_session_url.as_deref(),
            Some("https://claude.ai/session/abc")
        );
    }
}
