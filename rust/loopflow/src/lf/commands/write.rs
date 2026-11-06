//! Writes submitted by native clients to the Loopflow server. [`Write::run`]
//! calls the shared functions the CLI handlers call and returns the value the
//! CLI prints with `--json`; the carrier owns its trace.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::task;

/// A typed mutation with the same defaults and effects as its CLI spelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// On the wire each write is a resource-named `kind` (`task.complete`);
/// `desktop.*` writes are Desktop's own arrangement state.
#[serde(tag = "kind", deny_unknown_fields)]
// Exhaustive on purpose: the equivalence test matches every variant.
pub enum Write {
    /// `lf task create --session <id> --title <title> --json`, without launching.
    #[serde(rename = "task.create")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::task::Task>()))]
    TaskFromSession {
        directory: PathBuf,
        id: String,
        title: String,
    },
    /// `lf task complete <issue>`.
    #[serde(rename = "task.complete")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::task::Task>()))]
    TaskComplete { directory: PathBuf, issue: String },
    /// `lf task reopen <issue>`.
    #[serde(rename = "task.reopen")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::task::Task>()))]
    TaskReopen { directory: PathBuf, issue: String },
    /// `lf task edit <issue> --notes=<notes>`.
    #[serde(rename = "task.update")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::task::TaskEdit>()))]
    TaskNotes {
        directory: PathBuf,
        issue: String,
        notes: String,
    },
    /// `lf task abandon <issue>`: cancels the Task and removes its PRs and
    /// branches.
    #[serde(rename = "task.abandon")]
    #[schemars(extend("x-result" = generator.subschema_for::<String>()))]
    TaskAbandon { directory: PathBuf, issue: String },
    /// `lf task claim <issue> --json`.
    #[serde(rename = "task.claim")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::durable::TaskOwnership>()))]
    TaskClaim { directory: PathBuf, issue: String },
    /// `lf task unassign <issue> --json`.
    #[serde(rename = "task.unassign")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::durable::TaskOwnership>()))]
    TaskUnassign { directory: PathBuf, issue: String },
    /// `lf [-b] [-a <agent>] task run <issue> [--from <step>] --json`, started
    /// detached: the Graph process it started, or where the Task now waits.
    /// `approve` is `-b`: every step headless to `end`, approving Auto-Ship.
    #[serde(rename = "task.run")]
    #[schemars(extend("x-result" = generator.subschema_for::<task::TaskRun>()))]
    TaskRun {
        directory: PathBuf,
        issue: String,
        from: Option<String>,
        agent: Option<String>,
        approve: bool,
    },
    /// `lf --task <issue> ship --json`, started detached: approves Auto-Ship.
    #[serde(rename = "task.ship")]
    #[schemars(extend("x-result" = generator.subschema_for::<task::TaskShip>()))]
    TaskShip { directory: PathBuf, issue: String },
    /// `lf task hold <issue> --json`: withdraws the Task's Auto-Ship approval.
    #[serde(rename = "task.hold")]
    #[schemars(extend("x-result" = generator.subschema_for::<task::Hold>()))]
    TaskHold { directory: PathBuf, issue: String },
    /// `lf task steer <issue> <message> --json`: direction for the Task's
    /// runs, whoever writes it.
    #[serde(rename = "task.steer")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::task::comment::TaskComments>()))]
    TaskSteer {
        directory: PathBuf,
        issue: String,
        message: String,
    },
    /// `lf task report <issue> <message> --json`: progress for the Task's
    /// thread; an agent's carries its Session's provenance.
    #[serde(rename = "task.report")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::task::comment::TaskComments>()))]
    TaskReport {
        directory: PathBuf,
        issue: String,
        message: String,
    },
    /// `lf task interrupt <issue> --json`.
    #[serde(rename = "task.interrupt")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::task::TaskControlResult>()))]
    TaskInterrupt { directory: PathBuf, issue: String },
    /// `lf task move <issue> --wave <wave>`, or `--no-wave` without one.
    #[serde(rename = "task.move")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::task::TaskEdit>()))]
    TaskMove {
        directory: PathBuf,
        issue: String,
        wave: Option<String>,
    },
    /// `lf task save <issue> <path> --revision <revision> --json`, with
    /// `content` on stdin.
    #[serde(rename = "task.save")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::task::TaskFileSave>()))]
    TaskSave {
        directory: PathBuf,
        issue: String,
        path: String,
        revision: String,
        content: String,
    },
    /// `lf [--wave <wave>] session steer [<id>] <message> --json`: a steer
    /// one Session, every Session of the Wave or, with neither, every Session
    /// of the repository reads at its next boundary.
    #[serde(rename = "session.steer")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::steer::Steer>()))]
    SessionSteer {
        directory: PathBuf,
        session: Option<String>,
        wave: Option<String>,
        message: String,
    },
    /// `lf session rename --json -- <id> <name>`.
    #[serde(rename = "session.rename")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::human_session::SessionRecord>()))]
    SessionRename {
        directory: PathBuf,
        id: String,
        name: String,
    },
    /// `lf session bind --json --task <task> -- <id>`; with `preview`,
    /// `--dry-run`: the Task a bind would assign, assigning nothing.
    #[serde(rename = "session.bind")]
    #[schemars(extend("x-result" = (crate::server::schema::either::<crate::ops::human_session::SessionRecord, crate::ops::human_session::SessionBindingPreview>(generator))))]
    SessionBind {
        directory: PathBuf,
        id: String,
        task: String,
        preview: bool,
    },
    /// `lf [--agent <agent>] session resume [<id>] [--task <task>] [--wave <wave>]
    /// --prepare --json`, with `--repo <directory>` when it names none of them:
    /// the conversation and the command that opens it, no provider started.
    #[serde(rename = "session.resume")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::human_session::SessionRecord>()))]
    SessionResume {
        directory: PathBuf,
        id: Option<String>,
        task: Option<String>,
        wave: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] session new <id> --prepare --json`: the
    /// conversation on a fresh provider thread, not yet started.
    #[serde(rename = "session.new")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::human_session::SessionRecord>()))]
    SessionNew {
        directory: PathBuf,
        id: String,
        agent: Option<String>,
    },
    /// `lf wave add <wave> [--objective <objective>] --json`: the Wave, its
    /// authored files and first Project; an existing Wave is unchanged.
    #[serde(rename = "wave.create")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::wave_lifecycle::WaveAdded>()))]
    WaveAdd {
        directory: PathBuf,
        wave: String,
        objective: Option<String>,
    },
    /// `lf wave sync <wave> --json`: the planning refresh.
    #[serde(rename = "wave.sync")]
    #[schemars(extend("x-result" = generator.subschema_for::<Option<crate::lf::commands::waves::WaveDetailSnapshot>>()))]
    PlanningRefresh { directory: PathBuf, wave: String },
    /// `lf wave planning bind <wave> <project> --json`: the tracker Project
    /// bound as the Wave's current plan.
    #[serde(rename = "wave.planning.bind")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::project::Project>()))]
    WavePlanningBind {
        directory: PathBuf,
        wave: String,
        project: String,
    },
    /// `lf wave planning connect [<wave>|--all] [--team-key <key>]
    /// [--team-name <name>] --json`: one receipt per Wave connected.
    #[serde(rename = "wave.planning.connect")]
    #[schemars(extend("x-result" = generator.subschema_for::<Vec<crate::ops::linear::LinearInitResult>>()))]
    WavePlanningConnect {
        directory: PathBuf,
        wave: Option<String>,
        all: bool,
        team_key: Option<String>,
        team_name: Option<String>,
    },
    /// `lf wave planning reteam [--apply] --json`: the moves onto the
    /// repository's Team; a plan without `apply`.
    #[serde(rename = "wave.planning.reteam")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::linear::LinearReteamResult>()))]
    WavePlanningReteam { directory: PathBuf, apply: bool },
    /// `lf graph customize <name>`: copies a builtin Graph into the repository,
    /// recording nothing.
    #[serde(rename = "graph.customize")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::graph::Customized>()))]
    GraphCustomize { directory: PathBuf, name: String },
    /// `lf repository release run [<version>] [-t <target>] --json`, started
    /// detached: the Process carrying the release.
    #[serde(rename = "repository.release")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::launch::Launched>()))]
    RepositoryRelease {
        directory: PathBuf,
        version: Option<String>,
        target: Option<String>,
    },
    /// `lf [-a <agent>] commit [-m <message>] [-p] [--no-add] [<path>…] --json`.
    #[serde(rename = "checkout.commit")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::checkout::Committed>()))]
    CheckoutCommit {
        directory: PathBuf,
        message: Option<String>,
        push: bool,
        no_add: bool,
        paths: Vec<String>,
        agent: Option<String>,
    },
    /// `lf sync [<onto>] [--plan|--manual|--continue|--abort] [--adopt]
    /// --json`: the plan, a finished continue or abort, or the Process
    /// carrying a sync started detached.
    #[serde(rename = "checkout.sync")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::checkout::Sync>()))]
    CheckoutSync {
        directory: PathBuf,
        onto: Option<String>,
        preview: bool,
        manual: bool,
        #[serde(rename = "continue")]
        continue_sync: bool,
        abort: bool,
        adopt: bool,
    },
    /// `lf wt create <name> [--plan] [--persistent] --json`.
    #[serde(rename = "checkout.create")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::checkout::Checkout>()))]
    CheckoutCreate {
        directory: PathBuf,
        name: String,
        preview: bool,
        persistent: bool,
    },
    /// `lf account disconnect <provider> [<email>]`: forget the provider's
    /// local credentials, or one managed login's.
    #[serde(rename = "account.disconnect")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Account>()))]
    AccountDisconnect {
        provider: String,
        email: Option<String>,
    },
    /// `lf account connect <provider> [<email>] [--chrome-profile] [--import]
    /// [--api-key]`. Served, no profile is chosen interactively: a login with
    /// no remembered profile needs `chrome_profile`.
    #[serde(rename = "account.connect")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Account>()))]
    AccountConnect {
        provider: String,
        email: Option<String>,
        chrome_profile: Option<String>,
        import: bool,
        api_key: bool,
    },
    /// `lf account set <provider> [<email>] …`: a managed login's lifecycle,
    /// cooldown and browser choices, or the local credentials' browser choices.
    #[serde(rename = "account.set")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Account>()))]
    AccountSet {
        provider: String,
        email: Option<String>,
        login_email: Option<String>,
        routing: Option<String>,
        plan: Option<String>,
        clear_plan: bool,
        paid_through: Option<String>,
        clear_paid_through: bool,
        clear_cooldown: bool,
        chrome_profiles: Vec<String>,
        clear_chrome_profiles: bool,
    },
    /// `lf account token <provider> <email> [--file] [--chrome-profile]`: a
    /// long-lived Claude token for a managed login. `file` is a path on this
    /// Machine; the token itself never travels in the request.
    #[serde(rename = "account.token")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Account>()))]
    AccountToken {
        provider: String,
        email: String,
        file: Option<PathBuf>,
        chrome_profile: Option<String>,
    },
    /// `lf account redeem-reset <provider> <email> [--idempotency-key]
    /// [--credit-id] --json`: spend one banked Codex reset.
    #[serde(rename = "account.redeem_reset")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Redemption>()))]
    AccountRedeemReset {
        provider: String,
        email: String,
        idempotency_key: Option<String>,
        credit_id: Option<String>,
    },
    /// `lf account <provider> use <email>`: sign the provider's ordinary home
    /// in as a stored login.
    #[serde(rename = "account.use")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::account::Account>()))]
    AccountUse { provider: String, email: String },
    /// `lf -b [--agent <agent>] graph <name> [message]`, started detached: the
    /// Process carrying the Graph.
    #[serde(rename = "graph.run")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::launch::Launched>()))]
    GraphRun {
        directory: PathBuf,
        name: String,
        message: Option<String>,
        agent: Option<String>,
    },
    /// `lf -b [--agent <agent>] skill <name> [message]`, started detached: the
    /// Process carrying the skill's headless run.
    #[serde(rename = "skill.run")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::launch::Launched>()))]
    SkillRun {
        directory: PathBuf,
        name: String,
        message: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] pr publish [--title] [--body] --json`: push and
    /// create or refresh the checkout's ready PR.
    #[serde(rename = "pr.publish")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::Published>()))]
    PrPublish {
        directory: PathBuf,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] pr open [--title] [--body] --json`: push and
    /// create or refresh the checkout's draft PR; served, no review surface
    /// opens.
    #[serde(rename = "pr.open")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::Published>()))]
    PrOpen {
        directory: PathBuf,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] pr submit [--strict] [--worktree] [--message]
    /// [--title] [--body] --json`: prepare the PR for a teammate's merge.
    #[serde(rename = "pr.submit")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::Prepared>()))]
    PrSubmit {
        directory: PathBuf,
        strict: bool,
        worktree: Option<String>,
        message: Option<String>,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] pr arm [--strict] [--local] [--worktree]
    /// [--message] [--title] [--body] --json`: arm exact-head auto-merge and
    /// return.
    #[serde(rename = "pr.arm")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::Landed>()))]
    PrArm {
        directory: PathBuf,
        strict: bool,
        local: bool,
        worktree: Option<String>,
        message: Option<String>,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    },
    /// `lf [--agent <agent>] pr land [--wait-and-fix] [--strict] [--local]
    /// [--worktree] [--message] [--title] [--body] --json`, started
    /// detached: the Process carrying the landing.
    #[serde(rename = "pr.land")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::launch::Launched>()))]
    PrLand {
        directory: PathBuf,
        wait_and_fix: bool,
        strict: bool,
        local: bool,
        worktree: Option<String>,
        message: Option<String>,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    },
    /// `lf pr abandon [<branch>] [--force] --json`: close the PR and delete
    /// its worktree and branches. Served, unpublished work is deleted only
    /// with `force`.
    #[serde(rename = "pr.abandon")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::Abandoned>()))]
    PrAbandon {
        directory: PathBuf,
        branch: Option<String>,
        force: bool,
    },
    /// `lf pr reconcile --json`: check recorded landings once and settle
    /// verified merges.
    #[serde(rename = "pr.reconcile")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::ops::pr_landing::DeliveryCheck>()))]
    PrReconcile { directory: PathBuf },
    /// `lf pr request-review [<pr>] -r <reviewer>… --json`.
    #[serde(rename = "pr.request_review")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::pr::ReviewRequested>()))]
    PrRequestReview {
        directory: PathBuf,
        pr: Option<String>,
        reviewers: Vec<String>,
    },
    /// `lf desktop portfolio <change> --json`, run from `/`: which
    /// repositories the app shows.
    #[serde(rename = "desktop.portfolio.set")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::portfolio::Portfolio>()))]
    Portfolio {
        change: crate::portfolio::PortfolioCommand,
    },
    /// `lf desktop dashboard query [--revision <revision>] -- <id> <expression>`.
    #[serde(rename = "desktop.dashboard.query")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::Dashboard>()))]
    DashboardQuery {
        directory: PathBuf,
        id: String,
        expression: String,
        revision: Option<i64>,
    },
    /// `lf desktop dashboard pin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.pin")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::Dashboard>()))]
    DashboardPin {
        directory: PathBuf,
        id: String,
        revision: i64,
    },
    /// `lf desktop dashboard unpin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.unpin")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::Dashboard>()))]
    DashboardUnpin {
        directory: PathBuf,
        id: String,
        revision: i64,
    },
    /// `lf desktop dashboard rename --revision <revision> -- query:<id> <name>`.
    #[serde(rename = "desktop.dashboard.rename")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::Dashboard>()))]
    DashboardRename {
        directory: PathBuf,
        id: String,
        name: String,
        revision: i64,
    },
    /// `lf desktop dashboard hide query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.hide")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::desktop::DesktopTileRecord>()))]
    DashboardHide {
        directory: PathBuf,
        id: String,
        member: String,
    },
    /// `lf desktop dashboard show query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.show")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::desktop::DesktopTileRecord>()))]
    DashboardShow {
        directory: PathBuf,
        id: String,
        member: String,
    },
    /// `lf desktop dashboard slots query:<id> --revision <revision>`, with
    /// `slots` (JSON) on stdin.
    #[serde(rename = "desktop.dashboard.slots")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::Dashboard>()))]
    DashboardSlots {
        directory: PathBuf,
        id: String,
        revision: i64,
        slots: String,
    },
    /// `lf desktop dashboard move <board>/<member> --destination <destination>`:
    /// a Dashboard's tile (`query:<id>`) or the shared Task order (`working-tasks`).
    #[serde(rename = "desktop.dashboard.move")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::lf::commands::desktop::DesktopTileRecord>()))]
    TileMove {
        directory: PathBuf,
        board: String,
        member: String,
        destination: String,
    },
    /// `lf desktop layout <work> --save`, with `layout` (JSON) on stdin.
    #[serde(rename = "desktop.layout.save")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::work::selection::WorkPaneLayout>()))]
    PaneLayout {
        directory: PathBuf,
        work: String,
        layout: String,
    },
    /// `lf config set <key> <value> <destination flags> --json`: declare a
    /// setting in the destination's file; `value` is YAML.
    #[serde(rename = "config.set")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::config::Declaration>()))]
    ConfigSet {
        destination: crate::config::files::Destination,
        key: String,
        value: String,
    },
    /// `lf config unset <key> <destination flags> --json`: remove that
    /// declaration so other sources choose.
    #[serde(rename = "config.unset")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::config::Declaration>()))]
    ConfigUnset {
        destination: crate::config::files::Destination,
        key: String,
    },
    /// Appearance's theme file at `path`: set to `value`, or read when null.
    /// Desktop owns its Apply/Cancel transaction; no CLI spells it.
    #[serde(rename = "config.theme")]
    #[schemars(extend("x-result" = generator.subschema_for::<crate::engine::config::Declaration>()))]
    ConfigTheme {
        path: PathBuf,
        value: Option<String>,
    },
}

