//! One hosted execution: its viewers, who may type and who sizes it, the
//! retained screen, input retention, the native client's return line and exit
//! settlement. `serve` hosts one in an owner process; the execution itself is
//! behind [`Child`].

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::process::ExitStatus;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use crate::harness::native_exit::NativeExit;

use super::input::PendingInput;
use super::wire::{Event, Incoming, Own, Reply, Request, Tag};
use super::{
    deliver, dimensions, end_viewers, lifetime, native, output, place_events, provider_exit_code,
    publish_reports, release_viewer_hold, selected_directory, snapshot, survive, viewer_hold,
    Connection, Host, Ownership, Settling, Span, Viewer, GRACE, HEARTBEAT_TIMEOUT, LIMIT,
    OUTPUT_ACTIVITY_PAUSE,
};

/// Output read from the execution, with the line discipline it was written under.
pub(super) struct Chunk {
    pub stream: u8,
    pub bytes: Vec<u8>,
    /// The terminal was raw (no line editing) when this was read.
    pub raw: bool,
    /// The PTY position after these bytes, for a held terminal.
    pub end: Option<u64>,
}

/// The execution a host relays: where its output comes from, where input and
/// size go, and how its end is learned.
pub(super) enum Child {
    /// A child of this process on its own PTY or pipes.
    Local(Local),
    /// A child a holder keeps on a PTY, reached through the holder's socket.
    Held(super::held::Source),
}

pub(super) struct Local {
    pub readers: Vec<(u8, File)>,
    pub input: Option<File>,
    /// The PTY's modes before any client ran.
    pub cooked: Option<libc::termios>,
    pub exit: mpsc::Receiver<io::Result<ExitStatus>>,
    pub pid: u32,
    pub shell_jobs: Option<crate::os_process::TerminalJobs>,
}

impl Child {
    fn pid(&self) -> u32 {
        match self {
            Self::Local(local) => local.pid,
            Self::Held(source) => source.pid(),
        }
    }

    /// Resize the terminal and ask its foreground program to repaint.
    fn resize(&mut self, rows: u16, cols: u16) -> io::Result<()> {
        match self {
            Self::Local(local) => {
                let fd = local
                    .input
                    .as_ref()
                    .expect("terminal input stays open")
                    .as_raw_fd();
                super::resize_foreground(fd, rows, cols)
            }
            Self::Held(source) => {
                source.resize(rows, cols);
                Ok(())
            }
        }
    }

    fn forward(&mut self, pending: &mut PendingInput, owner: &Host, record: bool) {
        match self {
            Self::Local(local) => {
                if let Some(input) = local.input.as_mut() {
                    pending.forward(input, owner, record);
                }
            }
            Self::Held(source) => source.forward(pending, owner, record),
        }
    }

    fn close_input(&mut self) {
        match self {
            Self::Local(local) => {
                local.input.take();
            }
            Self::Held(source) => source.close_input(),
        }
    }

    /// Output that is ready, at most `passes` buffers per stream, so control
    /// stays responsive under continuous output.
    fn read(&mut self, passes: usize) -> io::Result<Vec<Chunk>> {
        match self {
            Self::Local(local) => {
                let mut chunks = Vec::new();
                let mut drained = Vec::new();
                for (stream, reader) in &mut local.readers {
                    let mut bytes = [0; 16384];
                    for _ in 0..passes {
                        let n = match reader.read(&mut bytes) {
                            Ok(0) => {
                                drained.push(*stream);
                                break;
                            }
                            Ok(n) => n,
                            Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                                drained.push(*stream);
                                break;
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                            Err(e) => return Err(e),
                        };
                        chunks.push(Chunk {
                            stream: *stream,
                            bytes: bytes[..n].to_vec(),
                            raw: super::is_raw(reader.as_raw_fd()),
                            end: None,
                        });
                    }
                }
                local
                    .readers
                    .retain(|(stream, _)| !drained.contains(stream));
                Ok(chunks)
            }
            Self::Held(source) => Ok(source.read(passes)),
        }
    }

    /// The PTY position everything read so far ends at, for a held terminal.
    fn position(&self) -> Option<u64> {
        match self {
            Self::Local(_) => None,
            Self::Held(source) => Some(source.position()),
        }
    }

