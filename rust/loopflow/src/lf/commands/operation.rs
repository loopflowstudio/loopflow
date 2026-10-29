//! The reads an app serves inside its own process. Each is an `lf` command:
//! [`Operation::argv`] names it, and the command's `--watch` and
//! [`Operation::subscribe`] go through the same function, so the app pulls
//! exactly the frames `lf <argv> --json --watch` prints.
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