impl Write {
    /// Make this write on the caller's thread and return what it wrote: the
    /// value the CLI spelling prints with `--json`. CLI handlers call the same
    /// shared functions and print that value.
    pub fn run(&self) -> anyhow::Result<serde_json::Value> {
        if let Some(value) = self.routed()? {
            return Ok(value);
        }
        let directory = self.directory();
        let task_repo = |issue: &str| crate::ops::task::task_repository(directory, Some(issue));
        match self {
            Self::TaskFromSession { id, title, .. } => value(&crate::ops::task::task_create(
                &crate::ops::task::task_repository(directory, None)?,
                None,
                Some(title.clone()),
                None,
                crate::ops::task::TaskAdoptionOptions {
                    session: Some(id.clone()),
                    worktree: None,
                },
            )?),
            Self::TaskComplete { issue, .. } => value(&crate::ops::task::task_complete(
                &task_repo(issue)?,
                issue,
                None,
            )?),
            Self::TaskReopen { issue, .. } => value(&crate::ops::task::task_reopen(issue, None)?),
            Self::TaskNotes { issue, notes, .. } => value(&crate::ops::task::task_edit(
                &task_repo(issue)?,
                issue,
                None,
                super::mutation::task_edit_update(&None, &Some(notes.clone()), None, &None),
            )?),
            Self::TaskAbandon { issue, .. } => value(&crate::ops::task::task_abandon(
                &task_repo(issue)?,
                Some(issue),
                false,
            )?),
            Self::TaskClaim { issue, .. } => value(&task::ownership(
                directory,
                issue,
                crate::ops::task_ownership::OwnershipChange::Claim,
            )?),
            Self::TaskUnassign { issue, .. } => value(&task::ownership(
                directory,
                issue,
                crate::ops::task_ownership::OwnershipChange::Unassign,
            )?),
            Self::TaskRun {
                issue,
                from,
                agent,
                approve,
                ..
            } => value(&task::run(
                directory,
                issue,
                from.as_deref(),
                agent.as_deref(),
                *approve,
            )?),
            Self::TaskShip { issue, .. } => value(&task::ship(directory, issue)?),
            Self::TaskHold { issue, .. } => value(&task::hold(directory, issue, None)?),
            Self::TaskSteer { issue, message, .. } => {
                value(&task::steer(directory, issue, None, message)?)
            }
            Self::TaskReport { issue, message, .. } => {
                value(&task::report(directory, issue, None, message)?)
            }
            Self::TaskInterrupt { issue, .. } => value(&task::interrupt(issue)?),
            Self::TaskMove { issue, wave, .. } => value(&crate::ops::task::task_move_membership(
                &task_repo(issue)?,
                issue,
                wave.as_deref(),
            )?),
            Self::TaskSave {
                issue,
                path,
                revision,
                content,
                ..
            } => {
                let content = super::mutation::read_task_draft(&mut content.as_bytes())?;
                value(&crate::ops::task::task_save(
                    issue, path, revision, &content,
                )?)
            }
            Self::SessionSteer {
                session,
                wave,
                message,
                ..
            } => {
                use crate::engine::session::SteerScope;
                let scope = match (session.as_deref(), wave.as_deref()) {
                    (Some(id), _) => SteerScope::Session(id),
                    (None, Some(wave)) => SteerScope::Wave(wave),
                    (None, None) => SteerScope::Repository,
                };
                value(&tokio::runtime::Runtime::new()?.block_on(async {
                    let store = super::session::open_shared_store().await?;
                    anyhow::Ok(
                        crate::engine::session::steer(&store, directory, scope, message).await?,
                    )
                })?)
            }
            Self::SessionRename { id, name, .. } => value(&super::session::renamed(id, name)?),
            Self::SessionBind {
                id, task, preview, ..
            } => super::session::bound(id, task, *preview),
            Self::SessionResume { .. } | Self::SessionNew { .. } => {
                let (cli, _) = self
                    .cli()
                    .expect("a Session preparation has a CLI spelling")?;
                value(&super::session::prepared(&cli, directory)?)
            }
            Self::WaveAdd {
                wave, objective, ..
            } => {
                let repo = crate::repo::require_repo_root(directory, "this command")?;
                value(&tokio::runtime::Runtime::new()?.block_on(
                    crate::ops::wave_lifecycle::add(&repo, wave, objective.as_deref()),
                )?)
            }
            Self::PlanningRefresh { wave, .. } => {
                let refreshed = super::ops::refresh_status(directory, Some(wave))?;
                Ok(serde_json::from_str(&super::waves::status_json(
                    directory,
                    Some(refreshed.as_deref().unwrap_or(wave)),
                    false,
                )?)?)
            }
            Self::WavePlanningBind { wave, project, .. } => value(
                &crate::engine::wave::planning_bind(directory, wave, project)?,
            ),
            Self::WavePlanningConnect {
                wave,
                all,
                team_key,
                team_name,
                ..
            } => value(&crate::engine::wave::planning_connect(
                directory,
                wave.as_deref(),
                *all,
                team_key.as_deref(),
                team_name.as_deref(),
                &crate::engine::Diagnostics,
            )?),
            Self::WavePlanningReteam { apply, .. } => value(&crate::engine::wave::planning_reteam(
                directory,
                *apply,
                &crate::engine::Diagnostics,
            )?),
            Self::GraphCustomize { name, .. } => value(&super::graph::Customized {
                path: crate::graph::customize(
                    name,
                    &crate::repo::working_directory_in(directory)?,
                )?,
            }),
            Self::RepositoryRelease {
                version, target, ..
            } => value(&crate::engine::repository::release(
                directory,
                version.as_deref(),
                target.as_deref(),
            )?),
            Self::CheckoutCommit {
                message,
                push,
                no_add,
                paths,
                agent,
                ..
            } => value(&crate::engine::checkout::commit(
                directory,
                message.as_deref(),
                *push,
                *no_add,
                paths,
                agent.as_deref(),
                &crate::engine::Diagnostics,
            )?),
            Self::CheckoutSync {
                onto,
                preview,
                manual,
                continue_sync,
                abort,
                adopt,
                ..
            } => value(&crate::engine::checkout::sync(
                directory,
                &crate::engine::checkout::SyncInput {
                    onto: onto.as_deref(),
                    preview: *preview,
                    manual: *manual,
                    continue_sync: *continue_sync,
                    abort: *abort,
                    adopt: *adopt,
                },
            )?),
            Self::AccountDisconnect { provider, email } => value(
                &crate::engine::account::disconnect(provider, email.as_deref())?,
            ),
            Self::AccountConnect {
                provider,
                email,
                chrome_profile,
                import,
                api_key,
            } => value(&crate::engine::account::connect(
                provider,
                email.as_deref(),
                chrome_profile.as_deref(),
                *import,
                *api_key,
            )?),
            Self::AccountSet {
                provider,
                email,
                login_email,
                routing,
                plan,
                clear_plan,
                paid_through,
                clear_paid_through,
                clear_cooldown,
                chrome_profiles,
                clear_chrome_profiles,
            } => value(&crate::engine::account::set(
                provider,
                email.as_deref(),
                &crate::engine::account::Settings {
                    login_email: login_email.clone(),
                    routing: routing.clone(),
                    plan: plan.clone(),
                    clear_plan: *clear_plan,
                    paid_through: paid_through.clone(),
                    clear_paid_through: *clear_paid_through,
                    clear_cooldown: *clear_cooldown,
                    chrome_profiles: chrome_profiles.clone(),
                    clear_chrome_profiles: *clear_chrome_profiles,
                },
            )?),
            Self::AccountToken {
                provider,
                email,
                file,
                chrome_profile,
            } => value(&crate::engine::account::token(
                provider,
                email,
                file.as_deref(),
                chrome_profile.as_deref(),
            )?),
            Self::AccountRedeemReset {
                provider,
                email,
                idempotency_key,
                credit_id,
            } => value(&crate::engine::account::redeem_reset(
                provider,
                email,
                idempotency_key.as_deref(),
                credit_id.as_deref(),
            )?),
            Self::AccountUse { provider, email } => {
                value(&crate::engine::account::activate(provider, email)?)
            }
            Self::GraphRun {
                name,
                message,
                agent,
                ..
            } => value(&crate::engine::graph::run(
                directory,
                name,
                message.as_deref(),
                agent.as_deref(),
            )?),
            Self::SkillRun {
                name,
                message,
                agent,
                ..
            } => value(&crate::engine::skill::run(
                directory,
                name,
                message.as_deref(),
                agent.as_deref(),
            )?),
            Self::PrPublish {
                title, body, agent, ..
            } => value(&crate::engine::pr::publish(
                directory,
                title.as_deref(),
                body.as_deref(),
                agent.as_deref(),
                &crate::engine::Diagnostics,
            )?),
            Self::PrOpen {
                title, body, agent, ..
            } => value(&crate::engine::pr::open(
                directory,
                title.as_deref(),
                body.as_deref(),
                agent.as_deref(),
                &crate::engine::Diagnostics,
            )?),
            Self::PrSubmit {
                strict,
                worktree,
                message,
                title,
                body,
                agent,
                ..
            } => value(&crate::engine::pr::submit(
                directory,
                &crate::engine::pr::LandInput {
                    strict: *strict,
                    local: false,
                    wait_and_fix: false,
                    worktree: worktree.clone(),
                    message: message.clone(),
                    title: title.clone(),
                    body: body.clone(),
                    agent: agent.clone(),
                },
                &crate::engine::Diagnostics,
            )?),
            Self::PrArm {
                strict,
                local,
                worktree,
                message,
                title,
                body,
                agent,
                ..
            } => value(&crate::engine::pr::arm(
                directory,
                &crate::engine::pr::LandInput {
                    strict: *strict,
                    local: *local,
                    wait_and_fix: false,
                    worktree: worktree.clone(),
                    message: message.clone(),
                    title: title.clone(),
                    body: body.clone(),
                    agent: agent.clone(),
                },
                &crate::engine::Diagnostics,
            )?),
            Self::PrLand {
                wait_and_fix,
                strict,
                local,
                worktree,
                message,
                title,
                body,
                agent,
                ..
            } => value(&crate::engine::pr::land(
                directory,
                &crate::engine::pr::LandInput {
                    strict: *strict,
                    local: *local,
                    wait_and_fix: *wait_and_fix,
                    worktree: worktree.clone(),
                    message: message.clone(),
                    title: title.clone(),
                    body: body.clone(),
                    agent: agent.clone(),
                },
            )?),
            Self::PrAbandon { branch, force, .. } => value(&crate::engine::pr::abandon(
                directory,
                branch.as_deref(),
                *force,
                &crate::engine::Diagnostics,
            )?),
            Self::PrReconcile { .. } => value(&crate::engine::pr::reconcile(directory)?),
            Self::PrRequestReview { pr, reviewers, .. } => value(
                &crate::engine::pr::request_review(directory, pr.as_deref(), reviewers)?,
            ),
            Self::CheckoutCreate {
                name,
                preview,
                persistent,
                ..
            } => value(&crate::engine::checkout::create(
                directory,
                name,
                *preview,
                *persistent,
                &crate::engine::Diagnostics,
            )?),
            Self::Portfolio { change } => value(&crate::portfolio::change(change, directory)?),
            Self::ConfigSet {
                destination,
                key,
                value: declared,
            } => value(&crate::engine::config::set(destination, key, declared)?),
            Self::ConfigUnset { destination, key } => {
                value(&crate::engine::config::unset(destination, key)?)
            }
            Self::ConfigTheme { path, value: theme } => {
                value(&crate::engine::config::theme(path, theme.as_deref())?)
            }
            Self::DashboardQuery { .. }
            | Self::DashboardPin { .. }
            | Self::DashboardUnpin { .. }
            | Self::DashboardRename { .. }
            | Self::DashboardSlots { .. }
            | Self::PaneLayout { .. } => {
                use crate::lf::{DashboardCommand, DesktopCommand};
                let dashboard = |cmd| DesktopCommand::Dashboard { cmd };
                let command = match self {
                    Self::DashboardQuery {
                        id,
                        expression,
                        revision,
                        ..
                    } => dashboard(DashboardCommand::Query {
                        container: id.clone(),
                        expression: expression.clone(),
                        revision: *revision,
                    }),
                    Self::DashboardPin { id, revision, .. } => dashboard(DashboardCommand::Pin {
                        query: id.clone(),
                        revision: *revision,
                    }),
                    Self::DashboardUnpin { id, revision, .. } => {
                        dashboard(DashboardCommand::Unpin {
                            query: id.clone(),
                            revision: *revision,
                        })
                    }
                    Self::DashboardRename {
                        id, name, revision, ..
                    } => dashboard(DashboardCommand::Rename {
                        target: format!("query:{id}"),
                        name: name.clone(),
                        revision: u64::try_from(*revision)?,
                    }),
                    Self::DashboardSlots { id, revision, .. } => {
                        dashboard(DashboardCommand::Slots {
                            query: format!("query:{id}"),
                            revision: *revision,
                        })
                    }
                    Self::PaneLayout { work, .. } => DesktopCommand::Layout {
                        work: work.clone(),
                        save: true,
                    },
                    _ => unreachable!("matched above"),
                };
                let mut input = self.input().unwrap_or_default().as_bytes();
                super::desktop::save_arrangement(None, directory, &command, &mut input)
            }
            Self::DashboardHide { id, member, .. } | Self::DashboardShow { id, member, .. } => {
                let verb = if matches!(self, Self::DashboardHide { .. }) {
                    "hide"
                } else {
                    "show"
                };
                super::desktop::save_tile(
                    None,
                    directory,
                    &format!("query:{id}"),
                    member,
                    verb,
                    &Default::default(),
                )
            }
            Self::TileMove {
                board,
                member,
                destination,
                ..
            } => super::desktop::save_tile(
                None,
                directory,
                board,
                member,
                "move",
                &[("destination".to_owned(), destination.clone())].into(),
            ),
        }
    }

