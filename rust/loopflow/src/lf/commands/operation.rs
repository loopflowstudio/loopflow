//! The reads an app serves inside its own process. Each is an `lf` command:
//! [`Operation::argv`] names it, and the command's `--watch` and
//! [`Operation::subscribe`] go through the same function, so the app pulls
//! exactly the frames `lf <argv> --json --watch` prints. A [`Query`] is a
//! one-shot read the same way: [`Query::read`] returns what `lf <argv> --json`
//! prints.
//!
//! A read the command scopes by its working directory names that directory
//! instead: `--cwd <directory>` in its argv, and the same path in-process,
//! where every subscription shares one working directory.

use std::path::PathBuf;

use clap::{Args, FromArgMatches};
use serde::{Deserialize, Serialize};

use super::activity::HistoryWindow;
use super::watch::{Subscription, Transport};
use super::waves::Shape;
use crate::lf::SessionListArgs;
use crate::ops::task::{task_changes, task_diff, task_file};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Operation {
    /// `lf history`: what Sessions did, by Work.
    History(HistoryWindow),
    /// `lf monitor ps`: live Loopflow call trees.
    MonitorPs,
    /// `lf desktop plan --all`: every repository's Waves and Tasks.
    DesktopPlan,
    /// `lf --cwd <directory> desktop plans`: the plans a repository window shows.
    DesktopPlans { directory: PathBuf },
    /// `lf --cwd <directory> desktop sessions`, with `--interactive all`
    /// when `headless` Sessions are shown too.
    DesktopSessions { directory: PathBuf, headless: bool },
    /// `lf --cwd <directory> planning status`, which exchanges the
    /// repository's planning while it runs.
    PlanningStatus { directory: PathBuf },
    /// `lf --cwd <directory> desktop task <issue>`: the Task pane.
    DesktopTask { directory: PathBuf, issue: String },
}

impl Operation {
    /// The `lf` command that prints this read, without `--json` or `--watch`.
    pub fn argv(&self) -> Vec<String> {
        let words = |words: &[&str]| words.iter().map(|word| word.to_string()).collect();
        let at = |directory: &PathBuf, command: &[&str]| {
            let mut argv = vec!["--cwd".to_owned(), directory.display().to_string()];
            argv.extend(words(command));
            argv
        };
        match self {
            Self::History(window) => window.argv(),
            Self::MonitorPs => words(&["monitor", "ps"]),
            Self::DesktopPlan => words(&["desktop", "plan", "--all"]),
            Self::DesktopPlans { directory } => at(directory, &["desktop", "plans"]),
            Self::DesktopSessions {
                directory,
                headless,
            } => {
                let mut argv = at(directory, &["desktop", "sessions"]);
                argv.extend(session_flags(*headless).iter().map(|flag| flag.to_string()));
                argv
            }
            Self::PlanningStatus { directory } => at(directory, &["planning", "status"]),
            Self::DesktopTask { directory, issue } => {
                let mut argv = at(directory, &["desktop", "task"]);
                argv.push(issue.clone());
                argv
            }
        }
    }

    /// Serve this read's watch inside this process, as `lf <argv> --json
    /// --watch` prints it. `wake` is called from any thread when a
    /// [`Subscription::pull`] may find a frame; dropping the subscription ends it.
    pub fn subscribe(
        self,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Subscription> {
        let transport = Transport::Pull(Box::new(wake));
        let subscription = match self {
            Self::History(window) => super::activity::watch(window, transport),
            Self::MonitorPs => super::top::watch_ps(transport),
            Self::DesktopPlan => {
                super::waves::watch_show(None, None, true, Shape::Desktop, transport)
            }
            Self::DesktopPlans { directory } => {
                super::desktop::watch_plans(None, directory, transport)
            }
            Self::DesktopSessions {
                directory,
                headless,
            } => {
                let args = session_args(headless)?;
                let filter = super::session::list_filter(&args, &directory)?;
                super::session::watch_list(filter, Shape::Desktop, transport)
            }
            Self::PlanningStatus { directory } => super::planning::watch_status(
                &super::planning::status_repository(&directory)?,
                transport,
            ),
            Self::DesktopTask { directory, issue } => {
                super::desktop::watch_task_pane(&directory, &issue, transport)
            }
        }?;
        Ok(subscription.expect("a pulled watch returns its subscription"))
    }
}

fn session_flags(headless: bool) -> &'static [&'static str] {
    if headless {
        &["--interactive", "all"]
    } else {
        &[]
    }
}

