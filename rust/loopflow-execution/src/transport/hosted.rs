//! One held terminal's execution as the Machine server hosts it: its viewers,
//! every one of which may type, who sizes it, the retained screen, queued input, the
//! native client's return line and exit settlement. The execution itself is
//! behind [`Source`], the holder's socket.

use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use crate::harness::native_exit::NativeExit;
use loopflow_model::id::TerminalId;

use super::held::Source;
use super::input::{self, PendingInput};
use super::wire::{CloseReason, Event, Incoming, Reply, Request, Tag};
use super::{
    close_viewers, deliver, dimensions, end_viewers, lifetime, native, output, place_events,
    provider_exit_code, publish_reports, release_viewer_hold, selected_directory, snapshot,
    viewer_hold, Settling, Sizing, Span, Viewer, HEARTBEAT_TIMEOUT, LIMIT,
};
use loopflow_storage::sqlite::ViewerHold;

/// PTY output read from the holder, with the line discipline it was written under.
pub(super) struct Chunk {
    pub bytes: Vec<u8>,
    /// The terminal was raw (no line editing) when this was read.
    pub raw: bool,
    /// The PTY position after these bytes.
    pub end: u64,
}

/// End what a Desktop shell left running in its terminal's session, off the
/// host's loop: stragglers get up to three seconds.
fn end_jobs(source: &Source, terminal: &TerminalId) -> Result<()> {
    if !crate::journal::process_ledger()?.is_shell_terminal(terminal)? {
        return Ok(());
    }
    // The holder made the shell its session's leader.
    let jobs = crate::os_process::TerminalJobs::new(source.pid());
    let terminal = terminal.clone();
    std::thread::spawn(move || {
        let settled = jobs
            .terminate()
            .map_err(anyhow::Error::from)
            .and_then(|()| Ok(crate::journal::process_ledger()?.settle_shell_jobs(&terminal)?));
        if let Err(error) = settled {
            tracing::error!(%terminal, %error, "Shell job cleanup remains pending");
        }
    });
    Ok(())
}

/// What one pass over a hosted execution came to.
pub enum Pass {
    Continue,
    /// The execution ended with this code and its viewers have its last
    /// output. Records still being written come back to be waited for.
    Ended(u8, Option<Settling>),
}

pub(super) struct Hosted {
    id: TerminalId,
    /// What the execution's own control requests present.
    control_key: String,
    viewers: Vec<Viewer>,
    /// The view whose input last reached the provider, which ends its paste if
    /// it leaves mid-way.
    writer: Option<String>,
    sizing: Sizing,
    size: (u16, u16),
    pending_input: PendingInput,
    output: output::Output,
    exit: Option<(u8, Instant)>,
    detach: bool,
    client: Option<native::NativeClient>,
    returning: Option<Event>,
    hint_due: bool,
    announced: bool,
    attached: bool,
    viewing: lifetime::Viewing,
    close_reason: CloseReason,
    /// Wakes the host's descriptor wait from another thread.
    waker: UnixStream,
    settling: Option<Settling>,
    observed_hold: Option<ViewerHold>,
    exit_record: Option<Box<NativeExit>>,
    /// The exec'd native client's attachment, checked off the byte path.
    attachment: Arc<lifetime::Client>,
    /// Where that client's conversation moved, once the watcher saw it go.
    departure: Option<crate::harness::native_exit::Settled>,
    /// Where a held terminal keeps what its launcher handed off, so a server
    /// that takes it up later has it too.
    kept: Option<std::path::PathBuf>,
    execution_directory_reported: bool,
    title: crate::terminal_title::OutputTitle,
    /// The Program Status viewers were last sent.
    reported_status: Option<loopflow_model::program_status::Records>,
    /// A taken-up terminal's output through this PTY position is replayed
    /// from the ring: its queries were answered when it was printed, or
    /// went unanswered while no server ran. Nothing answers them now.
    replayed_through: u64,
}

/// The threads the Machine server's held terminals share: viewing records
/// and Session-name reads.
#[derive(Debug)]
pub(super) struct Watchers {
    lifetime: lifetime::Watcher,
    titles: crate::terminal_title::TitleReader,
}

