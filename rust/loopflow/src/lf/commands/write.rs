//! Writes submitted by native clients to the Loopflow server. [`Write::run`]
//! calls the shared functions the CLI handlers call and returns the value the
//! CLI prints with `--json`; the carrier owns its trace. A write for a Task or
//! Session another Machine runs goes to that Machine's server as this same
//! value, over SSH, with the [`Sender`] it runs with there.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use loopflow_execution::engine::task;
use loopflow_execution::machine::wire::{Secrets, Selection, Sender};

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
    (run($this:ident, $directory:ident, $task_repo:ident, $remote:ident, $environment:ident, $origin:ident); $(
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
            /// Run a write another Machine sent here, never sending it on: in
            /// this Machine's checkout of the sender's repository, a launch
            /// it starts running with the sender's selection and secrets.
            pub(crate) fn run_here(mut self, sender: &Sender) -> anyhow::Result<WriteResult> {
                if let Some(directory) = self.directory_mut() {
                    *directory = super::work_route::repository_path(&sender.repository)?;
                }
                if let Self::TaskSave { issue, .. }
                | Self::SessionResume { task: Some(issue), .. }
                | Self::SessionNew { task: Some(issue), .. } = &mut self
                {
                    *issue = super::work_route::peer_task_id(&sender.repository, issue)?;
                }
                self.execute(None, Some(sender))
            }

            /// `remote` is the Machine a Task or Session write goes to instead.
            fn execute(&$this, $remote: Option<Remote>, $origin: Option<&Sender>) -> anyhow::Result<WriteResult> {
                let environment = $origin.map(|sender| sender.selection.environment(&sender.secrets)).transpose()?.unwrap_or_default();
                let $environment = environment.as_slice();
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
    run(self, directory, task_repo, remote, environment, origin);
    /// File or adopt a Task, with a durable request identity; optionally start it.
    #[serde(rename = "task.create")]
    TaskCreate {
        directory: PathBuf,
        request_id: String,
        title: Option<String>,
        request: Option<String>,
        wave: Option<String>,
        /// The Task this one is filed from, by ID or identifier; `null` files top-level.
        parent: Option<String>,
        graph: Option<String>,
        session: Option<String>,
        run: bool,
    } => super::task_create::TaskCreated;
    { request_id, title, request, wave, parent, graph, session, run, .. } => {
    // Served, planning exchange starts once the answer is written.
    let created = loopflow_execution::engine::creation::create(
        request_id,
        loopflow_execution::engine::creation::TaskCreation {
            directory: directory.to_path_buf(),
            title: title.clone(),
            request: request.clone(),
            wave: wave.clone(),
            parent: parent.clone(),
            graph: graph.clone(),
            session: session.clone(),
            worktree: None,
            run: *run,
            agent: None,
            approve: false,
        },
    )?;
    super::task_create::answer(request_id, created.receipt, created.session)?
    },

    /// `lf task complete <issue>`.
    #[serde(rename = "task.complete")]
    TaskComplete { directory: PathBuf, issue: String } => super::waves::TaskCompleted;
    { issue, .. } => super::waves::task_completed(
        loopflow_execution::ops::task::task_complete(&task_repo(issue)?, issue, None)?,
    )?,

    /// `lf task reopen <issue>`.
    #[serde(rename = "task.reopen")]
    TaskReopen { directory: PathBuf, issue: String } => loopflow_storage::work::task::Task;
    { issue, .. } => loopflow_execution::ops::task::task_reopen(issue, None)?,

    /// `lf task edit <issue> [--notes=<notes>] [--rank=<rank>]`: a new brief,
    /// a new position in its Project, or both.
    #[serde(rename = "task.update")]
    TaskUpdate {
        directory: PathBuf,
        issue: String,
        notes: Option<String>,
        rank: Option<u32>,
    } => loopflow_execution::ops::task::TaskEdit;
    { issue, notes, rank, .. } => loopflow_execution::ops::task::task_edit(
        &task_repo(issue)?,
        issue,
        None,
        super::mutation::task_edit_update(&None, notes, *rank, &None),
        None,
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
    { issue, .. } => match remote {
        Some(remote) => remote.call()?,
        None => task::ownership(
            directory,
            issue,
            loopflow_execution::ops::task_ownership::OwnershipChange::Claim,
        )?,
    },

    /// `lf task unassign <issue> --json`.
    #[serde(rename = "task.unassign")]
    TaskUnassign { directory: PathBuf, issue: String } => loopflow_model::durable::TaskOwnership;
    { issue, .. } => match remote {
        Some(remote) => remote.call()?,
        None => task::ownership(
            directory,
            issue,
            loopflow_execution::ops::task_ownership::OwnershipChange::Unassign,
        )?,
    },

    /// `lf [-b] [-a <agent>] task run <issue> [--from <step>] --json`, started
    /// detached: the Graph process it started, or where the Task now waits.
    /// `approve` is `-b`: every step headless to `end`; it writes no auto.
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
    } => match remote {
        Some(remote) => remote.call()?,
        None => task::run(
            directory,
            issue,
            from.as_deref(),
            graph.as_deref(),
            agent.as_deref(),
            *approve,
            environment,
        )?,
    },

    /// `lf task auto on <issue> --json`, started detached: turns auto on and returns the run, as
    /// `task.run` does. Nothing else turns auto on.
    #[serde(rename = "task.auto.on")]
    TaskAutoOn { directory: PathBuf, issue: String } => task::TaskRun;
    { issue, .. } => match remote {
        Some(remote) => remote.call()?,
        None => task::auto_on(directory, issue, environment)?,
    },

    /// `lf task auto off <issue> --json`: turns the Task's auto setting off.
    #[serde(rename = "task.auto.off")]
    TaskAutoOff {
        directory: PathBuf,
        issue: String,
        /// Why, saved as a comment on the Task.
        #[serde(deserialize_with = "Option::deserialize")]
        reason: Option<String>,
    } => task::AutoOff;
    { issue, reason, .. } => match remote {
        Some(remote) => remote.call()?,
        None => task::auto_off(issue, reason.as_deref())?,
    },

    /// `lf task sync <issue> [--keep-theirs | --use-mine]`: exchange the
    /// Task with its tracker now, first answering every value it lost.
    /// Answers what the command prints.
    #[serde(rename = "task.sync")]
    TaskSync {
        directory: PathBuf,
        issue: String,
        answer: Option<loopflow_execution::store::planning_sync::LostAnswer>,
    } => String;
    { issue, answer, .. } => loopflow_execution::ops::task::task_sync(
        &task_repo(issue)?,
        issue,
        *answer,
    )?,

    /// `lf repo tracker answer <action> --keep-theirs | --use-mine`: answer one
    /// lost line of the directory's repository, whoever owns it. Answers the
    /// line as it stood.
    #[serde(rename = "repo.tracker.answer")]
    TrackerAnswer {
        directory: PathBuf,
        action: String,
        answer: loopflow_execution::store::planning_sync::LostAnswer,
    } => loopflow_execution::store::planning_sync::TrackerActionStatus;
    { directory, action, answer } => loopflow_execution::ops::planning_sync::answer_lost(
        directory,
        action,
        *answer,
    )?,

    /// `lf steer <issue> <message> --json`: direction for the Task's
    /// runs, whoever writes it.
    #[serde(rename = "task.steer")]
    TaskSteer {
        directory: PathBuf,
        issue: String,
        message: String,
    } => task::TaskSteerReceipt;
    { issue, message, .. } => steer_task(&task_repo(issue)?, issue, None, message, origin)?,

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
        Some(machine) => TaskMoveResult::Machine(Box::new(loopflow_execution::ops::session_move::move_task(issue, machine, None, run_task)?)),
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
    } => loopflow_execution::ops::human_session::SessionRecord;
    { .. } => self.prepare_session(remote)?,

    /// `lf [--agent <agent>] session new <id> --prepare --json`: the
    /// conversation on a fresh provider thread, not yet started.
    #[serde(rename = "session.new")]
    SessionNew {
        directory: PathBuf,
        request_id: String,
        id: String,
        /// The Task whose conversation this is; it is prepared where that Task runs.
        #[serde(deserialize_with = "Option::deserialize")]
        task: Option<String>,
        agent: Option<String>,
    } => loopflow_execution::ops::human_session::SessionRecord;
    { .. } => self.prepare_session(remote)?,

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
    /// bound as the Wave's current Project.
    #[serde(rename = "wave.plan.bind")]
    WavePlanBind {
        replace: bool,
        directory: PathBuf,
        wave: String,
        project: String,
    } => loopflow_storage::work::project::Project;
    { wave, project, replace, .. } => {
        loopflow_execution::engine::wave::planning_bind(directory, wave, project, *replace)?
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

    /// `lf [-a <agent>] commit [-m <message>] [--no-push] [--no-add] [<path>…] --json`.
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

    /// `lf wt create <name> [--plan] [--persistent] [--stack] --json`.
    #[serde(rename = "checkout.create")]
    CheckoutCreate {
        directory: PathBuf,
        name: String,
        preview: bool,
        persistent: bool,
        stack: bool,
    } => loopflow_execution::engine::checkout::Checkout;
    {
        name,
        preview,
        persistent,
        stack,
        ..
    } => loopflow_execution::engine::checkout::create(
        directory,
        name,
        *preview,
        *persistent,
        *stack,
        &loopflow_execution::engine::Diagnostics,
    )?,

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

    /// Change a catalog setting if its authored declarations still match.
    #[serde(rename = "config.edit")]
    ConfigEdit {
        destination: loopflow_execution::config::files::Destination,
        key: String,
        expected: Vec<loopflow_execution::config::files::edit::Entry>,
        change: loopflow_execution::config::files::edit::Change,
    } => loopflow_execution::config::files::edit::Applied;
    { destination, key, expected, change } => loopflow_execution::engine::config::checked_edit(destination, key, expected, change)?,

    /// Preview a checked edit against the current configuration file.
    #[serde(rename = "config.preview")]
    ConfigPreview {
        destination: loopflow_execution::config::files::Destination,
        key: String,
        expected: Vec<loopflow_execution::config::files::edit::Entry>,
        change: loopflow_execution::config::files::edit::Change,
    } => String;
    { destination, key, expected, change } => loopflow_execution::engine::config::preview_edit(destination, key, expected, change)?,

    /// Preview, confirm or undo a move to the next broader configuration file.
    #[serde(rename = "config.promote")]
    ConfigPromote {
        destination: loopflow_execution::config::files::Destination,
        key: String,
        #[serde(deserialize_with = "Option::deserialize")]
        wave: Option<String>,
        action: loopflow_execution::engine::config::promotion::Action,
    } => loopflow_execution::engine::config::promotion::Outcome;
    { destination, key, wave, action } => loopflow_execution::engine::config::promotion::run(destination,key,wave.as_deref(),action)?,

    /// The Machine's `theme:` choice: set to `value`, or read when null.
    /// Desktop owns its Apply/Cancel transaction and the phone reads it; no
    /// CLI spells it.
    #[serde(rename = "config.theme")]
    ConfigTheme {
        value: Option<String>,
    } => loopflow_execution::engine::config::ThemeDeclaration;
    { value: theme } => loopflow_execution::engine::config::theme(theme.as_deref())?,
}

/// The directory a write runs in, borrowed as `$write` is: none for a
/// Machine-wide write.
macro_rules! directory_of {
    ($write:expr) => {
        match $write {
            Write::TaskCreate { directory, .. }
            | Write::TaskComplete { directory, .. }
            | Write::TaskReopen { directory, .. }
            | Write::TaskAbandon { directory, .. }
            | Write::TaskUpdate { directory, .. }
            | Write::TaskClaim { directory, .. }
            | Write::TaskUnassign { directory, .. }
            | Write::TaskRun { directory, .. }
            | Write::TaskAutoOn { directory, .. }
            | Write::TaskAutoOff { directory, .. }
            | Write::TaskSync { directory, .. }
            | Write::TrackerAnswer { directory, .. }
            | Write::TaskSteer { directory, .. }
            | Write::TaskReport { directory, .. }
            | Write::TaskInterrupt { directory, .. }
            | Write::TaskMove { directory, .. }
            | Write::TaskSave { directory, .. }
            | Write::SessionSteer { directory, .. }
            | Write::SessionRename { directory, .. }
            | Write::SessionBind { directory, .. }
            | Write::SessionStatus { directory, .. }
            | Write::SessionResume { directory, .. }
            | Write::SessionNew { directory, .. }
            | Write::SessionMove { directory, .. }
            | Write::WaveCreate { directory, .. }
            | Write::WaveRename { directory, .. }
            | Write::WaveRemove { directory, .. }
            | Write::WaveMachine { directory, .. }
            | Write::WaveSync { directory, .. }
            | Write::WavePlanBind { directory, .. }
            | Write::WavePlanConnect { directory, .. }
            | Write::WavePlanReteam { directory, .. }
            | Write::GraphCustomize { directory, .. }
            | Write::RepoRelease { directory, .. }
            | Write::CheckoutCommit { directory, .. }
            | Write::CheckoutSync { directory, .. }
            | Write::CheckoutCreate { directory, .. }
            | Write::GraphRun { directory, .. }
            | Write::SkillRun { directory, .. }
            | Write::PrPublish { directory, .. }
            | Write::PrOpen { directory, .. }
            | Write::PrSubmit { directory, .. }
            | Write::PrArm { directory, .. }
            | Write::PrLand { directory, .. }
            | Write::PrAbandon { directory, .. }
            | Write::PrReconcile { directory }
            | Write::PrRequestReview { directory, .. }
            | Write::DesktopDashboardQuery { directory, .. }
            | Write::DesktopDashboardPin { directory, .. }
            | Write::DesktopDashboardUnpin { directory, .. }
            | Write::DesktopDashboardRename { directory, .. }
            | Write::DesktopDashboardHide { directory, .. }
            | Write::DesktopDashboardShow { directory, .. }
            | Write::DesktopDashboardSlots { directory, .. }
            | Write::DesktopDashboardMove { directory, .. } => Some(directory),
            Write::DesktopPortfolioSet { .. }
            | Write::ConfigSet { .. }
            | Write::ConfigPromote { .. }
            | Write::ConfigPreview { .. }
            | Write::ConfigEdit { .. }
            | Write::ConfigUnset { .. }
            | Write::ConfigTheme { .. } => None,
        }
    };
}

impl Write {
    pub(crate) fn run(&self) -> anyhow::Result<WriteResult> {
        self.run_with(Remote::request)
    }

    pub(crate) fn run_with(
        &self,
        save: impl FnOnce(
            &loopflow_model::durable::Machine,
            Sender,
            &Write,
        ) -> anyhow::Result<loopflow_execution::machine::client::Response>,
    ) -> anyhow::Result<WriteResult> {
        let remote = self.remote()?;
        match (self, remote) {
            (Self::TaskSave { .. }, Some(remote)) => {
                let answer = save(&remote.machine, remote.sender, &remote.change)?;
                Ok(WriteResult::TaskSave(Box::new(serde_json::from_value(
                    answer.result,
                )?)))
            }
            (_, remote) => self.execute(remote, None),
        }
    }

    /// The Machine a Task or Session write goes to when its Work runs
    /// elsewhere, with the write naming that Work exactly. `None` runs it here.
    /// A steer handles its carrier fallback inside `steer_task` instead.
    fn remote(&self) -> anyhow::Result<Option<Remote>> {
        use loopflow_execution::machine::task::{remote_ownership_writer, remote_task};
        let (route, launching) = match self {
            Self::SessionResume { .. } | Self::SessionNew { .. } => return self.remote_session(),
            Self::TaskRun { issue, .. } | Self::TaskAutoOn { issue, .. } => {
                (remote_task(issue, None)?, true)
            }
            Self::TaskSave {
                issue, directory, ..
            } => (remote_task(issue, Some(directory))?, false),
            Self::TaskAutoOff { issue, .. } => (remote_task(issue, None)?, false),
            Self::TaskClaim { issue, .. } | Self::TaskUnassign { issue, .. } => {
                (remote_ownership_writer(issue, None)?, false)
            }
            _ => return Ok(None),
        };
        let Some(route) = route else {
            return Ok(None);
        };
        // The Task's Linear mapping resolves on the destination, whose own
        // Task ID may differ.
        let source = match self {
            Self::TaskAutoOn { .. } => tokio::runtime::Runtime::new()?.block_on(
                loopflow_execution::machine::task::resolve_task_source(
                    Some(route.task_id.as_str()),
                    self.directory(),
                ),
            )?,
            _ => None,
        };
        let mut write = self.clone();
        if let Self::TaskSave { issue, .. } = &mut write {
            *issue = route.peer_selector;
        } else if let Self::TaskRun { issue, .. }
        | Self::TaskAutoOn { issue, .. }
        | Self::TaskAutoOff { issue, .. }
        | Self::TaskClaim { issue, .. }
        | Self::TaskUnassign { issue, .. } = &mut write
        {
            *issue = source
                .as_ref()
                .and_then(|source| source.issue_id.clone())
                .unwrap_or_else(|| route.task_id.to_string());
        }
        let machine = find_machine(route.machine_id.as_str())?;
        Remote::new(machine, route.repository_id, write, launching, source).map(Some)
    }

    /// A conversation that lives on another Machine, prepared there. A named
    /// Task decides the Machine, wherever its Session last moved; otherwise
    /// `lf` routes the spelling as it would from a terminal.
    fn remote_session(&self) -> anyhow::Result<Option<Remote>> {
        let (directory, task) = match self {
            Self::SessionResume {
                directory, task, ..
            }
            | Self::SessionNew {
                directory, task, ..
            } => (directory, task),
            _ => return Ok(None),
        };
        let (machine, route, wave, repository) = match task {
            Some(task) => {
                let Some(route) =
                    loopflow_execution::machine::task::remote_task(task, Some(directory))?
                else {
                    return Ok(None);
                };
                (route.machine_id.clone(), Some(route), None, None)
            }
            None => {
                let (cli, args) = self.session_spelling()?;
                let Some(forward) = super::work_route::forward(&cli, &args, directory)? else {
                    return Ok(None);
                };
                (
                    forward.machine,
                    forward.route,
                    forward.wave,
                    forward.repository,
                )
            }
        };
        let machine = find_machine(machine.as_str())?;
        let repository = match repository.or(route.as_ref().map(|route| route.repository_id.clone())) {
            Some(repository) => repository,
            None => loopflow_execution::machine::task::current_repository(directory)?
                .ok_or_else(|| {
                    anyhow!(
                        "this Session runs on Machine {}; run `lf repo` here so it can address the repository",
                        machine.id
                    )
                })?,
        };
        let mut write = self.clone();
        if let (Self::SessionResume { task, .. } | Self::SessionNew { task, .. }, Some(route)) =
            (&mut write, &route)
        {
            *task = Some(route.peer_selector.clone());
        }
        if let Self::SessionResume { wave: resumed, .. } = &mut write {
            *resumed = resumed.take().or(wave);
        }
        let mut remote = Remote::new(machine, repository, write, false, None)?;
        remote.route = route;
        Ok(Some(remote))
    }

    /// `session.resume` / `session.new`: the conversation prepared where it
    /// lives, no provider started.
    fn prepare_session(
        &self,
        remote: Option<Remote>,
    ) -> anyhow::Result<loopflow_execution::ops::human_session::SessionRecord> {
        let Some(remote) = remote else {
            let (cli, _) = self.session_spelling()?;
            return super::session::prepared(&cli, self.directory());
        };
        if let Self::SessionResume { id: Some(id), .. } = self {
            // A Session resumes on the Machine it moved to, carried there first.
            loopflow_execution::ops::session_move::move_session(
                id,
                remote.machine.id.as_str(),
                run_task,
            )?;
        }
        super::ssh::prepared_remotely(remote.call()?, &remote.machine, remote.route.as_ref())
    }

    /// The Session preparation as `lf` spells it. Its local preparation still
    /// reads Clap's Session arguments; nothing sends this spelling.
    fn session_spelling(&self) -> anyhow::Result<(crate::lf::Cli, Vec<String>)> {
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
            _ => anyhow::bail!("not a Session preparation"),
        };
        args.extend(words.into_iter().map(str::to_owned));
        let cli = <crate::lf::Cli as clap::Parser>::try_parse_from(&args)
            .and_then(crate::lf::Cli::checked)?;
        Ok((cli, args))
    }

    /// What the command reads from stdin.
    pub fn input(&self) -> Option<&str> {
        match self {
            Self::TaskSave { content, .. } => Some(content),
            Self::DesktopDashboardSlots { slots, .. } => Some(slots),
            _ => None,
        }
    }

    pub(crate) fn directory(&self) -> &Path {
        directory_of!(self).map_or(Path::new("/"), PathBuf::as_path)
    }

    pub(crate) fn directory_mut(&mut self) -> Option<&mut PathBuf> {
        directory_of!(self)
    }
}

/// A Task or Session write for the Machine that runs its Work.
#[derive(Debug)]
pub(super) struct Remote {
    machine: loopflow_model::durable::Machine,
    change: Write,
    sender: Sender,
    /// The Task this Machine routed the change for, as it knows it.
    route: Option<loopflow_execution::durable::TaskExecutionRoute>,
}

impl Remote {
    /// `change` for `machine`'s checkout of `repository`, with what this `lf`
    /// runs with and, for a launch, the secrets `forward` names. Its files are
    /// delivered to `machine` now.
    fn new(
        machine: loopflow_model::durable::Machine,
        repository: loopflow_model::durable::RepositoryId,
        change: Write,
        launching: bool,
        source: Option<loopflow_execution::ops::task::remote::TaskSource>,
    ) -> anyhow::Result<Self> {
        // What this Machine runs with comes from its own checkout of the
        // repository, as `read::peer_of` captures it: a client may name the
        // owner's checkout path, which does not exist here.
        let local = super::work_route::repository_path(&repository)
            .unwrap_or_else(|_| change.directory().to_path_buf());
        let selection = Selection::capture(&local, source)?;
        let secrets = match launching {
            true => {
                let mut forwarded = loopflow_execution::machine::forward::read(&local)?;
                let mut secrets = forwarded.take_secrets();
                // Sign-in advice a launch there prints names it as this Machine does.
                secrets.carry_launcher_name(machine.name());
                let mut notices = Vec::new();
                loopflow_execution::machine::forward::deliver(&machine, &forwarded, &mut notices)?;
                for line in String::from_utf8_lossy(&notices).lines() {
                    loopflow_execution::journal::request::diagnostic(format_args!("{line}"));
                }
                secrets
            }
            false => Secrets::default(),
        };
        Ok(Self {
            machine,
            change,
            sender: Sender {
                repository,
                selection,
                secrets,
            },
            route: None,
        })
    }

    /// The answer of the destination's server; nothing runs here instead.
    pub(super) fn call<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        let answer = Self::request(&self.machine, self.sender.clone(), &self.change)?;
        Ok(serde_json::from_value(answer.result)?)
    }

    fn request(
        machine: &loopflow_model::durable::Machine,
        sender: Sender,
        change: &Write,
    ) -> anyhow::Result<loopflow_execution::machine::client::Response> {
        loopflow_execution::machine::client::Connection::open_on(machine, sender, None)?
            .request(change)
    }
}

