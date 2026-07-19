use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::time::Duration;

use anyhow::{anyhow, Context};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::durable::{
    AuthenticatedRequest, Basis, ControlCtx, EpochId, EpochReceipt, Feedback, InterruptReceipt,
    LaunchId, Placement, ProjectId, Run, SteerReceipt, TaskId, UserFeedback, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::lf::WorkCommand;
use crate::store::{open_store, storage_config_from_env, Store, StoreError};

#[derive(Debug, Serialize)]
struct WorkProjection {
    work: WorkRef,
    basis: crate::durable::Basis,
    status: WorkStatus,
    run: Option<Run>,
    feedback: Option<Feedback>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Placed(Placement),
    Steer(SteerReceipt),
    FeedbackContinued { status: WorkStatus },
    FeedbackEscalated { feedback: Feedback },
    Interrupted(InterruptReceipt),
    Abandoned(EpochReceipt),
}

pub fn run(command: &WorkCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(command))
}

async fn run_async(command: &WorkCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        WorkCommand::Status { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let projection = projection(&store, &work).await?;
            print_projection(&projection, *json)?;
        }
        WorkCommand::Place {
            kind,
            id,
            home_id,
            json,
        } => {
            let work = parse_work(kind, id)?;
            if !matches!(work, WorkRef::Wave(_)) {
                return Err(anyhow!(
                    "only Wave Work can move until Project and Task execution uses the shared Run supervisor"
                ));
            }
            let placement = store.place_work(&work, home_id).await?;
            print_receipt(&WorkReceipt::Placed(placement), *json)?;
        }
        WorkCommand::Steer {
            kind,
            id,
            message,
            json,
        } => {
            let work = parse_work(kind, id)?;
            let receipt = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .steer(&ControlCtx::Run(&lease), &work, message, None)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .steer(&ControlCtx::User(&request), &work, message, None)
                    .await?
            };
            print_receipt(&WorkReceipt::Steer(receipt), *json)?;
        }
        WorkCommand::Feedback {
            kind,
            id,
            continue_on_success,
            continue_on_exit,
        } => {
            let work = parse_work(kind, id)?;
            let policy = if *continue_on_exit {
                FeedbackExitPolicy::AnyExit
            } else if *continue_on_success {
                FeedbackExitPolicy::Success
            } else {
                FeedbackExitPolicy::Explicit
            };
            run_feedback(&store, &work, policy).await?;
        }
        WorkCommand::Continue { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let feedback = store
                .feedback(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no current Feedback", work.kind(), work.id()))?;
            let status = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .continue_feedback(&ControlCtx::Run(&lease), &work, &feedback.basis)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .continue_feedback(&ControlCtx::User(&request), &work, &feedback.basis)
                    .await?
            };
            print_receipt(&WorkReceipt::FeedbackContinued { status }, *json)?;
        }
        WorkCommand::Escalate { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let lease = crate::ops::ambient_run_lease(&store)
                .await?
                .ok_or_else(|| anyhow!("Feedback escalation requires an active parent Run"))?;
            let feedback = store
                .feedback(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no current Feedback", work.kind(), work.id()))?;
            let feedback = store
                .escalate_feedback(&lease, &work, &feedback.basis)
                .await?;
            print_receipt(&WorkReceipt::FeedbackEscalated { feedback }, *json)?;
        }
        WorkCommand::Interrupt { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let run = store
                .current_run(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no active Run", work.kind(), work.id()))?;
            let receipt = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .interrupt(&ControlCtx::Run(&lease), &work, &run.id)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .interrupt(&ControlCtx::User(&request), &work, &run.id)
                    .await?
            };
            print_receipt(&WorkReceipt::Interrupted(receipt), *json)?;
        }
        WorkCommand::Abandon {
            kind,
            id,
            reason,
            json,
        } => {
            let work = parse_work(kind, id)?;
            if crate::ops::ambient_run_lease(&store).await?.is_some() {
                return Err(anyhow!(
                    "Run callers cannot abandon Work; use the authenticated User surface"
                ));
            }
            let basis = store.current_epoch(&work).await?.current_basis;
            let receipt = store.abandon(&work, reason, &basis).await?;
            print_receipt(&WorkReceipt::Abandoned(receipt), *json)?;
        }
    }
    Ok(())
}

pub fn run_queue(json: bool) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async move {
        let store = open_shared_store().await?;
        let feedback = store.user_attention().await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&feedback)?);
        } else if feedback.is_empty() {
            println!("No Work needs your attention.");
        } else {
            for item in feedback {
                let feedback = &item.feedback;
                println!(
                    "{} {}  {}:{}  {}",
                    feedback.work.kind(),
                    feedback.work.id(),
                    feedback.basis.epoch_id,
                    feedback.basis.revision,
                    feedback.position.step,
                );
            }
        }
        Ok(())
    })
}

