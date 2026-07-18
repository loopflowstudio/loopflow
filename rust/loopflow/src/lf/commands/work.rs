use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::durable::{
    AuthenticatedRequest, Basis, ControlCtx, EpochId, EpochReceipt, InterruptReceipt, LaunchId,
    ProjectId, Review, Run, SteerReceipt, TaskId, UserReview, WorkRef, WorkStatus,
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
    review: Option<Review>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Steer(SteerReceipt),
    ReviewContinued { status: WorkStatus },
    ReviewEscalated { review: Review },
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
        WorkCommand::Review {
            kind,
            id,
            continue_on_success,
            continue_on_exit,
        } => {
            let work = parse_work(kind, id)?;
            let policy = if *continue_on_exit {
                ReviewExitPolicy::AnyExit
            } else if *continue_on_success {
                ReviewExitPolicy::Success
            } else {
                ReviewExitPolicy::Explicit
            };
            run_review(&store, &work, policy).await?;
        }
        WorkCommand::Continue { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let review = store
                .review(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no current Review", work.kind(), work.id()))?;
            let status = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .close_review(&ControlCtx::Run(&lease), &work, &review.basis)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .close_review(&ControlCtx::User(&request), &work, &review.basis)
                    .await?
            };
            print_receipt(&WorkReceipt::ReviewContinued { status }, *json)?;
        }
        WorkCommand::Escalate { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let lease = crate::ops::ambient_run_lease(&store)
                .await?
                .ok_or_else(|| anyhow!("Review escalation requires an active parent Run"))?;
            let review = store
                .review(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no current Review", work.kind(), work.id()))?;
            let review = store.escalate_review(&lease, &work, &review.basis).await?;
            print_receipt(&WorkReceipt::ReviewEscalated { review }, *json)?;
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
        let reviews = store.user_attention().await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&reviews)?);
        } else if reviews.is_empty() {
            println!("No Work needs your attention.");
        } else {
            for item in reviews {
                let review = &item.review;
                println!(
                    "{} {}  {}:{}  {}",
                    review.work.kind(),
                    review.work.id(),
                    review.basis.epoch_id,
                    review.basis.revision,
                    review.position.step,
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
    mut basis: Basis,
) -> anyhow::Result<()> {
    let store = open_exit_guard_store().await?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    write_guard_reply(&mut output, &ReviewGuardReply::Ready)?;

    loop {
        let mut line = String::new();
        let bytes = input.read_line(&mut line)?;
        if bytes == 0 || !line.ends_with('\n') {
            break;
        }
        let command = match serde_json::from_str::<ReviewGuardCommand>(&line) {
            Ok(command) => command,
            Err(error) => {
                if write_guard_reply(
                    &mut output,
                    &ReviewGuardReply::Error {
                        message: format!("invalid Review guard command: {error}"),
                    },
                )
                .is_err()
                {
                    break;
                }
                continue;
            }
        };
        match command {
            ReviewGuardCommand::Cancel => return Ok(()),
            ReviewGuardCommand::Steer { message } => {
                match store
                    .steer_review_if_current(&work, &launch_id, &message, &basis)
                    .await
                {
                    Ok(receipt) => {
                        basis = receipt.steer.basis;
                        if write_guard_reply(
                            &mut output,
                            &ReviewGuardReply::Steered {
                                basis: basis.clone(),
                            },
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                    Err(error) => {
                        if write_guard_reply(
                            &mut output,
                            &ReviewGuardReply::Error {
                                message: error.to_string(),
                            },
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    }

    continue_guarded_review(&store, &work, &launch_id, &basis).await
}

async fn open_exit_guard_store() -> anyhow::Result<Store> {
    for attempt in 0..20 {
        match open_shared_store().await {
            Ok(store) => return Ok(store),
            Err(error) if attempt < 19 => {
                tracing::debug!(%error, "Review exit guard is retrying store open");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("Review exit guard store loop returns on its last attempt")
}

async fn continue_guarded_review(
    store: &Store,
    work: &WorkRef,
    launch_id: &LaunchId,
    basis: &Basis,
) -> anyhow::Result<()> {
    for attempt in 0..20 {
        match store
            .continue_review_if_current(work, launch_id, basis)
            .await
        {
            Ok(_) | Err(StoreError::NotFound | StoreError::InvalidAuthority(_)) => return Ok(()),
            Err(StoreError::StaleBasis { .. }) => return Ok(()),
            Err(StoreError::Sqlite(error)) if attempt < 19 => {
                tracing::debug!(%error, "Review exit guard is retrying SQLite");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(anyhow!(error)),
        }
    }
    unreachable!("Review exit guard retry loop returns on its last attempt")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewExitPolicy {
    Explicit,
    Success,
    AnyExit,
}

#[derive(Debug)]
enum InputEvent {
    Line(String),
    Eof,
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReviewGuardCommand {
    Steer { message: String },
    Cancel,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ReviewGuardReply {
    Ready,
    Steered { basis: Basis },
    Error { message: String },
}

struct ReviewExitGuard {
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    _lock: std::fs::File,
}

impl ReviewExitGuard {
    fn spawn(work: &WorkRef, review: &Review) -> anyhow::Result<Self> {
        let lock_directory = std::env::temp_dir().join("loopflow-review-guards");
        std::fs::create_dir_all(&lock_directory)?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_directory.join(format!("{}.lock", review.launch_id.as_str())))?;
        lock.try_lock_exclusive().map_err(|error| {
            anyhow!("another --continue-on-exit client already owns this Review: {error}")
        })?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("__review-exit-guard")
            .arg(work.kind())
            .arg(work.id())
            .arg(review.launch_id.as_str())
            .arg(review.basis.epoch_id.as_str())
            .arg(review.basis.revision.to_string())
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
        let mut child = command.spawn().context("start Review exit guard")?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Review exit guard did not open stdin"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Review exit guard did not open stdout"))?;
        let mut guard = Self {
            input,
            output: BufReader::new(output),
            _lock: lock,
        };
        match guard.read_reply()? {
            ReviewGuardReply::Ready => Ok(guard),
            ReviewGuardReply::Error { message } => Err(anyhow!(message)),
            ReviewGuardReply::Steered { .. } => Err(anyhow!(
                "Review exit guard sent a Steer before it was ready"
            )),
        }
    }

    fn steer(&mut self, message: &str) -> anyhow::Result<Basis> {
        self.write_command(&ReviewGuardCommand::Steer {
            message: message.to_string(),
        })?;
        match self.read_reply()? {
            ReviewGuardReply::Steered { basis } => Ok(basis),
            ReviewGuardReply::Error { message } => Err(anyhow!(message)),
            ReviewGuardReply::Ready => Err(anyhow!("Review exit guard sent an extra ready reply")),
        }
    }

    fn cancel(&mut self) {
        let _ = self.write_command(&ReviewGuardCommand::Cancel);
    }

    fn write_command(&mut self, command: &ReviewGuardCommand) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.input, command)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        Ok(())
    }

    fn read_reply(&mut self) -> anyhow::Result<ReviewGuardReply> {
        let mut line = String::new();
        if self.output.read_line(&mut line)? == 0 {
            return Err(anyhow!("Review exit guard exited unexpectedly"));
        }
        serde_json::from_str(&line).context("parse Review exit guard reply")
    }
}

fn write_guard_reply(output: &mut impl Write, reply: &ReviewGuardReply) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *output, reply)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

async fn run_review(store: &Store, work: &WorkRef, policy: ReviewExitPolicy) -> anyhow::Result<()> {
    let item = find_user_review(store, work)
        .await?
        .ok_or_else(|| anyhow!("{} {} has no current User Review", work.kind(), work.id()))?;
    let launch_id = item.review.launch_id.clone();
    let mut basis = item.review.basis.clone();
    let mut latest_output = item.latest_output.clone();
    print_review(&item, policy);

    let mut guard = if policy == ReviewExitPolicy::AnyExit {
        Some(ReviewExitGuard::spawn(work, &item.review)?)
    } else {
        None
    };
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut line = String::new();
            match input.read_line(&mut line) {
                Ok(0) => {
                    let _ = input_tx.send(InputEvent::Eof);
                    break;
                }
                Ok(_) => {
                    let _ = input_tx.send(InputEvent::Line(line));
                }
                Err(error) => {
                    let _ = input_tx.send(InputEvent::Error(error.to_string()));
                    break;
                }
            }
        }
    });
    let mut refresh = tokio::time::interval(Duration::from_millis(300));
    loop {
        tokio::select! {
            event = input_rx.recv() => {
                match event.unwrap_or(InputEvent::Eof) {
                    InputEvent::Line(line) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match line {
                            "/continue" => {
                                continue_review(store, work, &basis).await?;
                                if let Some(guard) = &mut guard {
                                    guard.cancel();
                                }
                                println!("Continued.");
                                return Ok(());
                            }
                            "/detach" if policy == ReviewExitPolicy::AnyExit => {
                                eprintln!("/detach is unavailable with --continue-on-exit");
                            }
                            "/detach" => {
                                println!("Detached; Review remains open.");
                                return Ok(());
                            }
                            "/status" => print_review_status(work, &basis, &launch_id),
                            text => {
                                if let Some(guard) = &mut guard {
                                    basis = guard.steer(text)?;
                                } else {
                                    let receipt = store
                                        .steer_review_if_current(work, &launch_id, text, &basis)
                                        .await?;
                                    basis = receipt.steer.basis;
                                }
                            }
                        }
                    }
                    InputEvent::Eof => {
                        if policy == ReviewExitPolicy::Explicit {
                            println!("Detached; Review remains open.");
                            return Ok(());
                        }
                        continue_review(store, work, &basis).await?;
                        if let Some(guard) = &mut guard {
                            guard.cancel();
                        }
                        println!("Continued.");
                        return Ok(());
                    }
                    InputEvent::Error(error) => return Err(anyhow!("read Review input: {error}")),
                }
            }
            _ = refresh.tick() => {
                let Some(current) = find_user_review(store, work).await? else {
                    if let Some(guard) = &mut guard {
                        guard.cancel();
                    }
                    println!("Review continued elsewhere.");
                    return Ok(());
                };
                if current.review.launch_id != launch_id || current.review.basis != basis {
                    if let Some(guard) = &mut guard {
                        guard.cancel();
                    }
                    return Err(anyhow!("Review changed concurrently; leaving it open"));
                }
                if current.latest_output != latest_output {
                    latest_output = current.latest_output;
                    if let Some(output) = &latest_output {
                        println!("\n{output}");
                    }
                }
            }
        }
    }
}

async fn find_user_review(store: &Store, work: &WorkRef) -> anyhow::Result<Option<UserReview>> {
    Ok(store
        .user_attention()
        .await?
        .into_iter()
        .find(|item| &item.review.work == work))
}

async fn continue_review(store: &Store, work: &WorkRef, basis: &Basis) -> anyhow::Result<()> {
    let request = AuthenticatedRequest::cli();
    store
        .close_review(&ControlCtx::User(&request), work, basis)
        .await?;
    Ok(())
}

fn print_review(item: &UserReview, policy: ReviewExitPolicy) {
    let review = &item.review;
    println!(
        "Reviewing {} {} at {}:{} ({})",
        review.work.kind(),
        review.work.id(),
        review.basis.epoch_id,
        review.basis.revision,
        review.position.step,
    );
    if policy == ReviewExitPolicy::AnyExit {
        println!("Send direction, /status, or /continue. Exiting also continues.");
    } else {
        println!("Send direction, /status, /continue, or /detach.");
    }
    if let Some(output) = &item.latest_output {
        println!("\n{output}");
    }
}

fn print_review_status(work: &WorkRef, basis: &Basis, launch_id: &LaunchId) {
    println!(
        "{} {}  basis {}:{}  launch {}",
        work.kind(),
        work.id(),
        basis.epoch_id,
        basis.revision,
        launch_id,
    );
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
        review: store.review(work).await?,
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
            projection.review.as_ref().map_or("none", |review| {
                match (&review.attention, review.attention_at.is_some()) {
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
            WorkReceipt::Steer(receipt) => println!("steered {}", receipt.steer.id),
            WorkReceipt::ReviewContinued { status } => println!("continued Review: {status:?}"),
            WorkReceipt::ReviewEscalated { review } => println!(
                "escalated {} {} Review to User attention",
                review.work.kind(),
                review.work.id()
            ),
            WorkReceipt::Interrupted(receipt) => println!("interrupted {}", receipt.run_id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.epoch.id),
        }
    }
    Ok(())
}