    /// The Session writes whose Work can live on another Machine, and whose
    /// preparation reads the CLI's selectors, as `lf` spells them. SSH runs
    /// this spelling on the destination; nothing else parses it. Task writes
    /// route through [`crate::engine::task`].
    fn cli(&self) -> Option<anyhow::Result<(crate::lf::Cli, Vec<String>)>> {
        let mut args = vec![
            "lf".to_owned(),
            "--cwd".to_owned(),
            self.directory().display().to_string(),
        ];
        let words: Vec<&str> = match self {
            Self::SessionResume {
                id,
                task,
                wave,
                agent,
                ..
            } => {
                let mut words = Vec::new();
                if id.is_none() && task.is_none() && wave.is_none() {
                    args.extend(["--repo".to_owned(), self.directory().display().to_string()]);
                }
                if let Some(agent) = agent {
                    words.extend(["--agent", agent]);
                }
                words.extend(["session", "resume"]);
                words.extend(id.as_deref());
                if let Some(task) = task {
                    words.extend(["--task", task]);
                }
                if let Some(wave) = wave {
                    words.extend(["--wave", wave]);
                }
                words.extend(["--prepare", "--json"]);
                words
            }
            Self::SessionNew { id, agent, .. } => {
                let mut words = Vec::new();
                if let Some(agent) = agent {
                    words.extend(["--agent", agent]);
                }
                words.extend(["session", "new", id, "--prepare", "--json"]);
                words
            }
            _ => return None,
        };
        args.extend(words.into_iter().map(str::to_owned));
        Some(
            <crate::lf::Cli as clap::Parser>::try_parse_from(&args)
                .and_then(crate::lf::Cli::checked)
                .map_err(anyhow::Error::from)
                .map(|cli| (cli, args)),
        )
    }