    /// Read the output a view missed from `from` on; `None` paints it afresh.
    fn catch_up(&self, from: u64) -> Option<super::held::CatchUp> {
        match self {
            Self::Local(_) => None,
            Self::Held(source) => source.catch_up(from),
        }
    }

    /// Whether every output stream has ended.
    fn drained(&self) -> bool {
        match self {
            Self::Local(local) => local.readers.is_empty(),
            Self::Held(source) => source.drained(),
        }
    }

    fn is_raw(&self) -> bool {
        match self {
            Self::Local(local) => local
                .readers
                .first()
                .is_some_and(|(_, reader)| super::is_raw(reader.as_raw_fd())),
            Self::Held(source) => source.is_raw(),
        }
    }

    /// Return a terminal a client left raw to the modes it had before.
    fn restore_modes(&mut self) {
        match self {
            Self::Local(local) => {
                if let (Some((_, reader)), Some(cooked)) = (local.readers.first(), &local.cooked) {
                    // SAFETY: restores attributes read from this terminal before the client ran.
                    unsafe {
                        libc::tcsetattr(reader.as_raw_fd(), libc::TCSANOW, cooked);
                    }
                }
            }
            Self::Held(source) => source.restore_modes(),
        }
    }

    /// The execution's exit, reported once.
    fn exited(&mut self) -> io::Result<Option<ExitStatus>> {
        match self {
            Self::Local(local) => local.exit.try_recv().ok().transpose(),
            Self::Held(source) => Ok(source.exited().map(|ended| ended.status())),
        }
    }

    /// End what the execution left running in its terminal.
    fn end_jobs(&self, owner: &Host) -> Result<()> {
        match self {
            Self::Local(local) => {
                if let (Some(jobs), Some(owner)) = (&local.shell_jobs, owner.owner()) {
                    jobs.terminate()?;
                    crate::journal::process_ledger()?.settle_shell_jobs(owner)?;
                }
                Ok(())
            }
            Self::Held(_) => Ok(()),
        }
    }

    /// End the execution because nobody needs it any longer.
    fn retire(&self) {
        match self {
            // The owner process is the execution's whole lifetime.
            Self::Local(_) => crate::platform::kill_process(std::process::id()),
            // The holder reports the exit; this host ends when it arrives.
            Self::Held(source) => source.retire(),
        }
    }

    fn poll(&self, input: bool, fds: &mut Vec<libc::pollfd>) {
        let pollfd = |fd: RawFd, events: libc::c_short| libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        match self {
            Self::Local(local) => {
                for (_, reader) in &local.readers {
                    fds.push(pollfd(reader.as_raw_fd(), libc::POLLIN));
                }
                if let Some(writer) = local.input.as_ref().filter(|_| input) {
                    fds.push(pollfd(writer.as_raw_fd(), libc::POLLOUT));
                }
            }
            Self::Held(source) => source.poll(input, fds),
        }
    }

    /// When a lost connection to the execution is tried again.
    fn retry_at(&self) -> Option<Instant> {
        match self {
            Self::Local(_) => None,
            Self::Held(source) => source.retry_at(),
        }
    }
}

/// What one pass over a hosted execution came to.
pub(super) enum Pass {
    Continue,
    /// The execution ended with this code and its viewers have its last
    /// output. Records still being written come back to be waited for.
    Ended(u8, Option<Settling>),
}

pub(super) struct Hosted {
    id: Host,
    terminal: bool,
    /// The Desktop Task shell this execution is.
    shell: Option<String>,
    /// What the execution's own control requests present.
    control_key: String,
    viewers: Vec<Viewer>,
    /// Who may type.
    control: Option<String>,
    ownership: Ownership,
    size: (u16, u16),
    pending_input: PendingInput,
    end_input: bool,
    ending_failure: crate::store::RecordingGap,
    output: output::Output,
    exit: Option<(u8, Instant)>,
    detach: bool,
    client: Option<native::NativeClient>,
    returning: Option<Event>,
    hint_due: bool,
    announced: bool,
    attached: bool,
    activity: mpsc::Sender<lifetime::Activity>,
    ending: mpsc::Receiver<lifetime::Ending>,
    /// Wakes the host's descriptor wait from another thread.
    waker: UnixStream,
    settling: Option<Settling>,
    observed: lifetime::Activity,
    output_told: Option<Instant>,
    exit_record: Option<Box<NativeExit>>,
    execution_directory_reported: bool,
    planning: Option<crate::ops::planning_sync::PlanningSync>,
    title: Option<crate::terminal_title::OutputTitle>,
    /// A taken-up terminal's output through this PTY position is replayed
    /// from the ring: its queries were answered when it was printed, or
    /// went unanswered while no server ran. Nothing answers them now.
    replayed_through: u64,
}