/// Resolve the Task once for the CLI or a served request. A Machine receiving
/// a forwarded request can save or sync the steer, but never forwards it again.
pub(super) fn steer_task(
    repo: &Path,
    issue: &str,
    wave: Option<&str>,
    message: &str,
    sender: Option<&Sender>,
) -> anyhow::Result<task::TaskSteerReceipt> {
    use super::work_route::{peer_issue, TaskPeer};
    use crate::server::wire::Operation;
    use loopflow_execution::engine::{Error, ErrorCode};

    loopflow_execution::ops::task::validate_task_comment_input(message)
        .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?;
    let rt = tokio::runtime::Runtime::new()?;
    let (store, task) = rt
        .block_on(loopflow_execution::ops::task::resolve_saved_task(
            repo, wave, issue,
        ))
        .with_context(|| format!("resolving Task {issue}"))?;
    let participant = match sender {
        Some(sender) => sender.selection.participant.clone(),
        None => loopflow_execution::config::participant_name_in(repo)?,
    };
    let save = |fallback| rt.block_on(task::steer(&store, &task, message, participant, fallback));
    let Some(peer) = TaskPeer::resolve(&store.sqlite, &task.id)? else {
        return save(None);
    };
    let name = peer.runs_on.machine.name();
    let open = match sender {
        None => peer.connect(repo, &task, Some(TaskPeer::deadline())),
        Some(_) if peer.registered.is_none() => Err(anyhow!("{name} is not registered here")),
        Some(_) => Err(anyhow!("{name} now runs the Task")),
    };
    let mut connection = match open {
        Ok(connection) => connection,
        Err(error) => {
            return save(Some((peer.runs_on.machine, format!("{error:#}"))));
        }
    };

    // Once transmitted, the owner may have saved it even if the reply is lost.
    let change = Write::TaskSteer {
        directory: repo.to_owned(),
        issue: peer_issue(&task),
        message: message.to_owned(),
    };
    let mut receipt: task::TaskSteerReceipt = connection
        .request(Operation::Write(change))
        .and_then(|answer| Ok(serde_json::from_value(answer.result)?))
        .map_err(|error| Error::refused(&error, ErrorCode::OutcomeUnknown))?;
    if receipt.delivery.reached == task::SteerReached::Owner {
        receipt.delivery.sentence = format!("{name} · {}", receipt.delivery.sentence);
        receipt.delivery.runs_on = Some(peer.runs_on.machine);
    }
    Ok(receipt)
}