    /// What the destination printed, when this write's Work lives on another
    /// Machine: its `lf … --json` prints what its API returns.
    fn routed(&self) -> anyhow::Result<Option<serde_json::Value>> {
        let Some(spelled) = self.cli() else {
            return Ok(None);
        };
        let (cli, args) = spelled?;
        let Some(forward) = super::work_route::forward(&cli, &args, self.directory())? else {
            return Ok(None);
        };
        let (mut output, mut error) = (Vec::new(), Vec::new());
        let ran = forward.run(
            &cli,
            self.directory(),
            &mut super::mutation::CommandIo {
                input: &mut std::io::empty(),
                output: &mut output,
                error: &mut error,
            },
        );
        let diagnostics = String::from_utf8_lossy(&error);
        if let Err(failed) = ran {
            return Err(match diagnostics.trim() {
                "" => failed,
                said => failed.context(format!("the destination said: {said}")),
            });
        }
        for line in diagnostics.lines() {
            crate::journal::request::diagnostic(format_args!("{line}"));
        }
        Ok(Some(serde_json::from_slice(&output)?))
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskSave { content, .. } => Some(content),
            Self::DashboardSlots { slots, .. } => Some(slots),
            Self::PaneLayout { layout, .. } => Some(layout),
            _ => None,
        }
    }

    pub(crate) fn directory(&self) -> &Path {
        match self {
            Self::TaskFromSession { directory, .. }
            | Self::TaskComplete { directory, .. }
            | Self::TaskReopen { directory, .. }
            | Self::TaskAbandon { directory, .. }
            | Self::TaskNotes { directory, .. }
            | Self::TaskClaim { directory, .. }
            | Self::TaskUnassign { directory, .. }
            | Self::TaskRun { directory, .. }
            | Self::TaskShip { directory, .. }
            | Self::TaskHold { directory, .. }
            | Self::TaskSteer { directory, .. }
            | Self::TaskReport { directory, .. }
            | Self::TaskInterrupt { directory, .. }
            | Self::TaskMove { directory, .. }
            | Self::TaskSave { directory, .. }
            | Self::SessionSteer { directory, .. }
            | Self::SessionRename { directory, .. }
            | Self::SessionBind { directory, .. }
            | Self::SessionResume { directory, .. }
            | Self::SessionNew { directory, .. }
            | Self::WaveAdd { directory, .. }
            | Self::PlanningRefresh { directory, .. }
            | Self::WavePlanningBind { directory, .. }
            | Self::WavePlanningConnect { directory, .. }
            | Self::WavePlanningReteam { directory, .. }
            | Self::GraphCustomize { directory, .. }
            | Self::RepositoryRelease { directory, .. }
            | Self::CheckoutCommit { directory, .. }
            | Self::CheckoutSync { directory, .. }
            | Self::CheckoutCreate { directory, .. }
            | Self::GraphRun { directory, .. }
            | Self::SkillRun { directory, .. }
            | Self::PrPublish { directory, .. }
            | Self::PrOpen { directory, .. }
            | Self::PrSubmit { directory, .. }
            | Self::PrArm { directory, .. }
            | Self::PrLand { directory, .. }
            | Self::PrAbandon { directory, .. }
            | Self::PrReconcile { directory }
            | Self::PrRequestReview { directory, .. }
            | Self::DashboardQuery { directory, .. }
            | Self::DashboardPin { directory, .. }
            | Self::DashboardUnpin { directory, .. }
            | Self::DashboardRename { directory, .. }
            | Self::DashboardHide { directory, .. }
            | Self::DashboardShow { directory, .. }
            | Self::DashboardSlots { directory, .. }
            | Self::TileMove { directory, .. }
            | Self::PaneLayout { directory, .. } => directory,
            Self::Portfolio { .. }
            | Self::AccountDisconnect { .. }
            | Self::AccountConnect { .. }
            | Self::AccountSet { .. }
            | Self::AccountToken { .. }
            | Self::AccountRedeemReset { .. }
            | Self::AccountUse { .. }
            | Self::ConfigSet { .. }
            | Self::ConfigUnset { .. }
            | Self::ConfigTheme { .. } => Path::new("/"),
        }
    }
}