/// `lf desktop sessions`' arguments as its flags parse, defaults included.
fn session_args(headless: bool) -> anyhow::Result<SessionListArgs> {
    let command = SessionListArgs::augment_args(clap::Command::new("sessions"));
    let flags = std::iter::once("sessions").chain(session_flags(headless).iter().copied());
    Ok(SessionListArgs::from_arg_matches(
        &command.try_get_matches_from(flags)?,
    )?)
}

/// A one-shot read an app serves inside its own process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Query {
    /// `lf task diff <issue> --files --base <base>`: a Task's changed files.
    TaskChanges { issue: String, base: String },
    /// `lf task diff <issue> [path] --base <base>`, or with `--draft` and the
    /// draft on stdin: one Task's patch.
    TaskDiff {
        issue: String,
        path: Option<String>,
        base: String,
        draft: Option<String>,
    },
    /// `lf task file <issue> <path>`: one file of a Task's checkout.
    TaskFile { issue: String, path: String },
}

impl Query {
    /// The `lf` command that prints this read, without `--json`.
    pub fn argv(&self) -> Vec<String> {
        let words = |words: &[&str]| words.iter().map(|word| word.to_string()).collect();
        match self {
            Self::TaskChanges { issue, base } => {
                words(&["task", "diff", issue, "--files", "--base", base])
            }
            Self::TaskDiff {
                issue,
                path,
                base,
                draft,
            } => {
                let mut argv: Vec<String> = words(&["task", "diff", issue]);
                argv.extend(path.clone());
                argv.extend(["--base".into(), base.clone()]);
                if draft.is_some() {
                    argv.push("--draft".into());
                }
                argv
            }
            Self::TaskFile { issue, path } => words(&["task", "file", issue, path]),
        }
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskDiff { draft, .. } => draft.as_deref(),
            Self::TaskChanges { .. } | Self::TaskFile { .. } => None,
        }
    }

    /// What `lf <argv> --json` prints, read on the caller's thread.
    pub fn read(&self) -> anyhow::Result<String> {
        let json = match self {
            Self::TaskChanges { issue, base } => {
                serde_json::to_string_pretty(&task_changes(issue, base)?)
            }
            Self::TaskDiff {
                issue,
                path,
                base,
                draft,
            } => serde_json::to_string_pretty(&task_diff(
                issue,
                path.as_deref(),
                base,
                draft.as_deref(),
            )?),
            Self::TaskFile { issue, path } => {
                serde_json::to_string_pretty(&task_file(issue, path, false)?)
            }
        }?;
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::{Operation, Query};

    /// The JSON Desktop's `WorkOperation` and `TaskQuery` encode.
    #[test]
    fn desktop_encodings_name_their_reads() {
        let history: Operation = serde_json::from_str(
            r#"{"operation":"history","since":"7d","limit":50,"wave":null,"project":"p","task":null}"#,
        )
        .unwrap();
        assert_eq!(
            history.argv(),
            [
                "history",
                "--since",
                "7d",
                "--limit",
                "50",
                "--project",
                "p"
            ]
        );
        let sessions: Operation = serde_json::from_str(
            r#"{"operation":"desktop_sessions","directory":"/r","headless":true}"#,
        )
        .unwrap();
        assert_eq!(
            sessions.argv(),
            ["--cwd", "/r", "desktop", "sessions", "--interactive", "all"]
        );
        let diff: Query = serde_json::from_str(
            r#"{"query":"task_diff","issue":"FIX-1","path":null,"base":"parent","draft":null}"#,
        )
        .unwrap();
        assert_eq!(diff.argv(), ["task", "diff", "FIX-1", "--base", "parent"]);
    }
}
