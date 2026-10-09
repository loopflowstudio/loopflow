//! Start the watchdog before execing an agent, never after spawning it.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

// A connected lf invocation holds its writer until OS exit, including SIGKILL.
static HELD_LIFELINES: Mutex<Vec<File>> = Mutex::new(Vec::new());

// stdout acknowledges watchdog startup, not provider output. Ignoring TERM
// lets the watchdog and its sleep survive the group signal to deliver KILL.
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
    path: Option<&Path>,
    record: impl FnOnce(u32) -> std::io::Result<()> + Send,
) -> std::io::Result<tokio::process::Child> {
    let writer = prepare_lifeline(command.as_std_mut(), path)?;
    // Command::spawn waits for exec, so recording on its calling thread would
    // deadlock the child. The scoped thread writes in the parent, never after fork.
    let (parent, child) = std::os::unix::net::UnixStream::pair()?;
    let mut parent = above_stdio(File::from(OwnedFd::from(parent)))?;
    let child = above_stdio(File::from(OwnedFd::from(child)))?;
    let parent_fd = parent.as_raw_fd();
    // SAFETY: the child only closes its inherited peer, sends its own PID and
    // waits for acknowledgement using async-signal-safe syscalls. The parent
    // owns the recorder and performs all allocation, locking and database I/O.
    unsafe {
        command.pre_exec(move || {
            libc::close(parent_fd);
            let pid = libc::getpid() as u32;
            transfer(child.as_raw_fd(), &mut pid.to_ne_bytes(), true)?;
            let mut acknowledged = 0u8;
            transfer(
                child.as_raw_fd(),
                std::slice::from_mut(&mut acknowledged),
                false,
            )?;
            if acknowledged != b'.' {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            Ok(())
        });
    }
    let child = std::thread::scope(|scope| {
        let recorder = std::thread::Builder::new()
            .name("agent-process-record".into())
            .spawn_scoped(scope, move || {
                let mut pid = [0u8; 4];
                match parent.read_exact(&mut pid) {
                    Ok(()) => {}
                    // Spawn failed before reaching our hook; preserve that error.
                    Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                        return Ok(())
                    }
                    Err(error) => return Err(error),
                }
                record(u32::from_ne_bytes(pid))?;
                parent.write_all(b".")
            })?;
        let spawned = command.spawn();
        // A failed spawn might never enter pre_exec. Close the parent's copy
        // of the child endpoint before waiting for the recorder's EOF.
        drop(command);
        recorder
            .join()
            .map_err(|_| std::io::Error::other("AgentProcess recorder panicked"))??;
        spawned
    })?;
    retain_lifeline(writer);
    Ok(child)
}

/// Complete a tiny pre-exec handshake without allocating or using Rust locks.
fn transfer(fd: libc::c_int, bytes: &mut [u8], write: bool) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: callers supply a live buffer and an owned
        // socket. The offset remains within the buffer; both calls are
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

fn prepare_lifeline(command: &mut Command, path: Option<&Path>) -> std::io::Result<File> {
    let (reader, writer) = match path {
        Some(path) => {
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
            (reader, writer)
        }
        None => {
            let (reader, writer) = std::io::pipe()?;
            (
                above_stdio(File::from(OwnedFd::from(reader)))?,
                above_stdio(File::from(OwnedFd::from(writer)))?,
            )
        }
    };
    let null = above_stdio(File::options().write(true).open("/dev/null")?)?;
    command.process_group(0);
    let writer_fd = writer.as_raw_fd();
    // SAFETY: after fork this closure performs only stack operations and
    // async-signal-safe syscalls. Captured files stay owned by Command in
    // the parent; neither branch unwinds, allocates, or locks after fork.
    unsafe {
        command.pre_exec(move || {
            // CLOEXEC alone is too late: a launcher stalled before exec
            // would otherwise keep its own lifeline alive after lf dies.
            libc::close(writer_fd);
            start_watchdog(reader.as_raw_fd(), null.as_raw_fd())
        });
    }
    Ok(writer)
}