fn value(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::Write;

    /// The JSON `RegistryQueryTests.servedWriteEncodings` encodes in Swift.
    #[test]
    fn desktop_encodings_name_their_writes() {
        for json in [
            r#"{"kind":"task.move","directory":"/r","issue":"FIX-1","wave":null}"#,
            r#"{"kind":"desktop.dashboard.unpin","directory":"/r","id":"picked","revision":3}"#,
            r#"{"kind":"desktop.dashboard.query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#,
            r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"desktop.portfolio.set","change":{"action":"remove","path":"/r"}}"#,
            r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#,
            r#"{"kind":"session.new","directory":"/r","id":"session_1","agent":"codex"}"#,
            r#"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
            r#"{"kind":"task.run","directory":"/r","issue":"FIX-1","from":"implement","agent":"claude","approve":false}"#,
            r#"{"kind":"task.ship","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.steer","directory":"/r","issue":"FIX-1","message":"keep the API"}"#,
            r#"{"kind":"task.report","directory":"/r","issue":"FIX-1","message":"landed"}"#,
            r#"{"kind":"task.interrupt","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"session.steer","directory":"/r","session":null,"wave":"product","message":"ship by Friday"}"#,
            r#"{"kind":"wave.planning.bind","directory":"/r","wave":"product","project":"3f1c"}"#,
            r#"{"kind":"wave.planning.connect","directory":"/r","wave":null,"all":true,"team_key":null,"team_name":null}"#,
            r#"{"kind":"wave.planning.reteam","directory":"/r","apply":false}"#,
            r#"{"kind":"repository.release","directory":"/r","version":"minor","target":null}"#,
            r#"{"kind":"checkout.commit","directory":"/r","message":"Tidy","push":false,"no_add":false,"paths":[],"agent":null}"#,
            r#"{"kind":"checkout.sync","directory":"/r","onto":null,"preview":true,"manual":false,"continue":false,"abort":false,"adopt":false}"#,
            r#"{"kind":"checkout.create","directory":"/r","name":"spike","preview":false,"persistent":false}"#,
            r#"{"kind":"account.disconnect","provider":"claude","email":null}"#,
            r#"{"kind":"account.connect","provider":"codex","email":"maya@example.com","chrome_profile":"Profile 1","import":false,"api_key":false}"#,
            r#"{"kind":"account.set","provider":"codex","email":"maya@example.com","login_email":null,"routing":"active","plan":null,"clear_plan":false,"paid_through":null,"clear_paid_through":false,"clear_cooldown":true,"chrome_profiles":[],"clear_chrome_profiles":false}"#,
            r#"{"kind":"account.token","provider":"claude","email":"maya@example.com","file":"/tmp/token","chrome_profile":null}"#,
            r#"{"kind":"account.redeem_reset","provider":"codex","email":"maya@example.com","idempotency_key":null,"credit_id":null}"#,
            r#"{"kind":"account.use","provider":"claude","email":"maya@example.com"}"#,
            r#"{"kind":"graph.run","directory":"/r","name":"code","message":null,"agent":null}"#,
            r#"{"kind":"skill.run","directory":"/r","name":"debug","message":"the failing test","agent":"claude"}"#,
            r#"{"kind":"pr.publish","directory":"/r","title":null,"body":null,"agent":null}"#,
            r#"{"kind":"pr.open","directory":"/r","title":"Spike","body":null,"agent":"claude"}"#,
            r#"{"kind":"pr.submit","directory":"/r","strict":false,"worktree":null,"message":null,"title":null,"body":null,"agent":null}"#,
            r#"{"kind":"pr.arm","directory":"/r","strict":true,"local":false,"worktree":null,"message":null,"title":null,"body":null,"agent":null}"#,
            r#"{"kind":"pr.land","directory":"/r","wait_and_fix":true,"strict":false,"local":false,"worktree":null,"message":null,"title":null,"body":null,"agent":null}"#,
            r#"{"kind":"pr.abandon","directory":"/r","branch":"spike","force":true}"#,
            r#"{"kind":"pr.reconcile","directory":"/r"}"#,
            r#"{"kind":"pr.request_review","directory":"/r","pr":null,"reviewers":["alice"]}"#,
        ] {
            let write: Write = serde_json::from_str(json).unwrap();
            assert_eq!(
                serde_json::to_value(&write).unwrap(),
                serde_json::from_str::<serde_json::Value>(json).unwrap()
            );
        }
    }

    #[test]
    fn routed_writes_spell_the_command_ssh_runs() {
        let spelled = |json: &str| {
            let (_, args) = serde_json::from_str::<Write>(json)
                .unwrap()
                .cli()
                .expect("a routed write")
                .unwrap();
            args
        };
        assert_eq!(
            spelled(
                r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#
            ),
            [
                "lf",
                "--cwd",
                "/r",
                "--repo",
                "/r",
                "session",
                "resume",
                "--prepare",
                "--json"
            ]
        );
        assert_eq!(
            spelled(r#"{"kind":"session.new","directory":"/r","id":"session_1","agent":"codex"}"#),
            [
                "lf",
                "--cwd",
                "/r",
                "--agent",
                "codex",
                "session",
                "new",
                "session_1",
                "--prepare",
                "--json"
            ]
        );
        for json in [
            r#"{"kind":"task.complete","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.claim","directory":"/r","issue":"FIX-1"}"#,
        ] {
            assert!(serde_json::from_str::<Write>(json).unwrap().cli().is_none());
        }
    }
}
