//! A bare conversation's provider leads the terminal's foreground process
//! group, as it would started from the shell: Ctrl+C and Ctrl+Z reach it, and a
//! host that names a terminal by its foreground leader (Rex) shows the
//! provider, not the `lf` waiting behind it.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// The provider leading the terminal while lf waits for it; 0 when none.
static LEADER: AtomicU32 = AtomicU32::new(0);
/// lf itself was signaled while a provider led its terminal.
static SIGNALED: AtomicBool = AtomicBool::new(false);

/// The terminal handed to a child; dropping it takes the terminal back.
#[derive(Debug)]
pub(crate) struct Foreground {
    /// lf's terminal, close-on-exec: the child hands itself the terminal
    /// before exec, whatever its own stdin is, and the provider never holds it.
    terminal: OwnedFd,
}

/// Make `command`'s child its own process group and the foreground of the
/// terminal on lf's stdin. None inside a held terminal, when stdin is no
/// terminal, or lf is not in the terminal's foreground itself.
pub(crate) fn hand_to(command: &mut Command) -> Option<Foreground> {
    if crate::transport::current().is_some() {
        return None;
    }
    // SAFETY: tcgetpgrp and getpgrp only read process state.
    if unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) != libc::getpgrp() } {
        return None;
    }
    // SAFETY: F_DUPFD_CLOEXEC returns a new descriptor this guard owns.
    let terminal = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    if terminal < 0 {
        return None;
    }
    // SAFETY: the duplicate is open and owned by nothing else.
    let terminal = unsafe { OwnedFd::from_raw_fd(terminal) };
    let fd = terminal.as_raw_fd();
    // SAFETY: only async-signal-safe calls run between fork and exec. A
    // background group calling tcsetpgrp would be stopped by SIGTTOU, so it is
    // ignored for the call and restored before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let previous = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
            let handed = libc::tcsetpgrp(fd, libc::getpid());
            libc::signal(libc::SIGTTOU, previous);
            if handed < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Some(Foreground { terminal })
}

impl Foreground {
    /// Name `child` as the provider a signal to lf ends, until lf takes the
    /// terminal back.
    pub(crate) fn lead(&self, child: u32) {
        LEADER.store(child, Ordering::Release);
    }

    /// Whether the provider ended because its terminal or lf was signaled:
    /// Ctrl+C, a closed terminal or a `kill`, as it would end started alone.
    pub(crate) fn interrupted(&self, status: &ExitStatus) -> bool {
        SIGNALED.load(Ordering::Acquire)
            || matches!(
                status.signal(),
                Some(libc::SIGINT | libc::SIGHUP | libc::SIGTERM)
            )
    }

    /// A provider suspended with Ctrl+Z suspends lf with it, so the shell gets
    /// its terminal back as for the provider alone; `fg` continues both.
    /// Polled while lf waits for `child`.
    pub(crate) fn follow_stop(&self, child: u32) {
        let Ok(group) = libc::pid_t::try_from(child) else {
            return;
        };
        if !stopped(child) {
            return;
        }
        give(self.terminal.as_raw_fd(), current_group());
        // SAFETY: stops only lf itself; the shell's `fg` continues it.
        unsafe { libc::raise(libc::SIGSTOP) };
        give(self.terminal.as_raw_fd(), group);
        // SAFETY: continues only the provider's own process group.
        unsafe { libc::kill(-group, libc::SIGCONT) };
    }
}

impl Drop for Foreground {
    fn drop(&mut self) {
        LEADER.store(0, Ordering::Release);
        give(self.terminal.as_raw_fd(), current_group());
    }
}

/// A signal to lf while a provider leads its terminal (the shell's SIGHUP when
/// the terminal closes, a `kill`) ends the provider, and lf's own return
/// records that exit. Returns at once when no provider leads, otherwise only
/// after lf has had time to finish, so the caller's interrupt exit runs only for
/// a launch that hung.
pub(crate) fn end_leader() {
    let group = match libc::pid_t::try_from(LEADER.load(Ordering::Acquire)) {
        Ok(group) if group > 0 => group,
        _ => return,
    };
    SIGNALED.store(true, Ordering::Release);
    // SAFETY: signals only the provider's own process group.
    unsafe { libc::kill(-group, libc::SIGTERM) };
    if !leader_cleared(Duration::from_secs(5)) {
        // SAFETY: as above; a provider ignoring SIGTERM is killed.
        unsafe { libc::kill(-group, libc::SIGKILL) };
        leader_cleared(Duration::from_secs(5));
    }
    // lf records the exit and returns on its own thread; the process ends there.
    std::thread::sleep(Duration::from_secs(30));
}

fn leader_cleared(within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while LEADER.load(Ordering::Acquire) != 0 {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

fn current_group() -> libc::pid_t {
    // SAFETY: getpgrp only reads process state.
    unsafe { libc::getpgrp() }
}

/// From a background group, as lf is while the provider leads. A terminal that
/// closed returns an error that changes nothing.
fn give(terminal: RawFd, group: libc::pid_t) {
    // SAFETY: SIGTTOU is ignored only around the call so it cannot stop lf.
    unsafe {
        let previous = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::tcsetpgrp(terminal, group);
        libc::signal(libc::SIGTTOU, previous);
    }
}

/// WSTOPPED without WEXITED consumes only a stop: the exit stays for
/// `Child::try_wait`.
fn stopped(child: u32) -> bool {
    // SAFETY: siginfo_t is plain data; waitid fills it or leaves it zeroed.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a valid output buffer for this call.
    let found = unsafe {
        libc::waitid(
            libc::P_PID,
            child,
            &mut info,
            libc::WSTOPPED | libc::WNOHANG,
        )
    } == 0;
    #[cfg(target_os = "macos")]
    let pid = info.si_pid;
    // SAFETY: waitid filled the child fields when it found one.
    #[cfg(not(target_os = "macos"))]
    let pid = unsafe { info.si_pid() };
    found && pid != 0
}