pub fn run_exit_guard(
    kind: &str,
    id: &str,
    launch_id: &str,
    epoch_id: &str,
    revision: u64,
) -> anyhow::Result<()> {
    let work = parse_work(kind, id)?;
    let launch_id = LaunchId::parse(launch_id)?;
    let epoch_id = EpochId::parse(epoch_id)?;
    let basis = Basis { epoch_id, revision };
    tokio::runtime::Runtime::new()?.block_on(run_exit_guard_async(work, launch_id, basis))
}

async fn run_exit_guard_async(
    work: WorkRef,
    launch_id: LaunchId,
    basis: Basis,
) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let _lock = match feedback_guard_lock(&launch_id) {
        Ok(lock) => lock,
        Err(error) => {
            write_guard_reply(
                &mut output,
                &FeedbackGuardReply::Error {
                    message: error.to_string(),
                },
            )?;
            return Ok(());
        }
    };
    let store = match open_exit_guard_store().await {
        Ok(store) => store,
        Err(error) => {
            write_guard_reply(
                &mut output,
                &FeedbackGuardReply::Error {
                    message: error.to_string(),
                },
            )?;
            return Ok(());
        }
    };
    write_guard_reply(&mut output, &FeedbackGuardReply::Ready)?;
    drop(output);
    io::copy(&mut io::stdin().lock(), &mut io::sink())?;
    continue_guarded_feedback(&store, &work, &launch_id, &basis).await
}

fn feedback_guard_lock(launch_id: &LaunchId) -> anyhow::Result<std::fs::File> {
    let lock_directory = std::env::temp_dir().join("loopflow-feedback-guards");
    std::fs::create_dir_all(&lock_directory)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_directory.join(format!("{}.lock", launch_id.as_str())))?;
    lock.try_lock_exclusive().map_err(|error| {
        anyhow!("another --continue-on-exit client already owns this Feedback: {error}")
    })?;
    Ok(lock)
}