impl Watchers {
    pub fn spawn() -> Self {
        Self {
            lifetime: lifetime::Watcher::spawn(),
            titles: crate::terminal_title::TitleReader::spawn(),
        }
    }
}

impl Hosted {
    /// Host the execution known as `id`. `waker` wakes the host's wait when
    /// a settlement or a moved client has something for it.
    pub fn new(
        id: TerminalId,
        control_key: String,
        waker: UnixStream,
        watchers: &Watchers,
    ) -> Result<Self> {
        let attachment = Arc::new(lifetime::Client::default());
        let viewing =
            watchers
                .lifetime
                .watch(id.clone(), waker.try_clone()?, Arc::clone(&attachment));
        Ok(Self {
            control_key,
            viewers: Vec::new(),
            writer: None,
            sizing: Sizing::default(),
            size: (24, 80),
            pending_input: PendingInput::default(),
            output: output::Output::new(true)?,
            exit: None,
            detach: false,
            client: None,
            returning: None,
            hint_due: false,
            announced: false,
            attached: false,
            viewing,
            close_reason: CloseReason::Finished,
            waker,
            settling: None,
            observed_hold: None,
            exit_record: None,
            attachment,
            departure: None,
            kept: None,
            execution_directory_reported: false,
            title: crate::terminal_title::OutputTitle::new(&id, &watchers.titles),
            reported_status: None,
            replayed_through: 0,
            id,
        })
    }

    /// Take up a terminal already running at `rows` by `cols` whose output
    /// through PTY position `through` is replayed from its ring: the screen is
    /// built at the size its program last drew for.
    pub fn take_up(&mut self, rows: u16, cols: u16, through: u64) -> Result<()> {
        self.size = dimensions(rows, cols);
        self.output.resize(self.size.0, self.size.1)?;
        self.replayed_through = through;
        Ok(())
    }

    /// Keep the launcher's exit record and native client in `directory`,
    /// taking up what an earlier server kept there.
    pub fn keep_in(&mut self, directory: std::path::PathBuf) {
        if let Some(record) = read_kept::<Box<NativeExit>>(&directory.join(EXIT_RECORD)) {
            self.attachment.set(Some(&record));
            self.exit_record = Some(record);
        }
        if let Some(started) = read_kept::<NativeStarted>(&directory.join(NATIVE_CLIENT)) {
            self.client = Some(native::NativeClient::new(started.exit, started.detach));
        }
        self.kept = Some(directory);
    }

    /// Stop keeping `name`: a later server takes none up.
    fn forget_kept(&self, name: &str) {
        if let Some(directory) = &self.kept {
            remove_kept(&directory.join(name));
        }
    }

    /// The rows and columns the execution is drawn at.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// A connection that may become a viewer or carry the execution's requests.
    pub fn admit(&mut self, socket: UnixStream) {
        self.viewers.push(Viewer::new(socket));
    }

    /// The Program Status the execution's output reported, once it has a screen.
    pub fn program_status(&self) -> Option<&loopflow_model::program_status::Records> {
        self.output
            .terminal()
            .map(|terminal| terminal.program_status())
    }

