//! Writes submitted by native clients to the Loopflow server. [`Write::run`]
//! calls the shared functions the CLI handlers call and returns the value the
//! CLI prints with `--json`; the carrier owns its trace.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use loopflow_execution::engine::task;
use loopflow_execution::session::SessionOpenIntent;

/// Moving a Task changes its Wave membership or its Machine ownership.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub(crate) enum TaskMoveResult {
    Wave(Box<loopflow_execution::ops::task::TaskEdit>),
    Machine(Box<loopflow_model::durable::TaskOwnership>),
}

// Each result type supplies both its response variant and its schema. Dispatch
// constructs that variant before the server serializes it, so returning a
// different Rust type is a compile error.
macro_rules! writes {
    (run($this:ident, $directory:ident, $task_repo:ident); $(
        $(#[$meta:meta])*
        $name:ident { $($(#[$field_meta:meta])* $field:ident: $input:ty),* $(,)? }
        => $output:ty;
        { $($binding:tt)* } => $run:expr
    ),* $(,)?) => {
        /// A typed mutation with the same defaults and effects as its CLI spelling.
        /// On the wire each write is a resource-named `kind` (`task.complete`);
        /// `desktop.*` writes are Desktop's own arrangement state.
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(tag = "kind", deny_unknown_fields)]
        // Exhaustive on purpose: the equivalence test matches every variant.
        pub enum Write {
            $(
                $(#[$meta])*
                #[schemars(extend("x-result" = generator.subschema_for::<$output>()))]
                $name { $($(#[$field_meta])* $field: $input),* },
            )*
        }

        /// A write's typed answer; serialization adds no wrapper or result tag.
        #[derive(Debug, Serialize)]
        #[serde(untagged)]
        pub(crate) enum WriteResult {
            $($name(Box<$output>),)*
        }

        impl Write {
            /// Run the shared operation and keep its declared result until serialization.
            pub(crate) fn run(&$this) -> anyhow::Result<WriteResult> {
                let $directory = $this.directory();
                let $task_repo = |issue: &str| loopflow_execution::ops::task::task_repository($directory, Some(issue));
                match $this {
                    $(Self::$name { $($binding)* } => {
                        let result: $output = $run;
                        Ok(WriteResult::$name(Box::new(result)))
                    },)*
                }
            }
        }
    };
}

writes! {
    run(self, directory, task_repo);
    /// File or adopt a Task, with a durable request identity; optionally start it.
    #[serde(rename = "task.create")]
    TaskCreate {
        directory: PathBuf,
        request_id: String,
        title: Option<String>,
        request: Option<String>,
        wave: Option<String>,
        graph: Option<String>,
        session: Option<String>,
        run: bool,
    } => loopflow_execution::engine::creation::TaskCreateReceipt;
    { request_id, title, request, wave, graph, session, run, .. } => loopflow_execution::engine::creation::create(
        request_id,
        loopflow_execution::engine::creation::TaskCreation {
            directory: directory.to_path_buf(),
            title: title.clone(),
            request: request.clone(),
            wave: wave.clone(),
            graph: graph.clone(),
            session: session.clone(),
            worktree: None,
            run: *run,
            agent: None,
            approve: false,
        },
    )?,

    /// `lf task complete <issue>`.
    #[serde(rename = "task.complete")]
    TaskComplete { directory: PathBuf, issue: String } => loopflow_storage::work::task::Task;
    { issue, .. } => {
        loopflow_execution::ops::task::task_complete(&task_repo(issue)?, issue, None)?
    },

    /// `lf task reopen <issue>`.
    #[serde(rename = "task.reopen")]
    TaskReopen { directory: PathBuf, issue: String } => loopflow_storage::work::task::Task;
    { issue, .. } => loopflow_execution::ops::task::task_reopen(issue, None)?,

    /// `lf task edit <issue> --notes=<notes>`.
    #[serde(rename = "task.update")]
    TaskUpdate {
        directory: PathBuf,
        issue: String,
        notes: String,
    } => loopflow_execution::ops::task::TaskEdit;
    { issue, notes, .. } => loopflow_execution::ops::task::task_edit(
        &task_repo(issue)?,
        issue,
        None,
        super::mutation::task_edit_update(&None, &Some(notes.clone()), None, &None),
    )?,

    /// `lf task abandon <issue>`: cancels the Task and removes its PRs and
    /// branches.
    #[serde(rename = "task.abandon")]
    TaskAbandon { directory: PathBuf, issue: String } => String;
    { issue, .. } => {
        loopflow_execution::ops::task::task_abandon(&task_repo(issue)?, Some(issue), false)?
    },

    /// `lf task claim <issue> --json`.
    #[serde(rename = "task.claim")]
    TaskClaim { directory: PathBuf, issue: String } => loopflow_model::durable::TaskOwnership;
    { issue, .. } => task::ownership(
        directory,
        issue,
        loopflow_execution::ops::task_ownership::OwnershipChange::Claim,
    )?,

    /// `lf task unassign <issue> --json`.
    #[serde(rename = "task.unassign")]
    TaskUnassign { directory: PathBuf, issue: String } => loopflow_model::durable::TaskOwnership;
    { issue, .. } => task::ownership(
        directory,
        issue,
        loopflow_execution::ops::task_ownership::OwnershipChange::Unassign,
    )?,

    /// `lf [-b] [-a <agent>] task run <issue> [--from <step>] --json`, started
    /// detached: the Graph process it started, or where the Task now waits.
    /// `approve` is `-b`: every step headless to `end`, approving Auto-Ship.
    #[serde(rename = "task.run")]
    TaskRun {
        directory: PathBuf,
        issue: String,
        graph: Option<String>,
        from: Option<String>,
        agent: Option<String>,
        approve: bool,
    } => task::TaskRun;
    {
        issue,
        from,
        graph,
        agent,
        approve,
        ..
    } => task::run(
        directory,
        issue,
        from.as_deref(),
        graph.as_deref(),
        agent.as_deref(),
        *approve,
    )?,

    /// `lf --task <issue> ship --json`, started detached: approves Auto-Ship.
    #[serde(rename = "task.ship")]
    TaskShip { directory: PathBuf, issue: String } => task::TaskShip;
    { issue, .. } => task::ship(directory, issue)?,

    /// `lf task hold <issue> --json`: withdraws the Task's Auto-Ship approval.
    #[serde(rename = "task.hold")]
    TaskHold { directory: PathBuf, issue: String } => task::Hold;
    { issue, .. } => task::hold(directory, issue, None)?,

    /// `lf task sync <issue> [--keep-theirs | --use-mine] [--action <id>]`:
    /// exchange the Task with its tracker now, first answering the values it
    /// lost (or the one `action` names). Answers what the command prints.
    #[serde(rename = "task.sync")]
    TaskSync {
        directory: PathBuf,
        issue: String,
        answer: Option<loopflow_execution::store::planning_sync::LostAnswer>,
        action: Option<String>,
    } => String;
    { issue, answer, action, .. } => loopflow_execution::ops::task::task_sync(
        &task_repo(issue)?,
        issue,
        *answer,
        action.as_deref(),
    )?,

    /// `lf steer <issue> <message> --json`: direction for the Task's
    /// runs, whoever writes it.
    #[serde(rename = "task.steer")]
    TaskSteer {
        directory: PathBuf,
        issue: String,
        message: String,
    } => loopflow_model::comment::TaskComments;
    { issue, message, .. } => task::steer(directory, issue, None, message)?,

    /// `lf task report <issue> <message> --json`: progress for the Task's
    /// thread; an agent's carries its Session's provenance.
    #[serde(rename = "task.report")]
    TaskReport {
        directory: PathBuf,
        issue: String,
        message: String,
    } => loopflow_model::comment::TaskComments;
    { issue, message, .. } => task::report(directory, issue, None, message)?,

    /// `lf task interrupt <issue> --json`.
    #[serde(rename = "task.interrupt")]
    TaskInterrupt { directory: PathBuf, issue: String } => loopflow_execution::ops::task::TaskControlResult;
    { issue, .. } => task::interrupt(issue)?,

    /// `lf task move <issue> --wave <wave>`, or `--no-wave` without one.
    #[serde(rename = "task.move")]
    TaskMove {
        directory: PathBuf,
        issue: String,
        wave: Option<String>,
        machine: Option<String>,
    } => TaskMoveResult;
    { issue, wave, machine, .. } => match machine {
        Some(machine) => TaskMoveResult::Machine(Box::new(loopflow_execution::ops::session_move::move_task(issue, machine, None)?)),
        None => TaskMoveResult::Wave(Box::new(loopflow_execution::ops::task::task_move_membership(
            &task_repo(issue)?, issue, wave.as_deref(),
        )?)),
    },

    /// `lf task save <issue> <path> --revision <revision> --json`, with
    /// `content` on stdin.
    #[serde(rename = "task.save")]
    TaskSave {
        directory: PathBuf,
        issue: String,
        path: String,
        revision: String,
        content: String,
    } => loopflow_execution::ops::task::TaskFileSave;
    {
        issue,
        path,
        revision,
        content,
        ..
    } => {
        let content = super::mutation::read_task_draft(&mut content.as_bytes())?;
        loopflow_execution::ops::task::task_save(issue, path, revision, &content)?
    },

    /// `lf [--wave <wave>] steer [<id>] <message> --json`: a steer
    /// one Session, every Session of the Wave or, with neither, every Session
    /// of the repository reads at its next boundary.
    #[serde(rename = "session.steer")]
    SessionSteer {
        directory: PathBuf,
        session: Option<String>,
        wave: Option<String>,
        message: String,
    } => loopflow_execution::steer::Steer;
    {
        session,
        wave,
        message,
        ..
    } => {
        use loopflow_execution::engine::session::SteerScope;
        let scope = match (session.as_deref(), wave.as_deref()) {
            (Some(id), _) => SteerScope::Session(id),
            (None, Some(wave)) => SteerScope::Wave(wave),
            (None, None) => SteerScope::Repository,
        };
        tokio::runtime::Runtime::new()?.block_on(async {
            let store = super::session::open_shared_store().await?;
            anyhow::Ok(loopflow_execution::engine::session::steer(&store, directory, scope, message).await?)
        })?
    },

    /// `lf session rename --json -- <id> <name>`.
    #[serde(rename = "session.rename")]
    SessionRename {
        directory: PathBuf,
        id: String,
        name: String,
    } => loopflow_execution::ops::human_session::SessionRecord;
    { id, name, .. } => super::session::renamed(id, name)?,

    /// `lf session observe-status --json <id> --terminal <terminal> --stream
    /// <stream> --sequence <n> [--agent-process <id>]`, `records` on stdin: one
    /// Program Status reading from the terminal showing the Session.
    #[serde(rename = "session.status")]
    SessionStatus {
        directory: PathBuf,
        id: String,
        terminal: String,
        agent_process: Option<String>,
        stream: String,
        sequence: i64,
        records: loopflow_model::program_status::Records,
    } => super::session::StatusRecorded;
    {
        id, terminal, agent_process, stream, sequence, records, ..
    } => tokio::runtime::Runtime::new()?.block_on(super::session::status_recorded(
        id,
        terminal,
        agent_process.as_deref(),
        stream,
        *sequence,
        records,
    ))?,

    /// `lf session bind --json --task <task> -- <id>`; with `preview`,
    /// `--dry-run`: the Task a bind would assign, assigning nothing.
    #[serde(rename = "session.bind")]
    SessionBind {
        directory: PathBuf,
        id: String,
        task: String,
        preview: bool,
    } => super::session::SessionBinding;
    {
        id, task, preview, ..
    } => super::session::bound(id, task, *preview)?,

    /// `lf [--agent <agent>] session resume [<id>] [--task <task>] [--wave <wave>]
    /// --prepare --json`, with `--repo <directory>` when it names none of them:
    /// the conversation and the command that opens it, no provider started.
    #[serde(rename = "session.resume")]
    SessionResume {
        directory: PathBuf,
        id: Option<String>,
        task: Option<String>,
        wave: Option<String>,
        agent: Option<String>,
        intent: SessionOpenIntent,
    } => loopflow_execution::ops::human_session::SessionRecord;
    { .. } => self.prepare_session()?,

    /// `lf [--agent <agent>] session new <id> --prepare --json`: the
    /// conversation on a fresh provider thread, not yet started.
    #[serde(rename = "session.new")]
    SessionNew {
        directory: PathBuf,
        request_id: String,
        id: String,
        agent: Option<String>,
    } => loopflow_execution::ops::human_session::SessionRecord;
    { .. } => self.prepare_session()?,

    #[serde(rename = "wave.rename")]
    WaveRename {
        directory: PathBuf,
        wave: String,
        name: String,
    } => loopflow_execution::ops::wave_lifecycle::WaveRenamed;
    { wave, name, .. } => tokio::runtime::Runtime::new()?.block_on(
        loopflow_execution::ops::wave_lifecycle::rename(directory, wave, Some(name), None, None),
    )?,

    #[serde(rename = "wave.remove")]
    WaveRemove {
        directory: PathBuf,
        wave: String,
        preview: bool,
    } => loopflow_execution::ops::wave_lifecycle::WaveRemoval;
    { wave, preview, .. } => loopflow_execution::ops::wave_lifecycle::remove(directory, wave, *preview, false)?,

    #[serde(rename = "wave.machine")]
    WaveMachine {
        directory: PathBuf,
        wave: String,
        machine: String,
    } => loopflow_execution::engine::config::Declaration;
    { wave, machine, .. } => loopflow_execution::engine::config::set(
        &loopflow_execution::config::files::Destination::PersonalWave {
            repository: directory.to_path_buf(),
            wave: wave.clone(),
        },
        "machine",
        machine,
    )?,

    /// `lf --agent <agent> session move --all --on <harness[:model]>
    /// --json`: the open Sessions the selector names continue on the agent;
    /// the report says which moved, which move after their turn, and which
    /// were skipped.
    #[serde(rename = "session.move")]
    SessionMove {
        directory: PathBuf,
        on: Option<String>,
        agent: String,
    } => loopflow_execution::ops::human_session::moves_bulk::MoveReport;
    {
        on, agent, ..
    } => super::session::moved(on.as_deref(), agent)?,

    /// `lf wave add <wave> [--objective <objective>] --json`: the Wave, its
    /// authored files and first Project; an existing Wave is unchanged.
    #[serde(rename = "wave.create")]
    WaveCreate {
        directory: PathBuf,
        wave: String,
        objective: Option<String>,
    } => loopflow_execution::ops::wave_lifecycle::WaveAdded;
    {
        wave, objective, ..
    } => {
        let repo = loopflow_execution::repo::require_repo_root(directory, "this command")?;

        tokio::runtime::Runtime::new()?.block_on(loopflow_execution::ops::wave_lifecycle::add(
            &repo,
            wave,
            objective.as_deref(),
        ))?
    },

    /// `lf wave sync <wave> --json`: the planning refresh.
    #[serde(rename = "wave.sync")]
    WaveSync { directory: PathBuf, wave: String } => Option<crate::lf::commands::waves::WaveDetailSnapshot>;
    { wave, .. } => {
        let refreshed = super::ops::refresh_status(directory, Some(wave))?;
        super::waves::desktop_status(directory, refreshed.wave().unwrap_or(wave))?
    },

    /// `lf wave plan bind <wave> <project> --json`: the tracker Project
    /// bound as the Wave's current plan.
    #[serde(rename = "wave.plan.bind")]
    WavePlanBind {
        directory: PathBuf,
        wave: String,
        project: String,
    } => loopflow_storage::work::project::Project;
    { wave, project, .. } => {
        loopflow_execution::engine::wave::planning_bind(directory, wave, project)?
    },

    /// `lf wave plan connect [<wave>|--all] [--team-key <key>]
    /// [--team-name <name>] --json`: one receipt per Wave connected.
    #[serde(rename = "wave.plan.connect")]
    WavePlanConnect {
        directory: PathBuf,
        wave: Option<String>,
        all: bool,
        team_key: Option<String>,
        team_name: Option<String>,
    } => Vec<loopflow_execution::ops::linear::LinearInitResult>;
    {
        wave,
        all,
        team_key,
        team_name,
        ..
    } => loopflow_execution::engine::wave::planning_connect(
        directory,
        wave.as_deref(),
        *all,
        team_key.as_deref(),
        team_name.as_deref(),
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf wave plan reteam [--apply] --json`: the moves onto the
    /// repository's Team; a plan without `apply`.
    #[serde(rename = "wave.plan.reteam")]
    WavePlanReteam { directory: PathBuf, apply: bool } => loopflow_execution::ops::linear::LinearReteamResult;
    { apply, .. } => {
        loopflow_execution::engine::wave::planning_reteam(directory, *apply, &loopflow_execution::engine::Diagnostics)?
    },

    /// `lf graph customize <name>`: copies a builtin Graph into the repository,
    /// recording nothing.
    #[serde(rename = "graph.customize")]
    GraphCustomize { directory: PathBuf, name: String } => super::graph::Customized;
    { name, .. } => super::graph::Customized {
        path: loopflow_execution::graph::customize(name, &loopflow_execution::repo::working_directory_in(directory)?)?,
    },

    /// `lf repo release run [<version>] [-t <target>] --json`, started
    /// detached: the Process carrying the release.
    #[serde(rename = "repo.release")]
    RepoRelease {
        directory: PathBuf,
        version: Option<String>,
        target: Option<String>,
    } => loopflow_execution::engine::launch::Launched;
    {
        version, target, ..
    } => loopflow_execution::engine::repo::release(directory, version.as_deref(), target.as_deref())?,

    /// `lf [-a <agent>] commit [-m <message>] [-p] [--no-add] [<path>…] --json`.
    #[serde(rename = "checkout.commit")]
    CheckoutCommit {
        directory: PathBuf,
        message: Option<String>,
        push: bool,
        no_add: bool,
        paths: Vec<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::checkout::Committed;
    {
        message,
        push,
        no_add,
        paths,
        agent,
        ..
    } => loopflow_execution::engine::checkout::commit(
        directory,
        message.as_deref(),
        *push,
        *no_add,
        paths,
        agent.as_deref(),
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf sync [<onto>] [--plan|--manual|--continue|--abort] [--adopt]
    /// --json`: the plan, a finished continue or abort, or the Process
    /// carrying a sync started detached.
    #[serde(rename = "checkout.sync")]
    CheckoutSync {
        directory: PathBuf,
        onto: Option<String>,
        preview: bool,
        manual: bool,
        #[serde(rename = "continue")]
        continue_sync: bool,
        abort: bool,
        adopt: bool,
    } => loopflow_execution::engine::checkout::Sync;
    {
        onto,
        preview,
        manual,
        continue_sync,
        abort,
        adopt,
        ..
    } => loopflow_execution::engine::checkout::sync(
        directory,
        &loopflow_execution::engine::checkout::SyncInput {
            onto: onto.as_deref(),
            preview: *preview,
            manual: *manual,
            continue_sync: *continue_sync,
            abort: *abort,
            adopt: *adopt,
        },
    )?,

    /// `lf wt create <name> [--plan] [--persistent] --json`.
    #[serde(rename = "checkout.create")]
    CheckoutCreate {
        directory: PathBuf,
        name: String,
        preview: bool,
        persistent: bool,
    } => loopflow_execution::engine::checkout::Checkout;
    {
        name,
        preview,
        persistent,
        ..
    } => loopflow_execution::engine::checkout::create(
        directory,
        name,
        *preview,
        *persistent,
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf account disconnect <provider>`: disconnect a service.
    #[serde(rename = "account.disconnect")]
    AccountDisconnect { provider: String } => loopflow_execution::engine::account::Account;
    { provider } => loopflow_execution::engine::account::disconnect(provider)?,

    /// `lf account connect <provider>`: connect a service.
    #[serde(rename = "account.connect")]
    AccountConnect {
        provider: String,
        chrome_profile: Option<String>,
        api_key: bool,
    } => loopflow_execution::engine::account::Account;
    { provider, chrome_profile, api_key } => loopflow_execution::engine::account::connect(provider, chrome_profile.as_deref(), *api_key)?,

    /// `lf -b [--agent <agent>] graph <name> [message]`, started detached: the
    /// Process carrying the Graph.
    #[serde(rename = "graph.run")]
    GraphRun {
        directory: PathBuf,
        name: String,
        message: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::launch::Launched;
    {
        name,
        message,
        agent,
        ..
    } => loopflow_execution::engine::graph::run(directory, name, message.as_deref(), agent.as_deref())?,

    /// `lf -b [--agent <agent>] skill <name> [message]`, started detached: the
    /// Process carrying the skill's headless run.
    #[serde(rename = "skill.run")]
    SkillRun {
        directory: PathBuf,
        name: String,
        message: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::launch::Launched;
    {
        name,
        message,
        agent,
        ..
    } => loopflow_execution::engine::skill::run(directory, name, message.as_deref(), agent.as_deref())?,

    /// `lf [--agent <agent>] pr publish [--title] [--body] --json`: push and
    /// create or refresh the checkout's ready PR.
    #[serde(rename = "pr.publish")]
    PrPublish {
        directory: PathBuf,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::pr::Published;
    {
        title, body, agent, ..
    } => loopflow_execution::engine::pr::publish(
        directory,
        title.as_deref(),
        body.as_deref(),
        agent.as_deref(),
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf [--agent <agent>] pr open [--title] [--body] --json`: push and
    /// create or refresh the checkout's draft PR; served, no review surface
    /// opens.
    #[serde(rename = "pr.open")]
    PrOpen {
        directory: PathBuf,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::pr::Published;
    {
        title, body, agent, ..
    } => loopflow_execution::engine::pr::open(
        directory,
        title.as_deref(),
        body.as_deref(),
        agent.as_deref(),
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf [--agent <agent>] pr submit [--strict] [--worktree] [--message]
    /// [--title] [--body] --json`: prepare the PR for a teammate's merge.
    #[serde(rename = "pr.submit")]
    PrSubmit {
        directory: PathBuf,
        strict: bool,
        worktree: Option<String>,
        message: Option<String>,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::pr::Prepared;
    {
        strict,
        worktree,
        message,
        title,
        body,
        agent,
        ..
    } => loopflow_execution::engine::pr::submit(
        directory,
        &loopflow_execution::engine::pr::LandInput {
            strict: *strict,
            local: false,
            wait_and_fix: false,
            worktree: worktree.clone(),
            message: message.clone(),
            title: title.clone(),
            body: body.clone(),
            agent: agent.clone(),
        },
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf [--agent <agent>] pr arm [--strict] [--local] [--worktree]
    /// [--message] [--title] [--body] --json`: arm exact-head auto-merge and
    /// return.
    #[serde(rename = "pr.arm")]
    PrArm {
        directory: PathBuf,
        strict: bool,
        local: bool,
        worktree: Option<String>,
        message: Option<String>,
        title: Option<String>,
        body: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::engine::pr::Landed;
    {
        strict,
        local,
        worktree,
        message,
        title,
        body,
        agent,
        ..
    } => loopflow_execution::engine::pr::arm(
        directory,
        &loopflow_execution::engine::pr::LandInput {
            strict: *strict,
            local: *local,
            wait_and_fix: false,
            worktree: worktree.clone(),
            message: message.clone(),
            title: title.clone(),
            body: body.clone(),
            agent: agent.clone(),
        },
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf [--agent <agent>] pr land [--wait-and-fix] [--strict] [--local]
    /// [--worktree] [--message] [--title] [--body] --json`, started
    /// detached: the Process carrying the landing.
    #[serde(rename = "pr.land")]
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
    } => loopflow_execution::engine::launch::Launched;
    {
        wait_and_fix,
        strict,
        local,
        worktree,
        message,
        title,
        body,
        agent,
        ..
    } => loopflow_execution::engine::pr::land(
        directory,
        &loopflow_execution::engine::pr::LandInput {
            strict: *strict,
            local: *local,
            wait_and_fix: *wait_and_fix,
            worktree: worktree.clone(),
            message: message.clone(),
            title: title.clone(),
            body: body.clone(),
            agent: agent.clone(),
        },
    )?,

    /// `lf pr abandon [<branch>] [--force] --json`: close the PR and delete
    /// its worktree and branches. Served, unpublished work is deleted only
    /// with `force`.
    #[serde(rename = "pr.abandon")]
    PrAbandon {
        directory: PathBuf,
        branch: Option<String>,
        force: bool,
    } => loopflow_execution::engine::pr::Abandoned;
    { branch, force, .. } => loopflow_execution::engine::pr::abandon(
        directory,
        branch.as_deref(),
        *force,
        &loopflow_execution::engine::Diagnostics,
    )?,

    /// `lf pr reconcile --json`: check recorded landings once and settle
    /// verified merges.
    #[serde(rename = "pr.reconcile")]
    PrReconcile { directory: PathBuf } => loopflow_execution::ops::pr_landing::DeliveryCheck;
    { .. } => loopflow_execution::engine::pr::reconcile(directory)?,

    /// `lf pr request-review [<pr>] -r <reviewer>… --json`.
    #[serde(rename = "pr.request_review")]
    PrRequestReview {
        directory: PathBuf,
        pr: Option<String>,
        reviewers: Vec<String>,
    } => loopflow_execution::engine::pr::ReviewRequested;
    { pr, reviewers, .. } => {
        loopflow_execution::engine::pr::request_review(directory, pr.as_deref(), reviewers)?
    },

    /// `lf desktop portfolio <change> --json`, run from `/`: which
    /// repositories the app shows.
    #[serde(rename = "desktop.portfolio.set")]
    DesktopPortfolioSet {
        change: loopflow_execution::portfolio::PortfolioCommand,
    } => loopflow_execution::portfolio::Portfolio;
    { change } => loopflow_execution::portfolio::change(change, directory)?,

    /// `lf desktop dashboard query [--revision <revision>] -- <id> <expression>`.
    #[serde(rename = "desktop.dashboard.query")]
    DesktopDashboardQuery {
        directory: PathBuf,
        id: String,
        expression: String,
        revision: Option<i64>,
    } => loopflow_storage::work::selection::Dashboard;
    {
        id,
        expression,
        revision,
        ..
    } => super::desktop::save_dashboard(
        None,
        directory,
        &crate::lf::DashboardCommand::Query {
            container: id.clone(),
            expression: expression.clone(),
            revision: *revision,
        },
        &mut std::io::empty(),
    )?,

    /// `lf desktop dashboard pin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.pin")]
    DesktopDashboardPin {
        directory: PathBuf,
        id: String,
        revision: i64,
    } => loopflow_storage::work::selection::Dashboard;
    { id, revision, .. } => super::desktop::save_dashboard(
        None,
        directory,
        &crate::lf::DashboardCommand::Pin {
            query: id.clone(),
            revision: *revision,
        },
        &mut std::io::empty(),
    )?,

    /// `lf desktop dashboard unpin <id> --revision <revision>`.
    #[serde(rename = "desktop.dashboard.unpin")]
    DesktopDashboardUnpin {
        directory: PathBuf,
        id: String,
        revision: i64,
    } => loopflow_storage::work::selection::Dashboard;
    { id, revision, .. } => super::desktop::save_dashboard(
        None,
        directory,
        &crate::lf::DashboardCommand::Unpin {
            query: id.clone(),
            revision: *revision,
        },
        &mut std::io::empty(),
    )?,

    /// `lf desktop dashboard rename --revision <revision> -- query:<id> <name>`.
    #[serde(rename = "desktop.dashboard.rename")]
    DesktopDashboardRename {
        directory: PathBuf,
        id: String,
        name: String,
        revision: i64,
    } => loopflow_storage::work::selection::Dashboard;
    {
        id, name, revision, ..
    } => super::desktop::save_dashboard(
        None,
        directory,
        &crate::lf::DashboardCommand::Rename {
            target: format!("query:{id}"),
            name: name.clone(),
            revision: u64::try_from(*revision)?,
        },
        &mut std::io::empty(),
    )?,

    /// `lf desktop dashboard hide query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.hide")]
    DesktopDashboardHide {
        directory: PathBuf,
        id: String,
        member: String,
    } => crate::lf::commands::desktop::DesktopTileRecord;
    { id, member, .. } => super::desktop::save_tile(
        None,
        directory,
        &format!("query:{id}"),
        member,
        "hide",
        &Default::default(),
    )?,

    /// `lf desktop dashboard show query:<id>/<member>`.
    #[serde(rename = "desktop.dashboard.show")]
    DesktopDashboardShow {
        directory: PathBuf,
        id: String,
        member: String,
    } => crate::lf::commands::desktop::DesktopTileRecord;
    { id, member, .. } => super::desktop::save_tile(
        None,
        directory,
        &format!("query:{id}"),
        member,
        "show",
        &Default::default(),
    )?,

    /// `lf desktop dashboard slots query:<id> --revision <revision>`, with
    /// `slots` (JSON) on stdin.
    #[serde(rename = "desktop.dashboard.slots")]
    DesktopDashboardSlots {
        directory: PathBuf,
        id: String,
        revision: i64,
        slots: String,
    } => loopflow_storage::work::selection::Dashboard;
    {
        id,
        revision,
        slots,
        ..
    } => super::desktop::save_dashboard(
        None,
        directory,
        &crate::lf::DashboardCommand::Slots {
            query: format!("query:{id}"),
            revision: *revision,
        },
        &mut slots.as_bytes(),
    )?,

    /// `lf desktop dashboard move <board>/<member> --destination <destination>`:
    /// a Dashboard's tile (`query:<id>`) or the shared Task order (`working-tasks`).
    #[serde(rename = "desktop.dashboard.move")]
    DesktopDashboardMove {
        directory: PathBuf,
        board: String,
        member: String,
        destination: String,
    } => crate::lf::commands::desktop::DesktopTileRecord;
    {
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
    )?,

    /// `lf desktop layout <work> --save`, with `layout` (JSON) on stdin.
    #[serde(rename = "desktop.layout.save")]
    DesktopLayoutSave {
        directory: PathBuf,
        work: String,
        layout: String,
    } => loopflow_storage::work::selection::WorkPaneLayout;
    { work, layout, .. } => {
        super::desktop::save_layout(None, directory, work, &mut layout.as_bytes())?
    },

    /// `lf config set <key> <value> <destination flags> --json`: declare a
    /// setting in the destination's file; `value` is YAML.
    #[serde(rename = "config.set")]
    ConfigSet {
        destination: loopflow_execution::config::files::Destination,
        key: String,
        value: String,
    } => loopflow_execution::engine::config::Declaration;
    {
        destination,
        key,
        value: declared,
    } => loopflow_execution::engine::config::set(destination, key, declared)?,

    /// `lf config unset <key> <destination flags> --json`: remove that
    /// declaration so other sources choose.
    #[serde(rename = "config.unset")]
    ConfigUnset {
        destination: loopflow_execution::config::files::Destination,
        key: String,
    } => loopflow_execution::engine::config::Declaration;
    { destination, key } => loopflow_execution::engine::config::unset(destination, key)?,

    /// Appearance's theme file at `path`: set to `value`, or read when null.
    /// Desktop owns its Apply/Cancel transaction; no CLI spells it.
    #[serde(rename = "config.theme")]
    ConfigTheme {
        path: PathBuf,
        value: Option<String>,
    } => loopflow_execution::engine::config::Declaration;
    { path, value: theme } => {
        loopflow_execution::engine::config::theme(path, theme.as_deref())?
    },
}

impl Write {
    fn prepare_session(
        &self,
    ) -> anyhow::Result<loopflow_execution::ops::human_session::SessionRecord> {
        let directory = self.directory();
        let (cli, args) = self
            .cli()
            .expect("a Session preparation has a CLI spelling")?;
        let Some(forward) = super::work_route::forward(&cli, &args, directory)? else {
            return super::session::prepared(&cli, directory);
        };
        let (mut output, mut error) = (Vec::new(), Vec::new());
        let ran = forward.run(
            &cli,
            directory,
            &mut loopflow_execution::command::CommandIo {
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
            loopflow_execution::journal::request::diagnostic(format_args!("{line}"));
        }
        Ok(serde_json::from_slice(&output)?)
    }

    /// The Session writes whose Work can live on another Machine, and whose
    /// preparation reads the CLI's selectors, as `lf` spells them. SSH runs
    /// this spelling on the destination; nothing else parses it. Task writes
    /// route through [`loopflow_execution::engine::task`].
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
                intent,
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
                words.extend(intent.flag());
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
            Self::SessionNew {
                id,
                agent,
                request_id,
                ..
            } => {
                let mut words = Vec::new();
                if let Some(agent) = agent {
                    words.extend(["--agent", agent]);
                }
                words.extend([
                    "session",
                    "new",
                    id,
                    "--prepare",
                    "--json",
                    "--request-id",
                    request_id,
                ]);
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

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskSave { content, .. } => Some(content),
            Self::DesktopDashboardSlots { slots, .. } => Some(slots),
            Self::DesktopLayoutSave { layout, .. } => Some(layout),
            _ => None,
        }
    }

    pub(crate) fn directory(&self) -> &Path {
        match self {
            Self::TaskCreate { directory, .. }
            | Self::TaskComplete { directory, .. }
            | Self::TaskReopen { directory, .. }
            | Self::TaskAbandon { directory, .. }
            | Self::TaskUpdate { directory, .. }
            | Self::TaskClaim { directory, .. }
            | Self::TaskUnassign { directory, .. }
            | Self::TaskRun { directory, .. }
            | Self::TaskShip { directory, .. }
            | Self::TaskHold { directory, .. }
            | Self::TaskSync { directory, .. }
            | Self::TaskSteer { directory, .. }
            | Self::TaskReport { directory, .. }
            | Self::TaskInterrupt { directory, .. }
            | Self::TaskMove { directory, .. }
            | Self::TaskSave { directory, .. }
            | Self::SessionSteer { directory, .. }
            | Self::SessionRename { directory, .. }
            | Self::SessionBind { directory, .. }
            | Self::SessionStatus { directory, .. }
            | Self::SessionResume { directory, .. }
            | Self::SessionNew { directory, .. }
            | Self::SessionMove { directory, .. }
            | Self::WaveCreate { directory, .. }
            | Self::WaveRename { directory, .. }
            | Self::WaveRemove { directory, .. }
            | Self::WaveMachine { directory, .. }
            | Self::WaveSync { directory, .. }
            | Self::WavePlanBind { directory, .. }
            | Self::WavePlanConnect { directory, .. }
            | Self::WavePlanReteam { directory, .. }
            | Self::GraphCustomize { directory, .. }
            | Self::RepoRelease { directory, .. }
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
            | Self::DesktopDashboardQuery { directory, .. }
            | Self::DesktopDashboardPin { directory, .. }
            | Self::DesktopDashboardUnpin { directory, .. }
            | Self::DesktopDashboardRename { directory, .. }
            | Self::DesktopDashboardHide { directory, .. }
            | Self::DesktopDashboardShow { directory, .. }
            | Self::DesktopDashboardSlots { directory, .. }
            | Self::DesktopDashboardMove { directory, .. }
            | Self::DesktopLayoutSave { directory, .. } => directory,
            Self::DesktopPortfolioSet { .. }
            | Self::AccountDisconnect { .. }
            | Self::AccountConnect { .. }
            | Self::ConfigSet { .. }
            | Self::ConfigUnset { .. }
            | Self::ConfigTheme { .. } => Path::new("/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Write;

    #[test]
    fn typed_answers_keep_the_wire_shape() {
        use super::WriteResult;
        use loopflow_execution::engine::task::TaskShip;
        use serde_json::json;

        let shipping: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../loopflow-execution/tests/fixtures/dto/server/task_shipping.json"
        ))
        .unwrap();
        for expected in [
            shipping["result"].clone(),
            json!({"state": "already_shipping", "issue": "LOO-1", "lf_process_id": shipping["result"]["lf_process_id"]}),
            json!({"state": "at_end", "issue": "LOO-1"}),
        ] {
            let result: TaskShip = serde_json::from_value(expected.clone()).unwrap();
            let actual = serde_json::to_value(WriteResult::TaskShip(Box::new(result))).unwrap();
            assert_eq!(actual, expected);
            assert_result_schema("task.ship", &actual);
        }
        for (kind, answer, expected) in [
            (
                "wave.plan.connect",
                WriteResult::WavePlanConnect(Box::default()),
                json!([]),
            ),
            (
                "task.abandon",
                WriteResult::TaskAbandon(Box::new("LOO-1".to_owned())),
                json!("LOO-1"),
            ),
        ] {
            let actual = serde_json::to_value(answer).unwrap();
            assert_eq!(actual, expected);
            assert_result_schema(kind, &actual);
        }
    }

    #[test]
    fn a_write_returns_its_declared_type() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("theme.yaml");
        let result = Write::ConfigTheme {
            path: path.clone(),
            value: Some("dark".into()),
        }
        .run()
        .unwrap();
        let super::WriteResult::ConfigTheme(declaration) = &result else {
            panic!("theme writes return their declaration");
        };
        assert_eq!(declaration.path, path);
        let actual = serde_json::to_value(result).unwrap();
        assert_eq!(actual["value"], "dark");
        assert_result_schema("config.theme", &actual);
        assert!(std::fs::read_to_string(path).unwrap().contains("dark"));
    }

    fn assert_result_schema(kind: &str, actual: &serde_json::Value) {
        let schema = crate::server::schema::describe();
        let operation = schema["$defs"]["Write"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation["properties"]["kind"]["const"] == kind)
            .unwrap();
        let mut result = operation["x-result"].clone();
        result["$defs"] = schema["$defs"].clone();
        let validator = jsonschema::draft202012::new(&result).unwrap();
        assert!(validator.is_valid(actual), "{kind}: {actual}");
    }

    /// The JSON `RegistryQueryTests.servedWriteEncodings` encodes in Swift.
    #[test]
    fn desktop_encodings_name_their_writes() {
        for json in [
            r#"{"kind":"task.move","directory":"/r","issue":"FIX-1","wave":null,"machine":null}"#,
            r#"{"kind":"desktop.dashboard.unpin","directory":"/r","id":"picked","revision":3}"#,
            r#"{"kind":"desktop.dashboard.query","directory":"/r","id":"picked","expression":"-state:done","revision":null}"#,
            r#"{"kind":"task.hold","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"desktop.portfolio.set","change":{"action":"remove","path":"/r"}}"#,
            r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null,"intent":"resume"}"#,
            r#"{"kind":"session.new","directory":"/r","request_id":"new-thread","id":"session_1","agent":"codex"}"#,
            r#"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
            r#"{"kind":"task.run","directory":"/r","issue":"FIX-1","graph":null,"from":"implement","agent":"claude","approve":false}"#,
            r#"{"kind":"task.ship","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.steer","directory":"/r","issue":"FIX-1","message":"keep the API"}"#,
            r#"{"kind":"task.report","directory":"/r","issue":"FIX-1","message":"landed"}"#,
            r#"{"kind":"task.interrupt","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"session.steer","directory":"/r","session":null,"wave":"product","message":"ship by Friday"}"#,
            r#"{"kind":"wave.plan.bind","directory":"/r","wave":"product","project":"3f1c"}"#,
            r#"{"kind":"wave.plan.connect","directory":"/r","wave":null,"all":true,"team_key":null,"team_name":null}"#,
            r#"{"kind":"wave.plan.reteam","directory":"/r","apply":false}"#,
            r#"{"kind":"repo.release","directory":"/r","version":"minor","target":null}"#,
            r#"{"kind":"checkout.commit","directory":"/r","message":"Tidy","push":false,"no_add":false,"paths":[],"agent":null}"#,
            r#"{"kind":"checkout.sync","directory":"/r","onto":null,"preview":true,"manual":false,"continue":false,"abort":false,"adopt":false}"#,
            r#"{"kind":"checkout.create","directory":"/r","name":"spike","preview":false,"persistent":false}"#,
            r#"{"kind":"account.disconnect","provider":"linear"}"#,
            r#"{"kind":"account.connect","provider":"linear","chrome_profile":"Profile 1","api_key":false}"#,
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
                r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null,"intent":"resume"}"#
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
            spelled(
                r#"{"kind":"session.new","directory":"/r","request_id":"new-thread","id":"session_1","agent":"codex"}"#
            ),
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
                "--json",
                "--request-id",
                "new-thread"
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