/// `task.run` for `issue` from `directory`, where its Work lives.
pub(super) fn run_task(
    directory: &Path,
    issue: &str,
    from: Option<&str>,
) -> anyhow::Result<task::TaskRun> {
    let write = Write::TaskRun {
        directory: directory.to_path_buf(),
        issue: issue.to_owned(),
        graph: None,
        from: from.map(str::to_owned),
        agent: None,
        approve: false,
    };
    let WriteResult::TaskRun(run) = write.run()? else {
        unreachable!("task.run answers with a run")
    };
    Ok(*run)
}

pub(crate) fn find_machine(selector: &str) -> anyhow::Result<loopflow_model::durable::Machine> {
    tokio::runtime::Runtime::new()?
        .block_on(loopflow_execution::machine::connection::find_registered_machine(selector))
}

#[cfg(test)]
mod tests {
    use super::Write;

    #[test]
    fn typed_answers_keep_the_wire_shape() {
        use super::WriteResult;
        use loopflow_execution::engine::task::TaskRun;
        use serde_json::json;

        let running: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../loopflow-execution/tests/fixtures/dto/server/task_auto_on_answer.json"
        ))
        .unwrap();
        // `task.auto.on` answers as `task.run` does.
        for expected in [
            running["result"].clone(),
            json!({"state": "ended", "issue": "LOO-1"}),
        ] {
            let result: TaskRun = serde_json::from_value(expected.clone()).unwrap();
            let actual = serde_json::to_value(WriteResult::TaskAutoOn(Box::new(result))).unwrap();
            assert_eq!(actual, expected);
            assert_result_schema("task.auto.on", &actual);
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
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let path = home.path().join("config.yaml");
        let result = Write::ConfigTheme {
            value: Some("dark".into()),
        }
        .run()
        .unwrap();
        let super::WriteResult::ConfigTheme(declaration) = &result else {
            panic!("theme writes return their declaration");
        };
        assert_eq!(declaration.declaration.path, path);
        let actual = serde_json::to_value(result).unwrap();
        let mut expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/theme_declaration.json"
        ))
        .unwrap();
        expected["path"] = serde_json::json!(path);
        assert_eq!(actual, expected);
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
            r#"{"kind":"task.auto.off","directory":"/r","issue":"FIX-1","reason":null}"#,
            r#"{"kind":"desktop.portfolio.set","change":{"action":"remove","path":"/r"}}"#,
            r#"{"kind":"session.resume","directory":"/r","id":null,"task":null,"wave":null,"agent":null}"#,
            r#"{"kind":"session.new","directory":"/r","request_id":"new-thread","id":"session_1","task":"FIX-1","agent":"codex"}"#,
            r#"{"kind":"task.save","directory":"/","issue":"FIX-1","path":"a.txt","revision":"r","content":"text"}"#,
            r#"{"kind":"task.run","directory":"/r","issue":"FIX-1","graph":null,"from":"implement","agent":"claude","approve":false}"#,
            r#"{"kind":"task.auto.on","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"task.steer","directory":"/r","issue":"FIX-1","message":"keep the API"}"#,
            r#"{"kind":"task.report","directory":"/r","issue":"FIX-1","message":"landed"}"#,
            r#"{"kind":"task.interrupt","directory":"/r","issue":"FIX-1"}"#,
            r#"{"kind":"session.steer","directory":"/r","session":null,"wave":"product","message":"ship by Friday"}"#,
            r#"{"kind":"wave.plan.bind","replace":false,"directory":"/r","wave":"product","project":"3f1c"}"#,
            r#"{"kind":"wave.plan.connect","directory":"/r","wave":null,"all":true,"team_key":null,"team_name":null}"#,
            r#"{"kind":"wave.plan.reteam","directory":"/r","apply":false}"#,
            r#"{"kind":"repo.release","directory":"/r","version":"minor","target":null}"#,
            r#"{"kind":"checkout.commit","directory":"/r","message":"Tidy","push":false,"no_add":false,"paths":[],"agent":null}"#,
            r#"{"kind":"checkout.sync","directory":"/r","onto":null,"preview":true,"manual":false,"continue":false,"abort":false,"adopt":false}"#,
            r#"{"kind":"checkout.create","directory":"/r","name":"spike","preview":false,"persistent":false,"stack":false}"#,
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
}