    /// Serve every connection, move input and output, and judge the ending.
    pub fn pass(&mut self, child: &mut Source, mut bind_session: impl FnMut(&str)) -> Result<Pass> {
        let child_pid = child.pid();
        let previous_owner = self.sizing.id().map(str::to_owned);
        let mut size_change = false;
        let mut theme_change = false;
        let mut attaching = Vec::new();
        let mut closes = Vec::new();
        // Observe every connection before reading its requests.
        self.viewers.retain_mut(|viewer| {
            !viewer.heartbeat_expired(Instant::now()) && viewer.read().is_ok()
        });
        self.sizing.settle(&self.viewers);
        self.finish_departed_paste();
        let viewed = self.viewers.iter().any(Viewer::live);
        let native_ready = self.client.is_some()
            && self
                .viewers
                .iter()
                .any(|viewer| viewer.live() && viewer.ready);
        {
            let Self {
                viewers,
                writer,
                control_key,
                sizing,
                pending_input,
                output,
                detach,
                client,
                returning,
                hint_due,
                attached,
                exit_record,
                attachment,
                departure,
                kept,
                execution_directory_reported,
                ..
            } = self;
            for index in 0..viewers.len() {
                let viewer = &mut viewers[index];
                let result = (|| -> Result<()> {
                    while let Some((tag, payload)) = viewer.decoder.next()? {
                        let request = match tag {
                            Tag::Input => {
                                // Every attached view types. Only the view that
                                // sizes the terminal answers the agent's queries:
                                // every view's terminal answers them.
                                if !viewer.attached {
                                    continue;
                                }
                                let typed = if sizing.held_by(viewer) {
                                    payload
                                } else {
                                    input::keys(&payload).collect()
                                };
                                if typed.is_empty() {
                                    continue;
                                }
                                // Another view's paste ends before this one types.
                                if writer.as_ref() != Some(&viewer.id) {
                                    pending_input.finish_paste();
                                }
                                anyhow::ensure!(
                                    pending_input.push(&typed),
                                    "Provider input is busy"
                                );
                                *writer = Some(viewer.id.clone());
                                sizing.touch(viewer);
                                continue;
                            }
                            Tag::Request => serde_json::from_slice::<Incoming>(&payload)?,
                            _ => bail!("unexpected frame from viewer"),
                        };
                        let Incoming {
                            id: answers,
                            request,
                        } = request;
                        match request {
                            Request::Status {
                                key,
                                program,
                                state,
                                progress,
                            } if key == *control_key => {
                                output.report(program, state, progress);
                            }
                            Request::DetachViewers { key } if key == *control_key => {
                                *detach = true;
                            }
                            Request::Directory { key, cwd } if key == *control_key => {
                                output.directory(&cwd);
                            }
                            Request::NativeStarted {
                                key,
                                exit,
                                detach: line,
                            } if key == *control_key => {
                                *returning = Some(Event::Return {
                                    detach: line.clone(),
                                });
                                // A launcher that execs its provider never
                                // announces it again; a later server reads this.
                                if let Some(directory) = kept {
                                    let started = NativeStarted {
                                        exit: exit.clone(),
                                        detach: line.clone(),
                                    };
                                    keep(&directory.join(NATIVE_CLIENT), &started);
                                }
                                *client = Some(native::NativeClient::new(exit, line));
                            }
                            Request::NativeEnded { key } if key == *control_key => {
                                viewer.ending = Some(answers);
                            }
                            Request::CloseNotice { text } => {
                                closes.push((viewer.id.clone(), answers, Some(text)));
                            }
                            Request::RetireChat => {
                                closes.push((viewer.id.clone(), answers, None));
                            }
                            Request::InputHint { key, hint } if key == *control_key => {
                                viewer.input_hint = Some(hint);
                                *hint_due = true;
                            }
                            Request::Focus { focused } if viewer.attached => {
                                if focused {
                                    sizing.touch(viewer);
                                }
                                if sizing.held_by(viewer)
                                    && output
                                        .terminal()
                                        .is_some_and(|terminal| terminal.focus_reporting())
                                {
                                    let report: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
                                    let _ = pending_input.push(report);
                                }
                            }
                            Request::Controlled { ready } => {
                                if let Some(ready) = ready {
                                    viewer.ready = viewer.attached
                                        && client
                                            .as_ref()
                                            .is_some_and(|client| client.detach == ready);
                                }
                                if viewer.attached {
                                    viewer.heartbeat_at = Instant::now();
                                }
                                viewer.reply(
                                    answers,
                                    Reply::Controlled {
                                        viewed,
                                        ready: Some(native_ready),
                                    },
                                );
                            }
                            Request::ExitRecord { key, pid, record } if key == *control_key => {
                                // Graph steps share this terminal, but their Graph
                                // still owns their wait. Only our child can hand it off.
                                if pid != child_pid {
                                    viewer.reply(answers, Reply::Busy);
                                    continue;
                                }
                                if let Some(directory) = kept {
                                    keep(&directory.join(EXIT_RECORD), &record);
                                }
                                bind_session(&record.session);
                                attachment.set(Some(&record));
                                *departure = None;
                                *exit_record = Some(record);
                                viewer.reply(answers, Reply::Accepted);
                            }
                            Request::BindSession { key, session } if key == *control_key => {
                                bind_session(&session);
                                viewer.reply(answers, Reply::Accepted);
                            }
                            Request::DropExitRecord { key } if key == *control_key => {
                                *exit_record = None;
                                attachment.set(None);
                                *departure = None;
                                if let Some(directory) = kept {
                                    remove_kept(&directory.join(EXIT_RECORD));
                                }
                                viewer.reply(answers, Reply::Accepted);
                            }
                            Request::Attach {
                                rows,
                                cols,
                                name,
                                from,
                                focused,
                            } => {
                                *attached = true;
                                viewer.size = dimensions(rows, cols);
                                viewer.name = name;
                                viewer.from = from;
                                // A view continuing its screen showed the hint already.
                                viewer.wants_hint = from.is_none();
                                *hint_due = true;
                                viewer.attached = true;
                                viewer.heartbeat_at = Instant::now();
                                attaching.push(viewer.id.clone());
                                sizing.attach(viewer, focused.unwrap_or(true));
                                viewer.reply(answers, viewer.attached_reply(sizing.id()));
                                // Binding selects a directory for later connections;
                                // the running provider keeps its own.
                                if let Some(cwd) = exit_record
                                    .as_deref()
                                    .filter(|_| *execution_directory_reported)
                                    .and_then(selected_directory)
                                {
                                    output.directory(&cwd);
                                }
                                if let Some(cwd) = output.current_directory() {
                                    viewer.event(Event::Directory {
                                        cwd: cwd.to_path_buf(),
                                    });
                                }
                            }
                            Request::Resize { rows, cols } => {
                                viewer.size = dimensions(rows, cols);
                                size_change |= sizing.held_by(viewer);
                            }
                            Request::Theme { theme } if viewer.attached => {
                                viewer.theme = Some(theme);
                                theme_change |= sizing.held_by(viewer);
                            }
                            Request::Leave => {
                                viewer.leaving = true;
                                break;
                            }
                            _ => {} // a key the execution did not issue, or a passive connection
                        }
                    }
                    viewer.flush()?;
                    Ok(())
                })();
                viewer.leaving |= result.is_err();
                if viewer.leaving && sizing.held_by(viewer) {
                    sizing.settle(viewers);
                }
            }
        }
        self.viewers.retain(|viewer| !viewer.leaving);
        self.finish_departed_paste();
        let owner = self
            .viewers
            .iter()
            .find(|viewer| self.sizing.held_by(viewer));
        let owner_name = owner.and_then(|viewer| viewer.name.clone());
        let owner_change = previous_owner.as_deref() != self.sizing.id();
        let new_size = owner
            .filter(|_| owner_change || size_change)
            .map(|viewer| viewer.size);
        let theme = owner
            .filter(|_| owner_change || theme_change || !attaching.is_empty())
            .and_then(|viewer| viewer.theme.clone());
        if let Some((rows, cols)) = new_size {
            self.size = (rows, cols);
            child.resize(rows, cols);
            self.output.resize(rows, cols)?;
        }
        for viewer in self.viewers.iter_mut().filter(|viewer| viewer.attached) {
            // Fresh streams and ownership changes share the same appearance.
            let fresh = attaching.contains(&viewer.id);
            if fresh {
                // A held terminal's view that shows its screen up to a
                // position continues from there, without a snapshot.
                let from = viewer.from.take();
                viewer.catch_up = from.and_then(|from| child.catch_up(from));
                if let (Some(from), Some(_)) = (from, &viewer.catch_up) {
                    viewer.position = from;
                    viewer.catching_up = true;
                } else {
                    // Select the new owner's dimensions before painting its first
                    // screen, without waiting for a provider redraw.
                    paint(&self.output, child, viewer)?;
                    if from.is_some() {
                        viewer.event(Event::Live);
                    }
                }
            }
            if new_size.is_some() || fresh {
                viewer.event(Event::Resize {
                    rows: self.size.0,
                    cols: self.size.1,
                });
            }
            if owner_change || theme_change || fresh {
                if let Some(theme) = &theme {
                    viewer.event(Event::Theme {
                        theme: theme.clone(),
                    });
                }
            }
            if owner_change || fresh {
                viewer.event(Event::Owner {
                    viewer: self.sizing.id().map(str::to_owned),
                    name: owner_name.clone(),
                    held: self.sizing.held_by(viewer),
                });
            }
            if fresh {
                if let Some(status) = self.output.program_status() {
                    viewer.event(Event::ProgramStatus {
                        status: status.clone(),
                    });
                }
                if let Some(client) = &self.client {
                    viewer.event(Event::Return {
                        detach: client.detach.clone(),
                    });
                }
                if self.announced {
                    if let Some((code, _)) = self.exit {
                        viewer.event(Event::ChildExited { code });
                    }
                }
            }
        }
        if let Some(event) = self.returning.take() {
            for viewer in self.viewers.iter_mut().filter(|viewer| viewer.attached) {
                viewer.ready = false;
                viewer.event(event.clone());
            }
        }
        // The controls reach each view when it attaches and every view when the
        // execution registers them; a view continuing its screen has them already.
        if std::mem::take(&mut self.hint_due) {
            let hint = self.viewers.iter().rev().find_map(|v| v.input_hint.clone());
            if let Some(hint) = hint {
                for viewer in self
                    .viewers
                    .iter_mut()
                    .filter(|v| v.attached && v.wants_hint)
                {
                    viewer.wants_hint = false;
                    viewer.event(Event::Hint { text: hint.clone() });
                }
            }
        }
        // The view that sizes the terminal answers the agent's queries. While
        // it catches up it throws its answers away; the retained screen
        // answers live queries meanwhile, and when no view is attached.
        let answering = !self
            .viewers
            .iter()
            .any(|v| self.sizing.held_by(v) && !v.catching_up);
        if let Some(name) = self.title.tick() {
            self.output.title(&name);
        }
        child.forward(&mut self.pending_input);
        for chunk in child.read(16) {
            // An exec handoff announces before exec. The provider's first
            // output establishes that it reached its configured directory.
            if !self.execution_directory_reported {
                if let Some(record) = &self.exit_record {
                    let connected = crate::journal::process_ledger()
                        .and_then(|store| store.agent_state(&record.attachment.agent_process_id))
                        .ok()
                        .flatten()
                        .is_some_and(|agent| agent.pid.is_some() && agent.ended_at.is_none());
                    if connected {
                        self.output.directory(&record.cwd);
                        self.execution_directory_reported = true;
                    }
                }
            }
            let answering = answering && chunk.end > self.replayed_through;
            let pty = Span::Pty {
                start: chunk.end - chunk.bytes.len() as u64,
                end: chunk.end,
            };
            let aside = Span::Aside {
                through: Some(chunk.end),
            };
            let (ready, span) = match self.client.as_mut() {
                Some(client) => {
                    // A native client that changed the output sends lf's text.
                    let ready = client.accept(chunk.raw, &chunk.bytes);
                    let span = if ready == chunk.bytes { pty } else { aside };
                    (ready, span)
                }
                None => (chunk.bytes, pty),
            };
            self.deliver(1, &ready, answering, span);
        }
        self.catch_up(child)?;
        // After the reads: a provider that prints once and waits wakes nothing further.
        publish_reports(&mut self.output, &mut self.viewers);
        place_events(&self.output, &mut self.viewers, self.announced);
        // Output written before the execution detached reaches its viewers first.
        if self.detach {
            self.pending_input.finish_paste();
            self.writer = None;
            end_viewers(&mut self.viewers, Event::Detached);
            // The execution's own control connections outlive its viewers.
            self.viewers.retain(|v| !v.attached);
            self.detach = false;
        }
        if let Some(held) = self.client.as_mut().map(native::NativeClient::expire) {
            self.deliver(1, &held, answering, Span::Aside { through: None });
        }
        // The child wait reports once. A returning launcher announces its
        // client's end; an exec'd launcher leaves that announcement to our wait.
        let status = child.exited().map(|ended| ended.status());
        if let Some(moved) = self.attachment.take_departure() {
            if moved.moved && self.close_reason != CloseReason::Retired {
                self.close_reason = CloseReason::Replaced;
            }
            self.departure = Some(moved);
            if status.is_none() {
                // SAFETY: kill reads no memory. The child is the native client
                // this host serves and has not been reaped; its provider lives
                // separately and may already have been replaced.
                unsafe { libc::kill(child_pid as libc::pid_t, libc::SIGTERM) };
            }
        }
        if status.is_some() {
            self.attachment.set(None);
        }
        if self.viewers.iter().any(|viewer| viewer.ending.is_some())
            || (status.is_some() && self.exit_record.is_some())
        {
            if let Some(ended) = self.client.take() {
                self.forget_kept(NATIVE_CLIENT);
                self.finish_client(ended, child, answering);
            }
            for viewer in self.viewers.iter_mut() {
                let Some(answers) = viewer.ending.take() else {
                    continue;
                };
                viewer.reply(answers, Reply::NativeEnded);
                let _ = viewer.flush();
            }
        }
        if let Some(status) = status {
            end_jobs(child, &self.id)?;
            if let Some(record) = self.exit_record.take() {
                self.settling = Some(Settling::spawn(
                    record,
                    status,
                    self.waker.try_clone()?,
                    self.departure.take(),
                ));
            } else {
                // A launch that died before reporting its client's end.
                if let Some(held) = self.client.take().map(native::NativeClient::release) {
                    self.forget_kept(NATIVE_CLIENT);
                    self.deliver(1, &held, answering, Span::Aside { through: None });
                }
                self.exit = Some((provider_exit_code(status), Instant::now()));
            }
        }
        let delivered = self.settling.as_mut().and_then(|settling| {
            let settled = settling.poll()?;
            Some((settled, settling.process.clone()))
        });
        if let Some((settled, process)) = delivered {
            if settled.moved && self.close_reason != CloseReason::Retired {
                self.close_reason = CloseReason::Replaced;
            }
            // Recorded, or past waiting for: a later server settles it no more.
            self.forget_kept(EXIT_RECORD);
            for line in &settled.lines {
                self.deliver(
                    1,
                    format!("\r\n{line}\r\n").as_bytes(),
                    answering,
                    Span::Aside { through: None },
                );
            }
            self.output.report(
                format!("{process}/step"),
                loopflow_model::program_status::State::Clear,
                None,
            );
            self.output.report(
                process,
                match settled.code {
                    0 => loopflow_model::program_status::State::Done,
                    130 => loopflow_model::program_status::State::Idle,
                    _ => loopflow_model::program_status::State::Error,
                },
                None,
            );
            self.exit = Some((settled.code, Instant::now()));
        }
        // Include held native output and exit reports before announcing the end.
        if self.output.ready() && self.output.program_status() != self.reported_status.as_ref() {
            self.reported_status = self.output.program_status().cloned();
            if let Some(status) = &self.reported_status {
                for viewer in self.viewers.iter_mut().filter(|viewer| viewer.attached) {
                    viewer.event(Event::ProgramStatus {
                        status: status.clone(),
                    });
                }
                place_events(&self.output, &mut self.viewers, self.announced);
            }
        }
        if let Some((code, at)) = self.exit {
            if !self.announced && child.drained() && at.elapsed() > Duration::from_millis(50) {
                end_viewers(&mut self.viewers, Event::ChildExited { code });
                self.announced = true;
            }
            // Once viewers have received the final output, end the held
            // terminal and release its execution locks. Only never-viewed starts need a replay window.
            if self.announced
                && (at.elapsed() > Duration::from_secs(2)
                    || (self.attached
                        && self.viewers.iter().all(|viewer| viewer.output.is_empty())))
            {
                if !child.outcome_unknown() {
                    close_viewers(&mut self.viewers, self.close_reason);
                }
                release_viewer_hold(&self.id, self.observed_hold);
                snapshot::leave(&self.id, &self.output);
                return Ok(Pass::Ended(code, self.settling.take()));
            }
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let hold = viewer_hold(&self.viewers, self.observed_hold, now);
        if hold != self.observed_hold {
            self.observed_hold = hold;
            self.viewing.send(hold);
        }
        // Decide after all attaches and departures in this pass. The store's
        // observation can lag either, including a brief visit between sweeps.
        let grace_elapsed = match hold {
            Some(ViewerHold::Viewed(_)) => false,
            Some(ViewerHold::Absent(at)) => now >= at + loopflow_storage::sqlite::WORKSPACE_GRACE,
            None => true,
        };
        let notice = closes
            .iter()
            .rev()
            .find_map(|(_, _, text)| text.as_deref())
            .or_else(|| {
                (grace_elapsed && !closes.is_empty())
                    .then_some("Session closed after 15 minutes without a viewer")
            });
        let retiring = notice.is_some();
        if let Some(text) = notice {
            self.close_reason = CloseReason::Retired;
            self.deliver(
                1,
                format!("\r\n{text}\r\n").as_bytes(),
                answering,
                Span::Aside { through: None },
            );
            snapshot::leave(&self.id, &self.output);
        }
        for (requester, answers, _) in closes {
            if let Some(viewer) = self
                .viewers
                .iter_mut()
                .find(|viewer| viewer.id == requester)
            {
                viewer.reply(answers, Reply::Accepted);
                let _ = viewer.flush();
            }
        }
        if retiring {
            // A held terminal has no process to signal: the notice ends it.
            release_viewer_hold(&self.id, self.observed_hold);
            child.retire();
        }
        Ok(Pass::Continue)
    }

    /// A paste the departed writer left open ends with it.
    fn finish_departed_paste(&mut self) {
        let Some(writer) = &self.writer else {
            return;
        };
        if !self.viewers.iter().any(|v| v.live() && &v.id == writer) {
            self.writer = None;
            self.pending_input.finish_paste();
        }
    }

    fn deliver(&mut self, stream: u8, bytes: &[u8], answering: bool, span: Span) {
        deliver(
            &mut self.output,
            &mut self.viewers,
            &mut self.pending_input,
            stream,
            bytes,
            answering,
            span,
        );
    }

    /// Bring views that continued from a position to the live stream with
    /// PTY bytes from the holder's ring. One that reaches it receives the
    /// current state again; one the ring no longer reaches is painted afresh.
    fn catch_up(&mut self, child: &Source) -> Result<()> {
        let live = child.position();
        for viewer in self.viewers.iter_mut().filter(|v| v.catching_up) {
            let at = viewer.position;
            if at > live {
                // Ahead of a screen still being rebuilt from the ring: the
                // stream reaches it in time.
                viewer.catch_up = None;
                continue;
            }
            if at < live {
                if viewer.catch_up.is_none() {
                    viewer.catch_up = child.catch_up(at);
                }
                let caught = match &mut viewer.catch_up {
                    Some(catch_up) => catch_up.read(live),
                    None => Err(io::ErrorKind::NotFound.into()),
                };
                match caught {
                    Ok(bytes) => {
                        viewer.stream(Tag::Out, &bytes)?;
                        viewer.position = at + bytes.len() as u64;
                    }
                    Err(error) => {
                        tracing::debug!(%error, viewer = %viewer.id, "view cannot continue from its position; painting it afresh");
                        viewer.catch_up = None;
                        paint(&self.output, child, viewer)?;
                        viewer.event(Event::Live);
                        continue;
                    }
                }
            }
            if viewer.position == live {
                viewer.catch_up = None;
                viewer.catching_up = false;
                viewer.stream(Tag::Aside, &self.output.state())?;
                viewer.event(Event::Live);
            }
        }
        Ok(())
    }

    /// Drain and deliver what a finished native client still held, then its exit line.
    fn finish_client(
        &mut self,
        mut ended: native::NativeClient,
        child: &mut Source,
        answering: bool,
    ) {
        for chunk in child.read(LIMIT / 16384) {
            let ready = ended.accept(chunk.raw, &chunk.bytes);
            let span = Span::Aside {
                through: Some(chunk.end),
            };
            self.deliver(1, &ready, answering, span);
        }
        let raw = child.is_raw();
        let exit = ended.finish(raw, || child.restore_modes());
        self.deliver(1, &exit, answering, Span::Aside { through: None });
    }

    /// When this execution next needs a pass with no descriptor ready: held
    /// output, the exit announcement or a viewer's heartbeat.
    pub fn deadline(&self, child: &Source) -> Option<Instant> {
        let deadline = match self.exit {
            Some((_, at)) if self.announced => Some(at + Duration::from_secs(2)),
            Some((_, at)) if child.drained() => Some(at + Duration::from_millis(50)),
            _ => self
                .client
                .as_ref()
                .and_then(native::NativeClient::expires_at),
        };
        self.viewers
            .iter()
            .filter(|viewer| viewer.live())
            .map(|viewer| viewer.heartbeat_at + HEARTBEAT_TIMEOUT)
            .chain(deadline)
            .chain(Some(Instant::now() + Duration::from_secs(10)))
            .chain(child.retry_at())
            .chain(Some(self.title.deadline()))
            .chain(self.settling.as_ref().and_then(Settling::deadline))
            .min()
    }

    /// The descriptors whose readiness means this execution has work.
    pub fn poll(&self, child: &Source, fds: &mut Vec<libc::pollfd>) {
        for viewer in &self.viewers {
            let events = if viewer.draining || viewer.output.is_empty() {
                libc::POLLIN
            } else {
                libc::POLLIN | libc::POLLOUT
            };
            fds.push(libc::pollfd {
                fd: viewer.socket.as_raw_fd(),
                events,
                revents: 0,
            });
            if let Some(catch_up) = &viewer.catch_up {
                fds.push(libc::pollfd {
                    fd: catch_up.fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
        }
        child.poll(!self.pending_input.is_empty(), fds);
    }
}

/// The launcher's handed-off exit record, beside its held terminal.
pub(super) const EXIT_RECORD: &str = "exit-record.json";
/// The launcher's native client announcement, beside its held terminal.
const NATIVE_CLIENT: &str = "native-client.json";

/// What a launcher's `native_started` announced.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct NativeStarted {
    exit: String,
    detach: String,
}

/// What is kept in `file`; none when it is absent or unreadable.
pub(super) fn read_kept<T: serde::de::DeserializeOwned>(file: &std::path::Path) -> Option<T> {
    let bytes = std::fs::read(file).ok()?;
    serde_json::from_slice(&bytes)
        .inspect_err(
            |error| tracing::warn!(%error, file = %file.display(), "kept handoff unreadable"),
        )
        .ok()
}

/// What cannot be kept is still used by this server; only a server that
/// stops before then loses it.
fn keep(file: &std::path::Path, value: &impl serde::Serialize) {
    let draft = file.with_extension("json.draft");
    let written = serde_json::to_vec(value)
        .map_err(io::Error::from)
        .and_then(|bytes| std::fs::write(&draft, bytes))
        .and_then(|()| std::fs::rename(&draft, file));
    if let Err(error) = written {
        tracing::warn!(%error, file = %file.display(), "handoff not kept beside its terminal");
    }
}

pub(super) fn remove_kept(file: &std::path::Path) {
    if let Err(error) = std::fs::remove_file(file) {
        if error.kind() != io::ErrorKind::NotFound {
            tracing::warn!(%error, file = %file.display(), "kept handoff not removed");
        }
    }
}

/// Paint `viewer` the retained screen; it then stands at the PTY position
/// that screen covers.
fn paint(output: &output::Output, child: &Source, viewer: &mut Viewer) -> Result<()> {
    if let Some(snapshot) = output.snapshot()? {
        viewer.stream(Tag::Snapshot, &snapshot)?;
    }
    for (stream, bytes) in output.tail() {
        viewer.stream(Tag::stream(stream), bytes)?;
    }
    viewer.catching_up = false;
    viewer.place(child.position());
    Ok(())
}
