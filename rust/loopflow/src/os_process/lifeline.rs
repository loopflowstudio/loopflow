//! Start the watchdog before execing an agent, never after spawning it.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

// stdout acknowledges watchdog startup, not provider output. The watchdog has
// its own group so it survives provider shutdown and can deliver the final KILL.
// dash needs `kill -s TERM -- -pgid`, not `kill -TERM -- -pgid`.
const WATCHDOG: &std::ffi::CStr = cr#"printf .; exec 1>&-
while read -r _; do :; done
trap '' TERM
kill -s TERM -- "-$1" 2>/dev/null
sleep 2
kill -s KILL -- "-$1" 2>/dev/null"#;

/// Spawn a headless AgentProcess with its watchdog already running. Consume the
/// command so its inherited descriptors cannot outlive this spawn or be reused.
/// Record its OS identity before provider code executes; a failed record refuses
/// exec. A successful launch holds the lifeline until this lf invocation exits.
pub(crate) fn spawn_agent_process(
    mut command: tokio::process::Command,
    path: &Path,
    record: impl FnOnce(u32) -> std::io::Result<()> + Send,
) -> std::io::Result<tokio::process::Child> {
    let (reader, writer) = open_lifeline(path)?;
    let null = above_stdio(File::options().write(true).open("/dev/null")?)?;
    command.process_group(0);
    let writer_fd = writer.as_raw_fd();
    let custody = AgentProcessCustody::new(writer)?;
    let recording = SpawnRecording::prepare(command.as_std_mut(), move || {
        // SAFETY: this hook runs only after fork. Neither provider nor watchdog
        // may keep the parent's lifeline writer open.
        unsafe { libc::close(writer_fd) };
        start_watchdog(reader.as_raw_fd(), null.as_raw_fd())
    })?;
    let mut identity = None;
    let child = recording.spawn(
        |pid| {
            let started = crate::journal::process_started_at(pid)?.ok_or_else(|| {
                std::io::Error::other("AgentProcess birth unavailable before exec")
            })?;
            record(pid)?;
            identity = Some((pid, started));
            Ok(())
        },
        move || {
            let spawned = command.spawn();
            drop(command);
            spawned
        },
    )?;
    let (pid, started) = identity.expect("spawned provider was recorded before exec");
    custody.retain(pid, started);
    Ok(child)
}

/// Record a foreground provider before exec without changing its inherited
/// process group, controlling terminal, stdio or signal behavior.
pub(crate) fn spawn_native_agent_process(
    mut command: std::process::Command,
    record: impl FnOnce(u32) -> std::io::Result<()> + Send,
) -> std::io::Result<std::process::Child> {
    let recording = SpawnRecording::prepare(&mut command, || Ok(()))?;
    recording.spawn(record, move || {
        let spawned = command.spawn();
        drop(command);
        spawned
    })
}

/// Parent-side pre-exec recording channel. Consuming the command after spawn
/// closes its child endpoint even when an earlier pre-exec hook fails.
struct SpawnRecording(File);

impl SpawnRecording {
    fn prepare(
        command: &mut std::process::Command,
        before_record: impl FnMut() -> std::io::Result<()> + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        use std::os::unix::process::CommandExt;

        let (parent, child) = std::os::unix::net::UnixStream::pair()?;
        let parent = above_stdio(File::from(OwnedFd::from(parent)))?;
        let child = above_stdio(File::from(OwnedFd::from(child)))?;
        let parent_fd = parent.as_raw_fd();
        let mut before_record = before_record;
        // SAFETY: only async-signal-safe syscalls run in the child. The parent
        // thread owns the recorder, including allocation, locking and SQLite.
        unsafe {
            command.pre_exec(move || {
                libc::close(parent_fd);
                before_record()?;
                let pid = libc::getpid() as u32;
                transfer(child.as_raw_fd(), &mut pid.to_ne_bytes(), true)?;
                await_ready(child.as_raw_fd())
            });
        }
        Ok(Self(parent))
    }