impl Hosted {
    /// Host the execution known as `id`. `waker` wakes the host's wait when
    /// the lifetime policy or a settlement has something for it.
    pub fn new(
        id: Host,
        terminal: bool,
        shell: Option<String>,
        control_key: String,
        waker: UnixStream,
    ) -> Result<Self> {
        let (activity, ending) = lifetime::spawn(id.clone(), terminal, waker.try_clone()?);
        Ok(Self {
            terminal,
            shell,
            control_key,
            viewers: Vec::new(),
            control: None,
            ownership: Ownership::default(),
            size: (24, 80),
            pending_input: PendingInput::default(),
            end_input: false,
            ending_failure: crate::store::RecordingGap::default(),
            output: output::Output::new(terminal)?,
            exit: None,
            detach: false,
            client: None,
            returning: None,
            hint_due: false,
            announced: false,
            attached: false,
            activity,
            ending,
            waker,
            settling: None,
            observed: lifetime::Activity::default(),
            output_told: None,
            exit_record: None,
            execution_directory_reported: false,
            planning: None,
            title: terminal.then(|| crate::terminal_title::OutputTitle::new(&id)),
            replayed_through: 0,
            id,
        })
    }

    /// Let a test stage a connection or output before the host accepts and reads.
    #[cfg(test)]
    pub fn before_accept(&mut self, child: &mut Child) -> Result<()> {
        super::input_retry_tests::poll();
        let (Child::Local(local), Some(owner)) = (child, self.id.owner()) else {
            return Ok(());
        };
        super::lifetime_tests::before_read(owner, &mut local.readers, &mut self.output_told)
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

    /// The rows and columns the execution is drawn at.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// A connection that may become a viewer or carry the execution's requests.
    pub fn admit(&mut self, socket: UnixStream) {
        let mut viewer = Viewer::new(socket);
        if matches!(self.id, Host::Terminal(_)) {
            viewer.position = Some(0);
        }
        self.viewers.push(viewer);
    }

    /// Serve every connection, move input and output, and judge the ending.
    pub fn pass(&mut self, child: &mut Child) -> Result<Pass> {
        let terminal = self.terminal;
        let child_pid = child.pid();
        let mut replaced = Vec::new();
        let mut owner_change = false;
        let mut size_change = false;
        let mut theme_change = false;
        let mut attaching = Vec::new();
        // Observe every connection before admitting claims. A viewer earlier
        // in the list must not see Busy for a controller whose EOF is ready.
        self.viewers.retain_mut(|viewer| {
            viewer.expire_heartbeat(Instant::now());
            match viewer.read() {
                Ok(()) => !viewer.connection.expired(),
                Err(error) => survive(viewer, &error),
            }
        });
        owner_change |= self.ownership.settle(&self.viewers);
        self.release_lost_control();
        {
            let Self {
                viewers,
                control,
                control_key,
                ownership,
                pending_input,
                end_input,
                output,
                detach,
                client,
                returning,
                hint_due,
                attached,
                exit_record,
                execution_directory_reported,
                planning,
                ..
            } = self;
            viewers.retain_mut(|viewer| {
                let result = (|| -> Result<()> {
                    while let Some((tag, payload)) = viewer.decoder.next()? {
                        let request = match tag {
                            Tag::Input => {
                                // Stale and passive viewers cannot write.
                                if viewer.token.is_some() && viewer.token == *control && !*end_input
                                {
                                    anyhow::ensure!(
                                        pending_input.push(&payload),
                                        "Provider input is busy"
                                    );
                                }
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
                            } if key == *control_key && terminal => {
                                *returning = Some(Event::Return {
                                    detach: line.clone(),
                                });
                                *client = Some(native::NativeClient::new(exit, line));
                            }
                            Request::NativeEnded { key } if key == *control_key => {
                                viewer.ending = Some(answers);
                            }
                            Request::InputHint { key, hint } if key == *control_key => {
                                viewer.input_hint = Some(hint);
                                *hint_due = true;
                            }
                            Request::Controlled => {
                                if viewer.attached {
                                    viewer.heartbeat_at = Instant::now();
                                }
                                viewer.reply(
                                    answers,
                                    Reply::Controlled {
                                        held: control.is_some(),
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
                                // The launcher kept its repository's planning in
                                // sync while it waited; that wait is now ours.
                                *planning =
                                    crate::ops::planning_sync::PlanningSync::start_for_directory(
                                        &record.cwd,
                                    )
                                    .inspect_err(|error| {
                                        tracing::warn!(%error, "planning sync unavailable to this owner")
                                    })
                                    .ok();
                                *exit_record = Some(record);
                                viewer.reply(answers, Reply::Accepted);
                            }
                            Request::DropExitRecord { key } if key == *control_key => {
                                *exit_record = None;
                                *planning = None;
                                viewer.reply(answers, Reply::Accepted);
                            }
                            Request::Attach {
                                control: wants_control,
                                replace,
                                own,
                                rows,
                                cols,
                                name,
                                moved,
                                from,
                            } => {
                                if wants_control
                                    && !replace
                                    && control.is_some()
                                    && viewer.token != *control
                                {
                                    viewer.reply(answers, Reply::Busy);
                                    continue;
                                }
                                *attached = true;
                                viewer.size = dimensions(rows, cols);
                                viewer.name = name;
                                // Only a held terminal's viewer counts PTY positions.
                                viewer.from = from.filter(|_| viewer.position.is_some());
                                if wants_control {
                                    // A view continuing its screen showed the hint already.
                                    *hint_due |= viewer.from.is_none();
                                    if viewer.token != *control {
                                        if let Some(token) = &*control {
                                            replaced.push((token.clone(), moved));
                                        }
                                    }
                                    let token = uuid::Uuid::new_v4().to_string();
                                    *control = Some(token.clone());
                                    viewer.token = Some(token);
                                } else {
                                    if viewer.token.is_some() && viewer.token == *control {
                                        *control = None;
                                    }
                                    viewer.token = None;
                                }
                                viewer.attached = true;
                                viewer.heartbeat_at = Instant::now();
                                attaching.push(viewer.id.clone());
                                owner_change |= ownership.claim(own, viewer);
                                viewer
                                    .reply(answers, viewer.attached_reply(ownership.id(), terminal));
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
                            Request::Resume(resume) => {
                                viewer.resuming = Some((answers, resume));
                            }
                            Request::EndInput
                                if !terminal
                                    && viewer.token.is_some()
                                    && viewer.token == *control =>
                            {
                                *end_input = true;
                            }
                            Request::Resize { rows, cols } => {
                                viewer.size = dimensions(rows, cols);
                                size_change |= ownership.held_by(viewer);
                            }
                            Request::Own { mode } if viewer.attached => {
                                owner_change |= ownership.claim(Some(mode), viewer);
                            }
                            Request::Theme { theme } if viewer.attached => {
                                viewer.theme = Some(theme);
                                theme_change |= ownership.held_by(viewer);
                            }
                            Request::Leave => {
                                viewer.leaving = true;
                            }
                            _ => {} // stale and passive viewers cannot release another claim
                        }
                    }
                    viewer.flush()?;
                    Ok(())
                })();
                match result {
                    Ok(()) => !viewer.leaving,
                    Err(error) => error
                        .downcast_ref()
                        .is_some_and(|error| survive(viewer, error)),
                }
            });
        }
        // Release dropped connections' claims before a same-pass resume asks
        // for them again; reconnecting restores the stream, not its old claims.
        self.viewers.retain(|viewer| !viewer.connection.expired());
        owner_change |= self.ownership.settle(&self.viewers);
        self.release_lost_control();
        // A resume moves its connection onto the dropped viewer's stream.
        for index in 0..self.viewers.len() {
            let Some((answers, resume)) = self.viewers[index].resuming.take() else {
                continue;
            };
            let target = self.viewers.iter().position(|viewer| {
                viewer.id != self.viewers[index].id
                    && viewer.reconnect == resume.reconnect
                    && viewer.spool.holds(resume.received)
            });
            let Some(target) = target else {
                self.viewers[index].reply(answers, Reply::ReconnectRequired);
                let _ = self.viewers[index].flush();
                continue;
            };
            self.viewers[index].leaving = true;
            // A resuming peer that already closed has nothing to continue on.
            let Connection::Open(socket) = std::mem::replace(
                &mut self.viewers[index].connection,
                Connection::Dropped(Instant::now()),
            ) else {
                continue;
            };
            let viewer = &mut self.viewers[target];
            viewer.size = dimensions(resume.rows, resume.cols);
            size_change |= self.ownership.held_by(viewer);
            if resume.control && self.control.is_none() {
                let token = uuid::Uuid::new_v4().to_string();
                self.control = Some(token.clone());
                viewer.token = Some(token);
                self.hint_due = true;
            }
            // Recovery can reclaim free geometry, never displace a newer viewer.
            owner_change |= self
                .ownership
                .claim(resume.own.map(|_| Own::Request), viewer);
            viewer.resume(
                socket,
                answers,
                resume.received,
                self.ownership.id(),
                terminal,
            );
        }
        self.viewers.retain(|viewer| !viewer.leaving);
        owner_change |= self.ownership.settle(&self.viewers);
        let owner = self
            .viewers
            .iter()
            .find(|viewer| self.ownership.held_by(viewer));
        let new_size = owner
            .filter(|_| owner_change || size_change)
            .map(|viewer| viewer.size);
        let theme = owner
            .filter(|_| owner_change || theme_change || !attaching.is_empty())
            .and_then(|viewer| viewer.theme.clone());
        if let Some((rows, cols)) = new_size {
            self.size = (rows, cols);
            if terminal {
                child.resize(rows, cols)?;
                self.output.resize(rows, cols)?;
            }
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
                    viewer.position = Some(from);
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
                let event = self.ownership.event(viewer);
                viewer.event(event);
            }
            if fresh {
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
        // The viewer that held control learns where it went; LOO-618's viewer
        // leaves on that, a shared viewer may stay.
        for (token, notice) in replaced {
            for viewer in self
                .viewers
                .iter_mut()
                .filter(|viewer| viewer.token.as_ref() == Some(&token))
            {
                viewer.token = None;
                viewer.event(Event::Control {
                    held: false,
                    notice: notice.clone(),
                });
            }
        }
        if let Some(event) = self.returning.take() {
            for viewer in self.viewers.iter_mut().filter(|viewer| viewer.attached) {
                viewer.event(event.clone());
            }
        }
        // The controls reach the one controlling viewer when it attaches or
        // when the execution registers them, whichever comes later.
        if std::mem::take(&mut self.hint_due) {
            let hint = self.viewers.iter().rev().find_map(|v| v.input_hint.clone());
            if let Some(hint) = hint {
                for viewer in self
                    .viewers
                    .iter_mut()
                    .filter(|v| v.attached && v.token.is_some() && v.token == self.control)
                {
                    viewer.event(Event::Hint { text: hint.clone() });
                }
            }
        }
        let detached = !self
            .viewers
            .iter()
            .any(|v| v.token.is_some() && v.token == self.control);
        if detached {
            self.control = None;
        }
        // A controlling view still catching up throws away what its terminal
        // answers; the retained screen answers live queries meanwhile.
        let answering = !self
            .viewers
            .iter()
            .any(|v| v.token.is_some() && v.token == self.control && !v.catching_up);
        if let Some(name) = self.title.as_mut().and_then(|title| title.tick()) {
            self.output.title(&name);
        }
        // Typing at an idle shell prompt is not an agent draft: retain only
        // input that work running inside the shell could receive.
        let record = terminal
            && (self.shell.is_none()
                || self.pending_input.is_empty()
                || self
                    .id
                    .owner()
                    .is_none_or(crate::desktop::shells::holds_input));
        child.forward(&mut self.pending_input, &self.id, record);
        if self.end_input && self.pending_input.is_empty() {
            child.close_input();
        }
        for chunk in child.read(16)? {
            self.observed.output += 1;
            // An exec handoff announces before exec. The provider's first
            // output establishes that it reached its configured directory.
            if !self.execution_directory_reported {
                if let Some(record) = &self.exit_record {
                    let connected = crate::journal::process_ledger()
                        .and_then(|store| store.agent_process(&record.attachment.agent_process_id))
                        .ok()
                        .flatten()
                        .is_some_and(|process| {
                            process.pid.is_some() && process.completed_at.is_none()
                        });
                    if connected {
                        self.output.directory(&record.cwd);
                        self.execution_directory_reported = true;
                    }
                }
            }
            let answering = answering && chunk.end.is_none_or(|end| end > self.replayed_through);
            let pty = chunk.end.map(|end| Span::Pty {
                start: end - chunk.bytes.len() as u64,
                end,
            });
            let aside = Span::Aside { through: chunk.end };
            let (ready, span) = match self.client.as_mut().filter(|_| terminal) {
                Some(client) => {
                    // A native client that changed the output sends lf's text.
                    let ready = client.accept(chunk.raw, &chunk.bytes);
                    let span = pty.filter(|_| ready == chunk.bytes).unwrap_or(aside);
                    (ready, span)
                }
                None => (chunk.bytes, pty.unwrap_or(aside)),
            };
            self.deliver(chunk.stream, &ready, answering, span);
            if self
                .output_told
                .is_none_or(|told| told.elapsed() >= OUTPUT_ACTIVITY_PAUSE)
            {
                self.output_told = Some(Instant::now());
                let _ = self.activity.send(self.observed);
            }
        }
        self.catch_up(child)?;
        // After the reads: a provider that prints once and waits wakes nothing further.
        publish_reports(&mut self.output, &mut self.viewers);
        place_events(&self.output, &mut self.viewers, self.announced);
        // Output written before the execution detached reaches its viewers first.
        if self.detach {
            self.control = None;
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
        let status = child.exited()?;
        if self.viewers.iter().any(|viewer| viewer.ending.is_some())
            || (status.is_some() && self.exit_record.is_some())
        {
            self.pending_input.native_ended(&self.id);
            if let Some(ended) = self.client.take() {
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
            child.end_jobs(&self.id)?;
            if let Some(record) = self.exit_record.take() {
                self.settling = Some(Settling::spawn(record, status, self.waker.try_clone()?));
            } else {
                // A launch that died before reporting its client's end.
                if let Some(held) = self.client.take().map(native::NativeClient::release) {
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
                crate::program_status::State::Clear,
                None,
            );
            self.output.report(
                process,
                match settled.code {
                    0 => crate::program_status::State::Done,
                    130 => crate::program_status::State::Idle,
                    _ => crate::program_status::State::Error,
                },
                None,
            );
            self.planning = None;
            self.exit = Some((settled.code, Instant::now()));
        }
        if let Some((code, at)) = self.exit {
            if !self.announced && child.drained() && at.elapsed() > Duration::from_millis(50) {
                end_viewers(&mut self.viewers, Event::ChildExited { code });
                self.announced = true;
            }
            // Once viewers have received the final output, release execution
            // locks with the owner. Only never-viewed starts need a replay window.
            if self.announced
                && (at.elapsed() > Duration::from_secs(2)
                    || (self.attached
                        && self.viewers.iter().all(|viewer| viewer.output.is_empty())))
            {
                release_viewer_hold(&self.id, self.observed.hold);
                snapshot::leave(&self.id, &self.output);
                return Ok(Pass::Ended(code, self.settling.take()));
            }
        }
        // Ready output and viewer changes precede the final ending judgment.
        // A throttled/unconsumed activity report also invalidates the candidate.
        let viewed = self.observed.view(self.viewers.iter().any(|v| v.live()));
        if self.observed.hold(viewer_hold(
            &self.viewers,
            self.observed.hold,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )) | viewed
        {
            let _ = self.activity.send(self.observed);
        }
        #[cfg(test)]
        super::lifetime_tests::before_judgment();
        if let Ok(ending) = self.ending.try_recv() {
            let event = lifetime::report_result(
                &mut self.ending_failure,
                "Session ending",
                ending.confirm(&self.id, self.observed, self.pending_input.holds_owner()),
            )
            .flatten();
            if let Some(event) = event {
                child.end_jobs(&self.id)?;
                end_viewers(&mut self.viewers, event);
                release_viewer_hold(&self.id, self.observed.hold);
                snapshot::leave(&self.id, &self.output);
                child.retire();
            }
            // Flush even throttled activity so the applicable quiet interval
            // starts again and a settled owner can eventually end.
            if ending.activity != self.observed {
                let _ = self.activity.send(self.observed);
            }
        }
        #[cfg(test)]
        super::lifetime_tests::after_judgment(&self.output, self.observed)?;
        Ok(Pass::Continue)
    }

    /// Nobody types once the viewer holding the claim is gone.
    fn release_lost_control(&mut self) {
        if !self
            .viewers
            .iter()
            .any(|viewer| viewer.token.is_some() && viewer.token == self.control)
        {
            self.control = None;
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
    fn catch_up(&mut self, child: &Child) -> Result<()> {
        let Some(live) = child.position() else {
            return Ok(());
        };
        for viewer in self.viewers.iter_mut().filter(|v| v.catching_up) {
            let at = viewer
                .position
                .expect("a catching-up viewer counts positions");
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
                        viewer.position = Some(at + bytes.len() as u64);
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
            if viewer.position == Some(live) {
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
        child: &mut Child,
        answering: bool,
    ) {
        if let Ok(chunks) = child.read(LIMIT / 16384) {
            for chunk in chunks {
                let ready = ended.accept(chunk.raw, &chunk.bytes);
                let span = Span::Aside { through: chunk.end };
                self.deliver(chunk.stream, &ready, answering, span);
            }
        }
        let raw = child.is_raw();
        let exit = ended.finish(raw, || child.restore_modes());
        self.deliver(1, &exit, answering, Span::Aside { through: None });
    }

    /// When this execution next needs a pass with no descriptor ready: held
    /// output, the exit announcement, input retention or a disconnected
    /// viewer's grace.
    pub fn deadline(&self, child: &Child) -> Option<Instant> {
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
            .filter_map(|viewer| match viewer.connection {
                Connection::Dropped(at) => Some(at + GRACE),
                Connection::Open(_) | Connection::Draining(_) if viewer.live() => {
                    Some(viewer.heartbeat_at + HEARTBEAT_TIMEOUT)
                }
                Connection::Open(_) | Connection::Draining(_) => None,
            })
            .chain(deadline)
            .chain(Some(Instant::now() + Duration::from_secs(10)))
            .chain(self.pending_input.retry_at())
            .chain(child.retry_at())
            .chain(self.title.as_ref().map(|title| title.deadline()))
            .chain(self.settling.as_ref().and_then(Settling::deadline))
            .min()
    }

    /// The descriptors whose readiness means this execution has work.
    pub fn poll(&self, child: &Child, fds: &mut Vec<libc::pollfd>) {
        for viewer in &self.viewers {
            let (socket, events) = match &viewer.connection {
                Connection::Open(socket) if !viewer.output.is_empty() => {
                    (socket, libc::POLLIN | libc::POLLOUT)
                }
                Connection::Open(socket) | Connection::Draining(socket) => (socket, libc::POLLIN),
                Connection::Dropped(_) => continue,
            };
            fds.push(libc::pollfd {
                fd: socket.as_raw_fd(),
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
        child.poll(self.pending_input.needs_write(), fds);
    }
}

/// Paint `viewer` the retained screen; a held terminal's viewer then stands
/// at the PTY position that screen covers.
fn paint(output: &output::Output, child: &Child, viewer: &mut Viewer) -> Result<()> {
    if let Some(snapshot) = output.snapshot()? {
        viewer.stream(Tag::Snapshot, &snapshot)?;
    }
    for (stream, bytes) in output.tail() {
        viewer.stream(Tag::stream(stream), bytes)?;
    }
    viewer.catching_up = false;
    if let (Some(at), Some(_)) = (child.position(), viewer.position) {
        viewer.place(at);
    }
    Ok(())
}