async fn open_exit_guard_store() -> anyhow::Result<Store> {
    for attempt in 0..20 {
        match open_shared_store().await {
            Ok(store) => return Ok(store),
            Err(error) if attempt < 19 => {
                tracing::debug!(%error, "Feedback exit guard is retrying store open");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("Feedback exit guard store loop returns on its last attempt")
}

async fn continue_guarded_feedback(
    store: &Store,
    work: &WorkRef,
    launch_id: &LaunchId,
    basis: &Basis,
) -> anyhow::Result<()> {
    for attempt in 0..20 {
        match store
            .continue_feedback_if_current(work, launch_id, basis)
            .await
        {
            Ok(_) | Err(StoreError::NotFound | StoreError::InvalidAuthority(_)) => return Ok(()),
            Err(StoreError::StaleBasis { .. }) => return Ok(()),
            Err(StoreError::Sqlite(error)) if attempt < 19 => {
                tracing::debug!(%error, "Feedback exit guard is retrying SQLite");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(anyhow!(error)),
        }
    }
    unreachable!("Feedback exit guard retry loop returns on its last attempt")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedbackExitPolicy {
    Explicit,
    Success,
    AnyExit,
}

impl FeedbackExitPolicy {
    fn continues(self, presentation_succeeded: bool) -> bool {
        match self {
            Self::Explicit => false,
            Self::Success => presentation_succeeded,
            Self::AnyExit => true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FeedbackGuardReply {
    Ready,
    Error { message: String },
}

struct FeedbackExitGuard {
    input: ChildStdin,
    child: Child,
}

impl FeedbackExitGuard {
    fn spawn(work: &WorkRef, feedback: &Feedback) -> anyhow::Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("__feedback-exit-guard")
            .arg(work.kind())
            .arg(work.id())
            .arg(feedback.launch_id.as_str())
            .arg(feedback.basis.epoch_id.as_str())
            .arg(feedback.basis.revision.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        // SAFETY: the closure only starts a new session before exec and does not
        // touch memory shared with the parent process.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().context("start Feedback exit guard")?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Feedback exit guard did not open stdin"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Feedback exit guard did not open stdout"))?;
        let mut output = BufReader::new(output);
        let mut line = String::new();
        if output.read_line(&mut line)? == 0 {
            return Err(anyhow!("Feedback exit guard exited unexpectedly"));
        }
        match serde_json::from_str(&line).context("parse Feedback exit guard reply")? {
            FeedbackGuardReply::Ready => Ok(Self { input, child }),
            FeedbackGuardReply::Error { message } => Err(anyhow!(message)),
        }
    }

    fn continue_now(self) -> anyhow::Result<()> {
        let Self { input, mut child } = self;
        drop(input);
        let status = child.wait().context("wait for Feedback exit guard")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("Feedback exit guard exited with {status}"))
        }
    }
}

fn write_guard_reply(output: &mut impl Write, reply: &FeedbackGuardReply) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *output, reply)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

async fn run_feedback(
    store: &Store,
    work: &WorkRef,
    policy: FeedbackExitPolicy,
) -> anyhow::Result<()> {
    let item = find_user_feedback(store, work)
        .await?
        .ok_or_else(|| anyhow!("{} {} has no current User Feedback", work.kind(), work.id()))?;
    let guard = if policy == FeedbackExitPolicy::AnyExit {
        Some(FeedbackExitGuard::spawn(work, &item.feedback)?)
    } else {
        None
    };
    println!(
        "Opening Feedback for {} {} at {}:{} ({})",
        item.feedback.work.kind(),
        item.feedback.work.id(),
        item.feedback.basis.epoch_id,
        item.feedback.basis.revision,
        item.feedback.position.step,
    );
    let status = present_feedback(&item.feedback)?;
    let should_continue = policy.continues(status.success());
    if should_continue {
        if let Some(guard) = guard {
            guard.continue_now()?;
        } else {
            continue_guarded_feedback(store, work, &item.feedback.launch_id, &item.feedback.basis)
                .await?;
        }
        println!("Continued Feedback.");
    }
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Feedback presentation exited with {status}"))
    }
}

fn present_feedback(feedback: &Feedback) -> anyhow::Result<ExitStatus> {
    Command::new(std::env::current_exe()?)
        .arg("launch")
        .arg("present")
        .arg(feedback.launch_id.as_str())
        .status()
        .context("present Feedback Launch")
}

async fn find_user_feedback(store: &Store, work: &WorkRef) -> anyhow::Result<Option<UserFeedback>> {
    Ok(store
        .user_attention()
        .await?
        .into_iter()
        .find(|item| &item.feedback.work == work))
}

async fn open_shared_store() -> anyhow::Result<Store> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .context("open the shared Loopflow store")
}

async fn projection(store: &Store, work: &WorkRef) -> anyhow::Result<WorkProjection> {
    Ok(WorkProjection {
        work: work.clone(),
        basis: store.current_epoch(work).await?.current_basis,
        status: store.work_status(work).await?,
        run: store.current_run(work).await?,
        feedback: store.feedback(work).await?,
    })
}

fn parse_work(kind: &str, id: &str) -> anyhow::Result<WorkRef> {
    match kind {
        "wave" => Ok(WorkRef::Wave(WaveId::parse(id)?)),
        "project" => Ok(WorkRef::Project(ProjectId::parse(id)?)),
        "task" => Ok(WorkRef::Task(TaskId::parse(id)?)),
        value => Err(anyhow!("invalid Work kind {value:?}")),
    }
}

fn print_projection(projection: &WorkProjection, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(projection)?);
    } else {
        println!(
            "{} {}  {:?}\n  basis: {}:{}\n  run: {}\n  attention: {}",
            projection.work.kind(),
            projection.work.id(),
            projection.status,
            projection.basis.epoch_id,
            projection.basis.revision,
            projection
                .run
                .as_ref()
                .map_or("none", |run| run.id.as_str()),
            projection.feedback.as_ref().map_or("none", |feedback| {
                match (&feedback.attention, feedback.attention_at.is_some()) {
                    (crate::durable::AttentionRoute::User, true) => "user (pending)",
                    (crate::durable::AttentionRoute::User, false) => "user (parked)",
                    (crate::durable::AttentionRoute::Parent(_), true) => "parent (pending)",
                    (crate::durable::AttentionRoute::Parent(_), false) => "parent (parked)",
                }
            }),
        );
    }
    Ok(())
}

fn print_receipt(receipt: &WorkReceipt, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt)?);
    } else {
        match receipt {
            WorkReceipt::Placed(placement) => println!(
                "{} {}  ->  {}",
                placement.work.kind(),
                placement.work.id(),
                placement.home_id
            ),
            WorkReceipt::Steer(receipt) => println!("steered {}", receipt.steer.id),
            WorkReceipt::FeedbackContinued { status } => println!("continued Feedback: {status:?}"),
            WorkReceipt::FeedbackEscalated { feedback } => println!(
                "escalated {} {} Feedback to User attention",
                feedback.work.kind(),
                feedback.work.id()
            ),
            WorkReceipt::Interrupted(receipt) => println!("interrupted {}", receipt.run_id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.epoch.id),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::FeedbackExitPolicy;

    #[test]
    fn feedback_exit_policies_match_the_presentation_contract() {
        assert!(!FeedbackExitPolicy::Explicit.continues(true));
        assert!(!FeedbackExitPolicy::Explicit.continues(false));
        assert!(FeedbackExitPolicy::Success.continues(true));
        assert!(!FeedbackExitPolicy::Success.continues(false));
        assert!(FeedbackExitPolicy::AnyExit.continues(true));
        assert!(FeedbackExitPolicy::AnyExit.continues(false));
    }
}