fn retain_lifeline(writer: File) {
    HELD_LIFELINES
        .lock()
        .expect("lifeline list poisoned")
        .push(writer);
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
        let pid = libc::fork();
        if pid == -1 {
            return Err(std::io::Error::last_os_error());
        }
        if pid == 0 {
            libc::close(ready[0].as_raw_fd());
            if libc::dup2(reader, libc::STDIN_FILENO) == -1
                || libc::dup2(ready[1].as_raw_fd(), libc::STDOUT_FILENO) == -1
                || libc::dup2(null, libc::STDERR_FILENO) == -1
            {
                libc::_exit(127);
            }
            // Format the inherited group ID without an allocator. The final
            // byte stays NUL; a Unix pid fits in ten decimal digits.
            let mut group = libc::getpgrp() as u32;
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
        let mut byte = 0u8;
        loop {
            match libc::read(ready_reader.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) {
                1 if byte == b'.' => return Ok(()),
                -1 => {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
                _ => return Err(std::io::Error::from_raw_os_error(libc::EIO)),
            }
        }
    }
}

/// Hold a running AgentProcess across attachment transfer. Absent paths belong
/// to launches before lifelines; ENXIO means shutdown already began.
pub(crate) fn hold_agent_process_lifeline(path: &Path) -> std::io::Result<bool> {
    match File::options()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
    {
        Ok(writer) => {
            retain_lifeline(writer);
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn agent_process_lifeline_path(endpoint: &Path) -> PathBuf {
    // Preserve the FIFO path for attachment to already-running processes.
    endpoint.with_file_name("driver.lifeline")
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{
        hold_agent_process_lifeline, prepare_lifeline, retain_lifeline, spawn_agent_process,
    };
    use crate::engine::process::{kill_process_group, terminate_process_group};

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
    #[allow(clippy::zombie_processes)]
    #[test]
    fn lifeline_attached_lf_process() {
        let Ok(mode) = std::env::var(ATTACHED_MODE) else {
            return;
        };
        if mode == "closed_stdio" {
            // SAFETY: this is only the re-executed throwaway lf process.
            unsafe {
                libc::close(0);
                libc::close(1);
                libc::close(2);
            }
        }
        let mut command = Command::new("sleep");
        command
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let fifo = std::env::var_os(ATTACHED_FIFO);
        let out = std::env::var(ATTACHED_OUT).unwrap();
        let lifeline = prepare_lifeline(&mut command, fifo.as_deref().map(Path::new)).unwrap();
        if mode == "before_exec" {
            let pending = format!("{out}.tmp");
            let file = std::fs::File::create(&pending).unwrap();
            let pending = std::ffi::CString::new(pending).unwrap();
            let published = std::ffi::CString::new(out.clone()).unwrap();
            // SAFETY: this test-only pre-exec hook uses only syscalls and
            // stack bytes. It stalls after watchdog readiness, before exec.
            unsafe {
                command.pre_exec(move || {
                    let pid = (libc::getpid() as u32).to_ne_bytes();
                    if libc::write(file.as_raw_fd(), pid.as_ptr().cast(), pid.len()) != 4
                        || libc::rename(pending.as_ptr(), published.as_ptr()) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    libc::sleep(60);
                    Ok(())
                });
            }
        }
        let agent = command.spawn().unwrap();
        retain_lifeline(lifeline);
        std::fs::write(format!("{out}.tmp"), agent.id().to_ne_bytes()).unwrap();
        std::fs::rename(format!("{out}.tmp"), &out).unwrap();
        match mode.as_str() {
            "exit" => std::process::exit(0),
            "panic" => panic!("lf process panicked"),
            // Killed by the parent test.
            _ => std::thread::sleep(Duration::from_secs(60)),
        }
    }

    /// Start the throwaway lf process and return it with its agent's group.
    fn spawn_attached_lf(mode: &str, dir: &Path, fifo: Option<&Path>) -> (Child, u32) {
        let out = dir.join(format!("agent-{mode}"));
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "engine::process::lifeline::tests::lifeline_attached_lf_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ATTACHED_MODE, mode)
            .env(ATTACHED_OUT, &out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(fifo) = fifo {
            command.env(ATTACHED_FIFO, fifo);
        }
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
        // This test process takes the agent over, then the first lf process dies.
        assert!(hold_agent_process_lifeline(&fifo).unwrap());
        // SAFETY: signals only the throwaway lf process this test spawned.
        unsafe { libc::kill(attached_lf.id() as i32, libc::SIGKILL) };
        attached_lf.wait().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        let alive = group_alive(agent);
        assert!(terminate_process_group(agent));
        assert!(alive, "agent died although a second lf process held it");
        // An agent with no lifeline predates them; there is nothing to hold.
        assert!(!hold_agent_process_lifeline(&dir.path().join("absent")).unwrap());
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
        let error = spawn_agent_process(command, Some(&fifo), |_| Ok(())).unwrap_err();
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
        let mut child = spawn_agent_process(command, None, |pid| {
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
        let error = spawn_agent_process(command, Some(&fifo), |_| {
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
        let error = spawn_agent_process(command, None, |pid| {
            std::fs::write(&record, pid.to_string())
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(!record.exists());
    }

    #[tokio::test]
    async fn launch_preserves_native_stdio_arguments_and_exit_status() {
        use tokio::io::AsyncWriteExt;
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
        let mut child = spawn_agent_process(command, None, |_| Ok(())).unwrap();
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
