//! The reads an app serves inside its own process. Each is an `lf` command:
//! [`Operation::argv`] names it, and the command's `--watch` and
//! [`Operation::subscribe`] go through the same function, so the app pulls
//! exactly the frames `lf <argv> --json --watch` prints.

use serde::{Deserialize, Serialize};

use super::activity::HistoryWindow;
use super::watch::{Subscription, Transport};

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
}

impl Operation {
    /// The `lf` command that prints this read, without `--json` or `--watch`.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::History(window) => window.argv(),
            Self::MonitorPs => ["monitor", "ps"].map(String::from).to_vec(),
            Self::DesktopPlan => ["desktop", "plan", "--all"].map(String::from).to_vec(),
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
                super::waves::watch_show(None, None, true, super::waves::Shape::Desktop, transport)
            }
        }?;
        Ok(subscription.expect("a pulled watch returns its subscription"))
    }
}
