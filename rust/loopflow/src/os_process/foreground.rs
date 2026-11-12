//! A bare conversation's provider leads the terminal's foreground process
//! group, as it would started from the shell: Ctrl+C and Ctrl+Z reach it, and a
//! host that names a terminal by its foreground leader (Rex) shows the
//! provider, not the `lf` waiting behind it.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// The terminal handed to a child; dropping it takes the terminal back.
#[derive(Debug)]
pub(crate) struct Foreground {
    /// lf's terminal, close-on-exec: the child hands itself the terminal
    /// before exec, whatever its own stdin is, and the provider never holds it.
    terminal: OwnedFd,
}

/// Make `command`'s child its own process group and the foreground of the
/// terminal on lf's stdin. None when an owner holds the launch, stdin is no
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
        give(self.terminal.as_raw_fd(), current_group());
    }
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
