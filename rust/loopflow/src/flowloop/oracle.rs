use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::ops::pm::{pm_show, PmShowOptions};
use crate::ops::{NullProgress, OpsError, OpsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Oracle {
    PrMerged,
    KrSetDone,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

impl PrState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

/// A PR the task loop is tracking. Absence (no PR yet) is `Option::None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrOracle {
    pub number: u64,
    pub url: String,
    pub state: PrState,
}

/// Poll the tracked PR by number, or the current branch's PR when none is
/// remembered yet. A `gh` failure (no PR, no auth) reads as `None`.
pub fn poll_pr_oracle(worktree: &Path, remembered_pr: Option<u64>) -> OpsResult<Option<PrOracle>> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("view");
    if let Some(number) = remembered_pr {
        cmd.arg(number.to_string());
    }
    let output = cmd
        .arg("--json")
        .arg("state,url,number")
        .current_dir(worktree)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    parse_pr_view_json(&output.stdout).map(Some)
}

#[derive(Debug, Deserialize)]
struct GhPrView {
    state: String,
    url: String,
    number: u64,
}

fn parse_pr_view_json(raw: &[u8]) -> OpsResult<PrOracle> {
    let view: GhPrView = serde_json::from_slice(raw)
        .map_err(|err| OpsError::Parse(format!("failed to parse gh pr view: {err}")))?;
    let state = match view.state.as_str() {
        "MERGED" => PrState::Merged,
        "CLOSED" => PrState::Closed,
        _ => PrState::Open,
    };
    Ok(PrOracle {
        number: view.number,
        url: view.url,
        state,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KrItem {
    pub id: String,
    pub name: String,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KrSetStatus {
    Empty,
    Open(Vec<KrItem>),
    Done(Vec<KrItem>),
}

pub fn poll_kr_set(repo: &Path, wave: &str) -> OpsResult<KrSetStatus> {
    let result = pm_show(
        repo,
        &PmShowOptions {
            wave: Some(wave.to_string()),
        },
        &NullProgress,
    )?;
    kr_set_status(result.items.into_iter().filter_map(|item| {
        item.labels
            .iter()
            .any(|label| label.eq_ignore_ascii_case("kr"))
            .then_some(KrItem {
                id: item.id,
                name: item.name,
                completed: item.completed,
            })
    }))
}

pub fn kr_set_status(items: impl IntoIterator<Item = KrItem>) -> OpsResult<KrSetStatus> {
    let items: Vec<KrItem> = items.into_iter().collect();
    if items.is_empty() {
        return Ok(KrSetStatus::Empty);
    }
    if items.iter().all(|item| item.completed) {
        return Ok(KrSetStatus::Done(items));
    }
    Ok(KrSetStatus::Open(items))
}

pub fn worktree_clean(worktree: &Path) -> OpsResult<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "git status --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output.stdout.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{kr_set_status, parse_pr_view_json, KrItem, KrSetStatus, PrOracle, PrState};

    #[test]
    fn pr_oracle_parses_merged() {
        let oracle = parse_pr_view_json(
            br#"{"number":12,"state":"MERGED","url":"https://github.test/pr/12"}"#,
        )
        .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 12,
                url: "https://github.test/pr/12".to_string(),
                state: PrState::Merged,
            }
        );
    }

    #[test]
    fn pr_oracle_parses_open() {
        let oracle =
            parse_pr_view_json(br#"{"number":7,"state":"OPEN","url":"https://github.test/pr/7"}"#)
                .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 7,
                url: "https://github.test/pr/7".to_string(),
                state: PrState::Open,
            }
        );
    }

    #[test]
    fn pr_oracle_parses_closed() {
        let oracle = parse_pr_view_json(
            br#"{"number":8,"state":"CLOSED","url":"https://github.test/pr/8"}"#,
        )
        .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 8,
                url: "https://github.test/pr/8".to_string(),
                state: PrState::Closed,
            }
        );
    }

    #[test]
    fn kr_oracle_refuses_empty_set() {
        let status = kr_set_status([]).expect("status");

        assert_eq!(status, KrSetStatus::Empty);
    }

    #[test]
    fn kr_oracle_reports_open_when_any_kr_is_open() {
        let status = kr_set_status([
            KrItem {
                id: "kr-1".to_string(),
                name: "One".to_string(),
                completed: true,
            },
            KrItem {
                id: "kr-2".to_string(),
                name: "Two".to_string(),
                completed: false,
            },
        ])
        .expect("status");

        assert!(matches!(status, KrSetStatus::Open(_)));
    }

    #[test]
    fn kr_oracle_reports_done_when_all_krs_are_done() {
        let status = kr_set_status([KrItem {
            id: "kr-1".to_string(),
            name: "One".to_string(),
            completed: true,
        }])
        .expect("status");

        assert!(matches!(status, KrSetStatus::Done(_)));
    }
}