    fn spawn<T>(
        self,
        record: impl FnOnce(u32) -> std::io::Result<()> + Send,
        spawn: impl FnOnce() -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        // Command::spawn waits for exec; recording on its thread would deadlock.
        std::thread::scope(|scope| {
            let mut parent = self.0;
            let recorder = std::thread::Builder::new()
                .name("agent-process-record".into())
                .spawn_scoped(scope, move || {
                    let mut pid = [0u8; 4];
                    match parent.read_exact(&mut pid) {
                        Ok(()) => {}
                        // Spawn failed before our hook; retain the spawn error.
                        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                            return Ok(())
                        }
                        Err(error) => return Err(error),
                    }
                    record(u32::from_ne_bytes(pid))?;
                    parent.write_all(b".")
                })?;
            let spawned = spawn();
            recorder
                .join()
                .map_err(|_| std::io::Error::other("AgentProcess recorder panicked"))??;
            spawned
        })
    }
}

/// Complete a tiny pre-exec handshake without allocating or using Rust locks.
fn transfer(fd: libc::c_int, bytes: &mut [u8], write: bool) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: callers supply a live buffer and an owned
        // descriptor. The offset remains within the buffer; both calls are
        // async-signal-safe and neither retains the pointer.
        let result = unsafe {
            if write {
                libc::write(
                    fd,
                    bytes.as_mut_ptr().add(offset).cast(),
                    bytes.len() - offset,
                )
            } else {
                libc::read(
                    fd,
                    bytes.as_mut_ptr().add(offset).cast(),
                    bytes.len() - offset,
                )
            }
        };
        if result > 0 {
            offset += result as usize;
        } else if result == 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EIO));
        } else {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn open_lifeline(path: &Path) -> std::io::Result<(File, File)> {
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    // SAFETY: name is a valid NUL-terminated path.
    if unsafe { libc::mkfifo(name.as_ptr(), 0o600) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let reader = above_stdio(
        File::options()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?,
    )?;
    let writer = above_stdio(File::options().write(true).open(path)?)?;
    // SAFETY: reader is owned here; the watchdog needs blocking read.
    if unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((reader, writer))
}

/// A prospective attachment owns only this writer until its claim succeeds.
/// Dropping a failed claim cannot release another invocation's custody.
#[derive(Debug)]
pub(crate) struct AgentProcessCustody(std::sync::mpsc::Sender<(u32, i64)>);

impl AgentProcessCustody {
    fn new(writer: File) -> std::io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("agent-custody".into())
            .spawn(move || {
                // Until a successful claim, dropping the guard releases this writer.
                let Ok((pid, started)) = receiver.recv() else {
                    return;
                };
                loop {
                    // Leader death alone cannot release surviving helpers. The
                    // watchdog is outside this group so it cannot keep its own
                    // custody alive after the complete provider group exits.
                    if crate::journal::process_identity_evidence(pid, started)
                        == crate::journal::ProcessIdentityEvidence::Dead
                        && matches!(crate::journal::OsProcess::group_is_alive(pid), Ok(false))
                    {
                        break;
                    }
                    let mut fd = libc::pollfd {
                        fd: writer.as_raw_fd(),
                        events: 0,
                        revents: 0,
                    };
                    // SAFETY: poll borrows one initialized descriptor for this call.
                    let result = unsafe { libc::poll(&mut fd, 1, 250) };
                    if result > 0
                        && fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
                    {
                        break;
                    }
                    if result < 0 {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            })?;
        Ok(Self(sender))
    }

    /// Keep a non-writing standby through harness teardown and attachment loss.
    /// The OS closes it on lf exit; confirmed group death releases it earlier.
    pub(crate) fn retain(self, pid: u32, started: i64) {
        self.0
            .send((pid, started))
            .expect("custody worker waits for its owner");
    }
}

// Command installs native stdio before pre_exec. An invocation with a closed
// standard descriptor must not let that overwrite a lifeline descriptor.
fn above_stdio(file: File) -> std::io::Result<File> {
    if file.as_raw_fd() > libc::STDERR_FILENO {
        return Ok(file);
    }
    // SAFETY: duplicate an owned descriptor; a successful call transfers the
    // newly allocated descriptor to File exactly once.
    unsafe {
        let fd = libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3);
        if fd == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(File::from_raw_fd(fd))
    }
}

/// Called between fork and exec, after Command has established the new group.
fn start_watchdog(reader: libc::c_int, null: libc::c_int) -> std::io::Result<()> {
    // SAFETY: all descriptors and buffers are local, and all operations below
    // are async-signal-safe. The watchdog either execs /bin/sh or calls _exit.
    unsafe {
        let mut ready = [-1; 2];
        if libc::pipe(ready.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let ready = ready.map(|fd| OwnedFd::from_raw_fd(fd));
        for fd in &ready {
            if libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }
        let provider_group = libc::getpgrp() as u32;
        let pid = libc::fork();
        if pid == -1 {
            return Err(std::io::Error::last_os_error());
        }
        if pid == 0 {
            // The watchdog must not count as a surviving provider helper when
            // custody observes group death. It still targets the captured group.
            if libc::setpgid(0, 0) != 0 {
                libc::_exit(127);
            }
            libc::close(ready[0].as_raw_fd());
            if libc::dup2(reader, libc::STDIN_FILENO) == -1
                || libc::dup2(ready[1].as_raw_fd(), libc::STDOUT_FILENO) == -1
                || libc::dup2(null, libc::STDERR_FILENO) == -1
            {
                libc::_exit(127);
            }
            // Format the inherited group ID without an allocator. The final
            // byte stays NUL; a Unix pid fits in ten decimal digits.
            let mut group = provider_group;
            let mut digits = [0u8; 11];
            let mut start = 10;
            loop {
                start -= 1;
                digits[start] = b'0' + (group % 10) as u8;
                group /= 10;
                if group == 0 {
                    break;
                }
            }
            let argv = [
                c"/bin/sh".as_ptr(),
                c"-c".as_ptr(),
                WATCHDOG.as_ptr(),
                c"lf-agent-lifeline".as_ptr(),
                digits.as_ptr().add(start).cast(),
                std::ptr::null(),
            ];
            let env = [c"PATH=/usr/bin:/bin".as_ptr(), std::ptr::null()];
            libc::execve(c"/bin/sh".as_ptr(), argv.as_ptr(), env.as_ptr());
            libc::_exit(127);
        }
        let [ready_reader, ready_writer] = ready;
        drop(ready_writer);
        await_ready(ready_reader.as_raw_fd())
    }
}

fn await_ready(fd: libc::c_int) -> std::io::Result<()> {
    let mut byte = [0];
    transfer(fd, &mut byte, false)?;
    if byte[0] != b'.' {
        return Err(std::io::Error::from_raw_os_error(libc::EIO));
    }
    Ok(())
}

/// Acquire lifetime custody before claiming an attachment. Missing FIFOs and
/// ENXIO refuse handoff rather than claiming an already stopping provider.
pub(crate) fn hold_agent_process_lifeline(path: &Path) -> std::io::Result<AgentProcessCustody> {
    let writer = File::options()
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)?;
    AgentProcessCustody::new(above_stdio(writer)?)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{hold_agent_process_lifeline, spawn_agent_process, spawn_native_agent_process};
    use crate::os_process::{kill_process_group, terminate_process_group};

    #[test]
    fn native_spawn_records_before_exec_and_preserves_terminal_and_group() {
        use std::fs::File;
        use std::os::fd::FromRawFd;

        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("executed");
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: openpty writes two valid output descriptors; optional termios,
        // window size and name pointers are null. Files take ownership once.
        let (master, slave) = unsafe {
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ),
                0
            );
            (File::from_raw_fd(master), File::from_raw_fd(slave))
        };
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .args([
                "-c",
                "[ -t 0 ] && [ -t 1 ] && [ -t 2 ] || exit 1; printf executed > \"$1\"; exit 42",
                "fixture",
            ])
            .arg(&marker)
            .stdin(slave.try_clone().unwrap())
            .stdout(slave.try_clone().unwrap())
            .stderr(slave);
        let mut recorded = None;
        let mut child = spawn_native_agent_process(command, |pid| {
            assert!(!marker.exists(), "provider code ran before recording");
            // SAFETY: queries only; no process is signalled or modified.
            unsafe {
                assert_eq!(libc::getpgid(pid as i32), libc::getpgrp());
            }
            recorded = Some(pid);
            Ok(())
        })
        .unwrap();
        assert_eq!(recorded, Some(child.id()));
        assert_eq!(child.wait().unwrap().code(), Some(42));
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "executed");
        drop(master);
    }

    #[test]
    fn native_record_failure_prevents_effects_and_preserves_spawn_errors() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("executed");
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .args(["-c", "printf executed > \"$1\"", "fixture"])
            .arg(&marker);
        let error =
            spawn_native_agent_process(command, |_| Err(std::io::Error::other("record refused")))
                .unwrap_err();
        assert_eq!(error.to_string(), "record refused");
        assert!(!marker.exists());
        let error =
            spawn_native_agent_process(Command::new(root.path().join("absent")), |_| Ok(()))
                .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    const ATTACHED_MODE: &str = "LF_TEST_LIFELINE_LF";
    const ATTACHED_OUT: &str = "LF_TEST_LIFELINE_OUT";
    const ATTACHED_FIFO: &str = "LF_TEST_LIFELINE_FIFO";

    fn group_alive(group: u32) -> bool {
        // SAFETY: signal 0 probes a group this test created; nothing is delivered.
        unsafe { libc::kill(-(group as i32), 0) == 0 }
    }

    fn wait_until(mut done: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        done()
    }

    /// Not a test of its own: re-executed by the tests below as a throwaway
    /// lf process that prepares and starts an agent, then ends the way
    /// its mode says. Without the mode variable it does nothing.
    // The lf process ends without waiting on purpose: that is the case under test.
    #[tokio::test]
    async fn lifeline_attached_lf_process() {
        let Ok(mode) = std::env::var(ATTACHED_MODE) else {
            return;
        };
        if mode == "holder" {
            let fifo = std::env::var_os(ATTACHED_FIFO).unwrap();
            let pid = std::env::var("LF_TEST_LIFELINE_PID")
                .unwrap()
                .parse()
                .unwrap();
            hold_agent_process_lifeline(Path::new(&fifo))
                .unwrap()
                .retain(
                    pid,
                    crate::journal::process_started_at(pid).unwrap().unwrap(),
                );
            publish_pid(&std::env::var(ATTACHED_OUT).unwrap(), pid);
            std::thread::sleep(Duration::from_secs(60));
            return;
        }
        if mode == "closed_stdio" {
            // SAFETY: this is only the re-executed throwaway lf process.
            unsafe {
                libc::close(0);
                libc::close(1);
                libc::close(2);
            }
        }
        let mut command = tokio::process::Command::new("sleep");
        command
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let fifo = std::env::var_os(ATTACHED_FIFO).unwrap();
        let out = std::env::var(ATTACHED_OUT).unwrap();
        let agent = spawn_agent_process(command, Path::new(&fifo), |pid| {
            if mode == "before_exec" {
                // The parent recorder stalls after watchdog readiness, before
                // exec. Killing this lf must end the waiting child too.
                publish_pid(&out, pid);
                std::thread::sleep(Duration::from_secs(60));
            }
            Ok(())
        })
        .unwrap();
        publish_pid(&out, agent.id().unwrap());
        match mode.as_str() {
            "exit" => std::process::exit(0),
            "panic" => panic!("lf process panicked"),
            // Killed by the parent test.
            _ => std::thread::sleep(Duration::from_secs(60)),
        }
    }

    fn publish_pid(out: &str, pid: u32) {
        std::fs::write(format!("{out}.tmp"), pid.to_ne_bytes()).unwrap();
        std::fs::rename(format!("{out}.tmp"), out).unwrap();
    }

    /// Start the throwaway lf process and return it with its agent's group.
    fn spawn_attached_lf(mode: &str, dir: &Path, fifo: Option<&Path>) -> (Child, u32) {
        let out = dir.join(format!("agent-{mode}"));
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "os_process::lifeline::tests::lifeline_attached_lf_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ATTACHED_MODE, mode)
            .env(ATTACHED_OUT, &out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.env(
            ATTACHED_FIFO,
            fifo.map(Path::to_path_buf)
                .unwrap_or_else(|| dir.join(format!("{mode}.lifeline"))),
        );
        let attached_lf = command.spawn().unwrap();
        assert!(
            wait_until(|| out.exists()),
            "lf process never started its agent"
        );
        let agent = u32::from_ne_bytes(std::fs::read(&out).unwrap().try_into().unwrap());
        (attached_lf, agent)
    }

    fn agent_dies_when_attached_lf_ends(mode: &str, signal: Option<libc::c_int>) {
        let dir = tempfile::tempdir().unwrap();
        let (mut attached_lf, agent) = spawn_attached_lf(mode, dir.path(), None);
        if let Some(signal) = signal {
            assert!(group_alive(agent));
            // SAFETY: signals only the throwaway lf process this test spawned.
            unsafe { libc::kill(attached_lf.id() as i32, signal) };
        }
        attached_lf.wait().unwrap();
        let died = wait_until(|| !group_alive(agent));
        kill_process_group(agent);
        assert!(died, "agent outlived an lf process that ended by {mode}");
    }

    #[test]
    fn agent_dies_when_its_attached_lf_exits() {
        agent_dies_when_attached_lf_ends("exit", None);
    }

    #[test]
    fn agent_dies_when_its_attached_lf_panics() {
        agent_dies_when_attached_lf_ends("panic", None);
    }

    #[test]
    fn agent_dies_when_its_attached_lf_is_terminated() {
        agent_dies_when_attached_lf_ends("sigterm", Some(libc::SIGTERM));
    }

    #[test]
    fn agent_dies_when_its_attached_lf_is_killed() {
        agent_dies_when_attached_lf_ends("sigkill", Some(libc::SIGKILL));
    }

    #[test]
    fn agent_survives_its_first_attached_lf_while_another_holds_the_lifeline() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("agent.lifeline");
        let (mut attached_lf, agent) = spawn_attached_lf("handoff", dir.path(), Some(&fifo));
        // Acquire before the claim: launcher death in that interval is safe.
        let custody = hold_agent_process_lifeline(&fifo).unwrap();
        let started = crate::journal::process_started_at(agent).unwrap().unwrap();
        // SAFETY: signals only the throwaway lf process this test spawned.
        unsafe { libc::kill(attached_lf.id() as i32, libc::SIGKILL) };
        attached_lf.wait().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        let alive = group_alive(agent);
        custody.retain(agent, started);
        assert!(terminate_process_group(agent));
        assert!(alive, "agent died although a second lf process held it");
        // Missing custody must refuse takeover.
        assert!(hold_agent_process_lifeline(&dir.path().join("absent")).is_err());
    }

    #[test]
    fn either_holder_can_die_first_and_failed_custody_does_not_end_the_provider() {
        for launcher_first in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let fifo = dir.path().join("agent.lifeline");
            let (mut launcher, agent) = spawn_attached_lf("launcher", dir.path(), Some(&fifo));
            // A failed attachment drops only its prospective custody.
            drop(hold_agent_process_lifeline(&fifo).unwrap());
            let ready = dir.path().join("holder-ready");
            let mut holder = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "os_process::lifeline::tests::lifeline_attached_lf_process",
                    "--test-threads=1",
                ])
                .env(ATTACHED_MODE, "holder")
                .env(ATTACHED_FIFO, &fifo)
                .env(ATTACHED_OUT, &ready)
                .env("LF_TEST_LIFELINE_PID", agent.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            assert!(wait_until(|| ready.exists()));
            let (first, last) = if launcher_first {
                (&mut launcher, &mut holder)
            } else {
                (&mut holder, &mut launcher)
            };
            first.kill().unwrap();
            first.wait().unwrap();
            std::thread::sleep(Duration::from_secs(3));
            let survived = group_alive(agent);
            last.kill().unwrap();
            last.wait().unwrap();
            let ended = wait_until(|| !group_alive(agent));
            kill_process_group(agent);
            assert!(survived, "first holder's death ended the provider");
            assert!(ended, "last holder's death retained the provider");
        }
    }

    #[tokio::test]
    async fn leader_exit_does_not_release_surviving_helpers() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("lifeline");
        let mut command = tokio::process::Command::new("/bin/sh");
        command.env_clear().args(["-c", "/bin/sleep 60 & exit 0"]);
        let mut child = spawn_agent_process(command, &fifo, |_| Ok(())).unwrap();
        let pid = child.id().unwrap();
        assert!(child.wait().await.unwrap().success());
        std::thread::sleep(Duration::from_secs(3));
        let alive = group_alive(pid);
        assert!(terminate_process_group(pid));
        assert!(
            alive,
            "leader exit released custody of its surviving helper"
        );
        assert!(wait_until(|| hold_agent_process_lifeline(&fifo).is_err()));
    }

    #[tokio::test]
    async fn natural_provider_exit_releases_custody_and_watchdog() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("lifeline");
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        let mut child = spawn_agent_process(command, &fifo, |_| Ok(())).unwrap();
        let pid = child.id().unwrap();
        assert!(child.wait().await.unwrap().success());
        assert!(wait_until(|| !group_alive(pid)));
        // Custody observes death asynchronously; wait for watchdog EOF too.
        assert!(wait_until(|| hold_agent_process_lifeline(&fifo).is_err()));
    }

    #[test]
    fn agent_dies_when_lf_is_killed_before_provider_exec() {
        agent_dies_when_attached_lf_ends("before_exec", Some(libc::SIGKILL));
    }

    #[test]
    fn lifeline_survives_stdio_replacement_with_closed_standard_descriptors() {
        agent_dies_when_attached_lf_ends("closed_stdio", Some(libc::SIGKILL));
    }

    #[tokio::test]
    async fn failed_exec_releases_its_watchdog_and_keeps_the_spawn_error() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("lifeline");
        let command = tokio::process::Command::new(dir.path().join("absent-provider"));
        let error = spawn_agent_process(command, &fifo, |_| Ok(())).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(
            wait_until(|| {
                std::fs::File::options()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&fifo)
                    .is_err_and(|error| error.raw_os_error() == Some(libc::ENXIO))
            }),
            "failed spawn kept its watchdog alive"
        );
    }

    #[tokio::test]
    async fn provider_exec_observes_its_recorded_identity() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("process");
        let executed = dir.path().join("executed");
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .args([
                "-c",
                "test \"$(cat \"$1\")\" = \"$$\" && printf recorded > \"$2\"",
                "fixture",
            ])
            .arg(&record)
            .arg(&executed);
        let mut child = spawn_agent_process(command, &dir.path().join("lifeline"), |pid| {
            assert!(!executed.exists());
            // SAFETY: read metadata of the throwaway child supplied by spawn.
            assert_eq!(unsafe { libc::getpgid(pid as i32) }, pid as i32);
            std::fs::write(&record, pid.to_string())
        })
        .unwrap();
        assert!(child.wait().await.unwrap().success());
        assert_eq!(std::fs::read(&executed).unwrap(), b"recorded");
    }

    #[tokio::test]
    async fn rejected_process_record_prevents_provider_effects() {
        let dir = tempfile::tempdir().unwrap();
        let executed = dir.path().join("executed");
        let fifo = dir.path().join("lifeline");
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .args(["-c", "printf executed > \"$1\"", "fixture"])
            .arg(&executed);
        let error = spawn_agent_process(command, &fifo, |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "attachment changed",
            ))
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "attachment changed");
        assert!(!executed.exists());
        assert!(wait_until(|| std::fs::File::options()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)
            .is_err_and(|error| error.raw_os_error() == Some(libc::ENXIO))));
    }

    #[tokio::test]
    async fn pre_exec_failure_keeps_its_error_without_waiting_for_a_record() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("record");
        let mut command = tokio::process::Command::new("/bin/sh");
        command.current_dir(dir.path().join("absent"));
        let error = spawn_agent_process(command, &dir.path().join("lifeline"), |pid| {
            std::fs::write(&record, pid.to_string())
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(!record.exists());
    }

    #[tokio::test]
    async fn launch_preserves_native_stdio_arguments_and_exit_status() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .args([
                "-c",
                "read -r line; printf '%s|%s' \"$line\" \"$1\"; printf stderr >&2; exit 42",
                "fixture",
                "arg with 'quotes'",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child =
            spawn_agent_process(command, &dir.path().join("lifeline"), |_| Ok(())).unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"input bytes\n")
            .await
            .unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.code(), Some(42));
        assert_eq!(output.stdout, b"input bytes|arg with 'quotes'");
        assert_eq!(output.stderr, b"stderr");
    }
}
