//! A retained Session terminal. tmux owns the screen and application PTY; only
//! the controller registers a terminal client. Passive views read snapshots and
//! have no application input, terminal-response, or resize path.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use unicode_width::UnicodeWidthChar;

#[derive(Debug)]
pub(crate) struct SessionTerminal {
    directory: PathBuf,
}

impl SessionTerminal {
    pub(crate) fn new(name: &str) -> Result<Self> {
        let home = crate::store::lf_home_dir();
        let home = home.canonicalize().unwrap_or(home);
        let digest = Sha256::digest(format!("{}\0{name}", home.display()).as_bytes());
        // Unix socket paths must fit sun_path even for long Home/worktree paths.
        // SAFETY: geteuid has no preconditions.
        let root = PathBuf::from(format!("/tmp/lf-term-{}", unsafe { libc::geteuid() }));
        fs::create_dir_all(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let directory = root.join(hex::encode(&digest[..16]));
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        command
            .args(["-S"])
            .arg(self.directory.join("socket"))
            .env_remove("TMUX")
            .env_remove("TMUX_PANE");
        command
    }

    fn lock(&self, name: &str) -> Result<File> {
        Ok(OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.directory.join(name))?)
    }

    pub(crate) fn is_running(&self) -> Result<bool> {
        if !self.directory.join("socket").exists() {
            return Ok(false);
        }
        let output = self
            .command()
            .args(["has-session", "-t", "=session"])
            .output()?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => bail!(
                "inspect retained Session terminal: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    pub(crate) fn start(&self, cwd: &Path, shell: &str) -> Result<()> {
        let launch = self.lock("launch.lock")?;
        FileExt::lock_exclusive(&launch)?;
        if self.is_running()? {
            return Ok(());
        }
        let config = self.directory.join("tmux.conf");
        fs::write(
            &config,
            concat!(
                "set -g status off\n",
                "set -g default-terminal xterm-256color\n",
                "set -g terminal-features 'xterm*:RGB:clipboard'\n",
                "set -g set-clipboard on\n",
                "set -s get-clipboard request\n",
                "set -g history-limit 10000\n",
                "set -g window-size latest\n",
                "set -g update-environment ''\n",
                "set -g exit-empty on\n",
            ),
        )?;
        let output = self
            .command()
            .arg("-f")
            .arg(config)
            .args(["new-session", "-d", "-s", "session", "-c"])
            .arg(cwd)
            .args(["/bin/sh", "-lc", shell])
            .current_dir(cwd)
            .output()?;
        if !output.status.success() {
            bail!(
                "start retained Session terminal: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    pub(crate) fn attach(&self, take_control: bool) -> Result<bool> {
        if !self.directory.join("socket").exists() {
            return Ok(false);
        }
        // SAFETY: isatty inspects only the process's stdin descriptor.
        if unsafe { libc::isatty(0) } == 0 {
            bail!("attaching a retained Session requires a terminal");
        }
        let admission = self.lock("attach.lock")?;
        FileExt::lock_exclusive(&admission)?;
        // This one server read establishes both existence and the controller.
        let clients = self
            .command()
            .args(["list-clients", "-F", "#{client_pid}"])
            .output()?;
        if clients.status.code() == Some(1) {
            return Ok(false);
        }
        if !clients.status.success() {
            bail!(
                "inspect retained terminal clients: {}",
                String::from_utf8_lossy(&clients.stderr)
            );
        }
        if !clients.stdout.is_empty() {
            if !take_control {
                drop(admission);
                self.view()?;
                return Ok(true);
            }
            let output = self
                .command()
                .args(["detach-client", "-s", "session"])
                .output()?;
            if !output.status.success() {
                bail!(
                    "detach previous controller: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        crate::journal::connect::phase("connection_prepared");
        let mut child = self
            .command()
            .args(["attach-session", "-t", "=session"])
            .spawn()?;
        let client_pid = child.id().to_string();
        let socket = self.directory.join("socket");
        crate::engine::agent::register_interrupt_cleanup(move || {
            // Query the private server for this exact attachment. A delayed
            // cleanup cannot detach a successor with a different client PID.
            if let Ok(clients) = Command::new("tmux")
                .arg("-S")
                .arg(&socket)
                .args(["list-clients", "-F", "#{client_pid} #{client_name}"])
                .output()
            {
                for line in String::from_utf8_lossy(&clients.stdout).lines() {
                    if let Some((pid, name)) = line.split_once(' ') {
                        if pid == client_pid {
                            let _ = Command::new("tmux")
                                .arg("-S")
                                .arg(&socket)
                                .args(["detach-client", "-t", name])
                                .output();
                        }
                    }
                }
            }
        });
        // Keep admission until tmux has registered the new controller. Otherwise
        // a concurrent opener could register a second controller.
        let deadline = Instant::now() + Duration::from_secs(5);
        let attached = loop {
            let output = self
                .command()
                .args(["list-clients", "-F", "#{client_pid}"])
                .output()?;
            if String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|pid| pid == child.id().to_string())
            {
                break true;
            }
            if child.try_wait()?.is_some() || Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(admission);
        if !attached {
            let _ = child.kill();
            let _ = child.wait();
            bail!("terminal client failed before attachment; retained Session was not replaced");
        }
        crate::journal::connect::phase("attached");
        let status = child.wait()?;
        if !status.success() {
            bail!("terminal attachment exited with {status}; Session remains independent");
        }
        Ok(true)
    }

    fn view(&self) -> Result<()> {
        let _terminal = PassiveTerminal::enter()?;
        crate::journal::connect::phase("connection_prepared");
        crate::journal::connect::phase("attached");
        let mut previous = Vec::new();
        let mut input = [0; 4096];
        let mut left = 0usize;
        let mut top = 0usize;
        loop {
            // A single snapshot at a time: a blocked stdout never queues later
            // snapshots or applies backpressure to the application's PTY.
            let mut child = self
                .command()
                .args(["capture-pane", "-p", "-e", "-t", "session:0.0"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let mut screen = Vec::new();
            child
                .stdout
                .take()
                .expect("snapshot stdout is piped")
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut screen)?;
            if screen.len() > 4 * 1024 * 1024 {
                let _ = child.kill();
                let _ = child.wait();
                bail!("retained terminal snapshot exceeds 4 MiB");
            }
            if !child.wait()?.success() {
                break;
            }
            let (rows, columns) = view_size();
            let rendered = render_snapshot(&screen, rows, columns, top, left);
            if rendered != previous {
                write_frame(&rendered)?;
                previous = rendered;
            }
            let mut fd = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: fd points to one initialized pollfd for the call's duration.
            if unsafe { libc::poll(&mut fd, 1, 100) } > 0 {
                let count = std::io::stdin().read(&mut input)?;
                if count == 0
                    || input[..count]
                        .iter()
                        .any(|byte| matches!(byte, 3 | 4 | b'q'))
                {
                    break;
                }
                match &input[..count] {
                    b"h" | b"\x1b[D" => left = left.saturating_sub(8),
                    b"l" | b"\x1b[C" => left = left.saturating_add(8),
                    b"k" | b"\x1b[A" => top = top.saturating_sub(1),
                    b"j" | b"\x1b[B" => top = top.saturating_add(1),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn view_size() -> (usize, usize) {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: ioctl writes to one winsize; zero initialization supplies fallback.
    unsafe {
        libc::ioctl(0, libc::TIOCGWINSZ, size.as_mut_ptr());
        let size = size.assume_init();
        (
            usize::from(size.ws_row.max(1)),
            usize::from(size.ws_col.max(1)),
        )
    }
}

fn render_snapshot(screen: &[u8], rows: usize, columns: usize, top: usize, left: usize) -> Vec<u8> {
    let mut result = String::from("\x1b[H\x1b[2J\x1b[?7l");
    for (row, line) in String::from_utf8_lossy(screen)
        .lines()
        .skip(top)
        .take(rows)
        .enumerate()
    {
        result.push_str(&format!("\x1b[{};1H\x1b[0m", row + 1));
        let mut column = 0;
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            // capture-pane generates SGR styling, never historical OSC/DCS
            // queries. Keep only those generated styles in passive rendering.
            if ch == '\x1b' {
                if chars.next() == Some('[') {
                    let mut style = String::from("\x1b[");
                    for ch in chars.by_ref() {
                        style.push(ch);
                        if ch == 'm' {
                            result.push_str(&style);
                            break;
                        }
                        if !ch.is_ascii_digit() && !matches!(ch, ';' | ':') {
                            break;
                        }
                    }
                }
                continue;
            }
            if ch.is_control() {
                continue;
            }
            let width = ch.width().unwrap_or(0);
            if column >= left && column + width <= left.saturating_add(columns) {
                result.push(ch);
            }
            column += width;
        }
    }
    result.push_str("\x1b[0m\x1b[?7h");
    result.into_bytes()
}

#[derive(Debug)]
struct PassiveTerminal {
    saved: libc::termios,
    output_flags: libc::c_int,
}

impl PassiveTerminal {
    fn enter() -> Result<Self> {
        let mut saved = std::mem::MaybeUninit::uninit();
        // SAFETY: tcgetattr initializes saved on success; stdin must be a tty.
        if unsafe { libc::tcgetattr(0, saved.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error()).context("passive view needs a terminal");
        }
        // SAFETY: tcgetattr succeeded.
        let saved = unsafe { saved.assume_init() };
        let mut raw = saved;
        // SAFETY: raw is initialized; these operations affect only this viewer.
        unsafe {
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(0, libc::TCSANOW, &raw) != 0 {
                return Err(std::io::Error::last_os_error()).context("enter passive terminal");
            }
        }
        // A viewer that stops reading must never trap cleanup in write_all.
        // SAFETY: fcntl inspects and updates this process's stdout descriptor.
        let output_flags = unsafe { libc::fcntl(1, libc::F_GETFL) };
        // SAFETY: stdout is valid; preserve its original flags for restoration.
        let nonblocking = output_flags != -1
            && unsafe { libc::fcntl(1, libc::F_SETFL, output_flags | libc::O_NONBLOCK) } != -1;
        if !nonblocking {
            let error = std::io::Error::last_os_error();
            // SAFETY: saved came from a successful tcgetattr above.
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &saved) };
            return Err(error).context("prepare passive output");
        }
        crate::engine::agent::register_interrupt_cleanup(move || {
            restore_terminal(&saved, output_flags);
        });
        let terminal = Self {
            saved,
            output_flags,
        };
        write_frame(b"\x1b[?1049h\x1b[?25l")?;
        Ok(terminal)
    }
}

fn write_frame(mut frame: &[u8]) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !frame.is_empty() {
        // SAFETY: frame is valid for its length; passive stdout is nonblocking.
        let written = unsafe { libc::write(1, frame.as_ptr().cast(), frame.len()) };
        if written > 0 {
            frame = &frame[written as usize..];
            continue;
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) {
            return Err(error).context("write passive terminal snapshot");
        }
        if Instant::now() >= deadline {
            bail!("passive terminal stopped reading; the retained Session is unchanged");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn restore_terminal(saved: &libc::termios, output_flags: libc::c_int) {
    // SAFETY: saved is a valid previous stdin configuration.
    unsafe { libc::tcsetattr(0, libc::TCSANOW, saved) };
    let reset = b"\x1b[0m\x1b[?25h\x1b[?1049l";
    // SAFETY: reset is valid; best-effort nonblocking output cannot trap cleanup.
    unsafe {
        libc::fcntl(1, libc::F_SETFL, output_flags | libc::O_NONBLOCK);
        libc::write(1, reset.as_ptr().cast(), reset.len());
        libc::fcntl(1, libc::F_SETFL, output_flags);
    }
}

impl Drop for PassiveTerminal {
    fn drop(&mut self) {
        restore_terminal(&self.saved, self.output_flags);
    }
}

#[cfg(test)]
mod tests {
    use super::{render_snapshot, SessionTerminal};

    #[test]
    fn passive_view_crops_without_resizing_application() {
        let screen = "history\n\x1b[38;2;17;93;201mabc界def\nlast";
        let rendered = String::from_utf8(render_snapshot(screen.as_bytes(), 1, 5, 1, 3)).unwrap();
        assert!(rendered.contains("\x1b[38;2;17;93;201m界def"));
        assert!(!rendered.contains("history"));
        assert!(!rendered.contains("last"));
        assert!(!rendered.contains("abc"));
    }

    #[test]
    #[ignore = "requires tmux with get-clipboard request support; run explicitly"]
    fn retained_screen_excludes_historical_queries() {
        let directory = tempfile::tempdir().unwrap();
        let terminal = SessionTerminal {
            directory: directory.path().to_path_buf(),
        };
        struct Cleanup<'a>(&'a SessionTerminal);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.command().arg("kill-server").output();
            }
        }
        let _cleanup = Cleanup(&terminal);
        terminal
            .start(
                directory.path(),
                "printf '\\033[38;2;17;93;201mretained draft\\033[0m\\033]52;c;?\\007'; sleep 5",
            )
            .unwrap();
        let mut screen = Vec::new();
        for _ in 0..100 {
            screen = terminal
                .command()
                .args(["capture-pane", "-p", "-e", "-t", "session:0.0"])
                .output()
                .unwrap()
                .stdout;
            if screen.windows(14).any(|part| part == b"retained draft") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(String::from_utf8_lossy(&screen).contains("retained draft"));
        assert!(!screen.windows(4).any(|part| part == b"\x1b]52"));
        assert!(terminal.is_running().unwrap());
    }
}
