//! Real loopback OpenSSH, separate Machine stores, and Desktop's routed runtime.
//! Keys, daemon, configuration and known-hosts are disposable; no system service.
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{desktop_shell_bootstrap, Fixture};
use loopflow::store::sqlite::SqliteStore;

const TASK: &str = "task_5e55105e000040008000000000000007";

fn quote(value: &Path) -> String {
    shlex::try_quote(value.to_str().unwrap())
        .unwrap()
        .into_owned()
}

fn script(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn success(mut command: Command) -> String {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{command:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

struct Ssh {
    source: Fixture,
    destination: Fixture,
    daemon: Child,
    machine: String,
    bin: PathBuf,
    config: PathBuf,
}

impl Ssh {
    fn new() -> Self {
        let source = Fixture::new();
        let destination = Fixture::new();
        let machine = destination.shell_task(TASK);
        success(source.command(&["machine", "id"]));
        let source_store = SqliteStore::new(&source.home.path().join("loopflow.db")).unwrap();
        source_store
            .add_machine(&machine, "fixture", "fixture")
            .unwrap();
        let root = source.home.path();
        for name in ["host", "client", "unauthorized"] {
            let mut command = Command::new("ssh-keygen");
            command
                .args(["-q", "-t", "ed25519", "-N", "", "-f"])
                .arg(root.join(name));
            success(command);
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut user = Command::new("id");
        user.arg("-un");
        let user = success(user).trim().to_owned();
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        // A fixture-only PATH shim supplies ssh's config; all protocol/auth/PTY
        // traffic still uses real OpenSSH and production Machine routing.
        let config = root.join("ssh_config");
        script(
            &bin.join("ssh"),
            &format!(
                "#!/bin/sh\nexec /usr/bin/ssh -F {} \"$@\"\n",
                quote(&config)
            ),
        );
        let remote_bin = destination.home.path().join(".cargo/bin");
        std::fs::create_dir_all(&remote_bin).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), remote_bin.join("lf")).unwrap();
        let forced = root.join("destination");
        script(&forced, &format!("#!/bin/sh\ncd {} || exit 1\nexec /usr/bin/env -i HOME={} LF_HOME={} LF_BIN={} PATH=/usr/bin:/bin:/usr/sbin:/sbin SHELL=/bin/bash TERM=xterm-256color /bin/sh -c \"$SSH_ORIGINAL_COMMAND\"\n", quote(destination.home.path()), quote(destination.home.path()), quote(destination.home.path()), quote(Path::new(env!("CARGO_BIN_EXE_lf")))));
        let daemon_config = root.join("sshd_config");
        std::fs::write(&daemon_config, format!("Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nPidFile {}\nAuthorizedKeysFile {}\nStrictModes no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\nPermitRootLogin yes\nAllowUsers {user}\nForceCommand {}\nLogLevel ERROR\n", root.join("host").display(), root.join("sshd.pid").display(), root.join("client.pub").display(), forced.display())).unwrap();
        let host = std::fs::read_to_string(root.join("host.pub")).unwrap();
        std::fs::write(
            root.join("known_hosts"),
            format!("[127.0.0.1]:{port} {host}"),
        )
        .unwrap();
        std::fs::write(&config, format!("Host fixture\n HostName 127.0.0.1\n Port {port}\n User {user}\n IdentityFile {}\n IdentitiesOnly yes\n IdentityAgent none\n UserKnownHostsFile {}\n GlobalKnownHostsFile /dev/null\n", root.join("client").display(), root.join("known_hosts").display())).unwrap();
        drop(listener);
        let daemon = Command::new("/usr/sbin/sshd")
            .args(["-D", "-e", "-f"])
            .arg(daemon_config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(File::create(root.join("sshd.log")).unwrap())
            .spawn()
            .expect("isolated OpenSSH server is required");
        let mut fixture = Self {
            source,
            destination,
            daemon,
            machine: machine.to_string(),
            bin,
            config,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                fixture.daemon.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "sshd: {}",
                std::fs::read_to_string(fixture.source.home.path().join("sshd.log")).unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        fixture
    }

    fn source_command(&self, args: &[&str]) -> Command {
        let mut command = self.source.command(args);
        command.current_dir(self.source.home.path()).env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.bin.display()),
        );
        command
    }

    fn command(&self, id: &str, action: &str) -> Command {
        self.source_command(&[
            "--machine",
            &self.machine,
            "desktop",
            "shell",
            TASK,
            "--checkout",
            self.destination.repo.path().to_str().unwrap(),
            "--id",
            id,
            action,
        ])
    }

    fn control(&self, socket: &Path, operation: &str) -> Command {
        let mut command = Command::new("/usr/bin/ssh");
        command
            .arg("-F")
            .arg(&self.config)
            .arg("-S")
            .arg(socket)
            .args(["-O", operation, "fixture"]);
        command
    }

    fn sockets(&self) -> Vec<PathBuf> {
        let digest = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
            self.source
                .home
                .path()
                .canonicalize()
                .unwrap()
                .as_os_str()
                .as_encoded_bytes(),
        ));
        // SAFETY: geteuid has no preconditions.
        let root = PathBuf::from(format!(
            "/tmp/lf-ssh-{}-{}",
            unsafe { libc::geteuid() },
            &digest[..16]
        ));
        std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .map(|e| e.unwrap().path())
            .collect()
    }

    fn masters(&self) -> BTreeSet<i32> {
        self.sockets()
            .iter()
            .map(|socket| {
                let output = self.control(socket, "check").output().unwrap();
                assert!(output.status.success(), "{output:?}");
                String::from_utf8(output.stderr)
                    .unwrap()
                    .split("pid=")
                    .nth(1)
                    .unwrap()
                    .split(')')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect()
    }

    fn disconnect(&self) {
        for socket in self.sockets() {
            success(self.control(&socket, "exit"));
        }
    }

    fn census(&self) -> Vec<(i32, i32, String, String)> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,ppid=,stat=,tty="])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert_eq!(fields.len(), 4, "invalid process inventory: {line}");
                (
                    fields[0].parse().unwrap(),
                    fields[1].parse().unwrap(),
                    fields[2].into(),
                    fields[3].into(),
                )
            })
            .collect()
    }

    fn descendants(&self, roots: &[i32]) -> BTreeSet<i32> {
        let mut owned: BTreeSet<_> = roots.iter().copied().collect();
        let inventory = self.census();
        loop {
            let before = owned.len();
            for (pid, parent, _, _) in &inventory {
                if owned.contains(parent) {
                    owned.insert(*pid);
                }
            }
            if owned.len() == before {
                break;
            }
        }
        owned.remove(&(self.daemon.id() as i32));
        owned
    }

    fn assert_baseline(&self, owned: &BTreeSet<i32>) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let remaining: Vec<_> = self
                .census()
                .into_iter()
                .filter(|(pid, parent, state, _)| {
                    !state.starts_with('Z')
                        && (owned.contains(pid) || *parent == self.daemon.id() as i32)
                })
                .collect();
            if remaining.is_empty() && self.sockets().is_empty() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "retained processes/PTYs: {remaining:?}; SSH sockets: {:?}",
                self.sockets()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Ssh {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "isolated sshd: {}",
                std::fs::read_to_string(self.source.home.path().join("sshd.log"))
                    .unwrap_or_default()
            );
        }
        // Keep teardown effective after an assertion or failed startup too.
        for socket in self.sockets() {
            let _ = self.control(&socket, "exit").output();
        }
        // Join detached owners before the destination store and SSH server go away.
        loopflow_test_support::end_recorded_processes(
            self.destination.home.path(),
            Path::new(env!("CARGO_BIN_EXE_lf")),
        );
        for pid in self.descendants(&[self.daemon.id() as i32]) {
            // SAFETY: only this fixture's server descendants are selected.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

struct Pane {
    child: Child,
    terminal: File,
    screen: Vec<u8>,
    original_modes: libc::termios,
}

impl Pane {
    fn spawn(mut command: Command) -> Self {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: output pointers are valid; optional PTY settings are null.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: each freshly allocated descriptor is transferred exactly once.
        let terminal = unsafe { File::from_raw_fd(master) };
        // SAFETY: the slave is independently owned and valid.
        let slave = unsafe { File::from_raw_fd(slave) };
        // SAFETY: the parent owns this master; children must not retain it.
        assert_eq!(
            unsafe { libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
        // SAFETY: the master is live and tcgetattr initializes these attributes.
        let original_modes = unsafe {
            let mut modes = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(master, &mut modes), 0);
            modes
        };
        command
            .process_group(0)
            .stdin(slave.try_clone().unwrap())
            .stdout(slave.try_clone().unwrap())
            .stderr(slave);
        let child = command.spawn().unwrap();
        // SAFETY: fcntl changes only this fixture's master to nonblocking.
        assert_eq!(
            unsafe { libc::fcntl(terminal.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        Self {
            child,
            terminal,
            screen: Vec::new(),
            original_modes,
        }
    }

    fn assert_modes_restored(&self) {
        // SAFETY: the master remains owned by this pane after its viewer exits.
        let modes = unsafe {
            let mut modes = std::mem::zeroed::<libc::termios>();
            assert_eq!(libc::tcgetattr(self.terminal.as_raw_fd(), &mut modes), 0);
            modes
        };
        assert_eq!(modes.c_iflag, self.original_modes.c_iflag);
        assert_eq!(modes.c_oflag, self.original_modes.c_oflag);
        assert_eq!(modes.c_lflag, self.original_modes.c_lflag);
        assert_eq!(modes.c_cc, self.original_modes.c_cc);
    }

    fn input(&mut self, command: &str) {
        self.terminal.write_all(command.as_bytes()).unwrap();
    }

    fn read(&mut self) {
        let mut bytes = [0; 8192];
        while let Ok(count @ 1..) = self.terminal.read(&mut bytes) {
            self.screen.extend_from_slice(&bytes[..count]);
        }
    }

    fn until(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            self.read();
            if String::from_utf8_lossy(&self.screen).contains(text) {
                return;
            }
            assert!(
                self.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "missing {text}: {}",
                String::from_utf8_lossy(&self.screen)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            self.read();
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "pane did not exit: {}",
                String::from_utf8_lossy(&self.screen)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        // SAFETY: this fixture launched the child as its own process group.
        unsafe { libc::kill(-(self.child.id() as i32), libc::SIGTERM) };
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "sshd sandbox cannot nest inside test_network.py; run directly with --ignored"
)]
fn ssh_shell_lifecycles_reconnect_and_settle_processes_ptys_and_sockets() {
    let fixture = Ssh::new();
    // The destination's holder keeps each shell's PTY.
    let _server = fixture.destination.machine_server();
    for cycle in 0..2 {
        let id = format!("ssh-shell-{cycle}");
        let mut command = fixture.command(&id, "--create");
        command.args(["--bootstrap", &desktop_shell_bootstrap()]);
        command.args([
            "--",
            "/bin/sh",
            "-c",
            "printf 'launch\\n' >> \"$HOME/launches\"; printf 'remote-ready\\n'",
        ]);
        let mut pane = Pane::spawn(command);
        pane.until("remote-ready");
        let owner = fixture.destination.wait_shell_terminal(&id);
        pane.input("export RETAINED=destination; sleep 300 & printf '%s %s\\n' $$ $! > \"$HOME/pids\"; printf 'where:%s\\n' \"$PWD\"; tty\r");
        pane.until(&format!(
            "where:{}",
            fixture
                .destination
                .repo
                .path()
                .canonicalize()
                .unwrap()
                .display()
        ));
        pane.until("/dev/");
        let pids: Vec<i32> = fixture
            .destination
            .wait_file("pids")
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(pids.len(), 2);
        let shell = pids[0];
        let mut owned: BTreeSet<_> = pids.into_iter().collect();
        // Include the actual SSH client/master, server helpers, inner shell jobs
        // and their PTY names in the inventory, rather than counting receipts.
        let inventory = fixture.census();
        owned.extend(fixture.descendants(&[
            fixture.daemon.id() as i32,
            pane.child.id() as i32,
            shell,
        ]));
        assert!(
            inventory
                .iter()
                .any(|(pid, _, _, tty)| *pid == shell && tty != "??" && tty != "?"),
            "no inner PTY: {inventory:?}"
        );
        let masters = fixture.masters();
        assert_eq!(masters.len(), 1);
        owned.extend(&masters);
        // Abrupt network transport loss, with the destination owner still alive.
        for pid in masters {
            // SAFETY: ssh -O check identified this fixture's isolated master.
            assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
        }
        assert!(!pane.exit().success(), "connection loss must be reported");
        assert!(fixture.destination.held_running(&owner));
        let mut returning = Pane::spawn(fixture.command(&id, "--create"));
        fixture.destination.wait_held_controlled(&owner);
        returning.input("printf 'retained:%s:%s\\n' \"$RETAINED\" $$\r");
        returning.until(&format!("retained:destination:{shell}"));
        assert_eq!(fixture.destination.wait_shell_terminal(&id), owner);
        assert_eq!(
            fixture.destination.wait_file("launches").lines().count(),
            cycle + 1
        );
        owned.extend(fixture.descendants(&[
            fixture.daemon.id() as i32,
            returning.child.id() as i32,
            shell,
        ]));
        if cycle == 0 {
            returning.input("printf 'machine:'; lf machine id\r");
            returning.until(&format!("machine:{}", fixture.machine));
        }
        success(fixture.command(&id, "--close"));
        returning.exit();
        fixture.destination.held_ends(&owner);
        fixture.destination.shell_settles(&owner);
        owned.extend(fixture.masters());
        fixture.disconnect();
        fixture.assert_baseline(&owned);
        let settled: bool = fixture
            .destination
            .sql()
            .query_row(
                "SELECT jobs_settled FROM desktop_shells WHERE id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(settled, "cycle {cycle}");
        std::fs::remove_file(fixture.destination.home.path().join("pids")).unwrap();
    }
    let (receipts, settled): (u32, u32) = fixture
        .destination
        .sql()
        .query_row(
            "SELECT count(*),sum(jobs_settled) FROM desktop_shells",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((receipts, settled), (2, 2));
    assert_eq!(
        fixture.source.shell_count(),
        0,
        "remote panes must never create local shells"
    );
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "sshd sandbox cannot nest inside test_network.py; run directly with --ignored"
)]
fn ssh_shell_authentication_failure_never_falls_back_to_a_local_shell() {
    let fixture = Ssh::new();
    let _server = fixture.destination.machine_server();
    let original = std::fs::read_to_string(&fixture.config).unwrap();
    std::fs::write(
        &fixture.config,
        original.replace("/client\n", "/unauthorized\n"),
    )
    .unwrap();
    let mut denied = Pane::spawn(fixture.command("denied", "--create"));
    assert!(!denied.exit().success());
    assert!(
        String::from_utf8_lossy(&denied.screen).contains("Permission denied"),
        "{}",
        String::from_utf8_lossy(&denied.screen)
    );
    assert_eq!(fixture.destination.shell_count(), 0);
    std::fs::write(&fixture.config, original).unwrap();
    // A stale checkout and an ended shell are the destination's rules, the
    // same as a local Desktop shell's.
    fixture.disconnect();
    fixture.assert_baseline(&BTreeSet::new());
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "sshd sandbox cannot nest inside test_network.py; run directly with --ignored"
)]
fn ssh_prepared_viewer_reuses_held_terminal_without_an_extra_pty_and_releases_on_leave() {
    let fixture = Ssh::new();
    // The checkout's conversation is the Task's: the destination's server
    // starts it in a holder's terminal.
    let _server = fixture.destination.machine_server();
    let hold = |terminal: &str| -> Option<String> {
        fixture
            .destination
            .sql()
            .query_row(
                "SELECT viewer_hold FROM held_terminals WHERE id=?1",
                [terminal],
                |r| r.get(0),
            )
            .unwrap()
    };
    let wait_hold = |terminal: &str, expected: &str| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while hold(terminal).as_deref() != Some(expected) {
            assert!(
                Instant::now() < deadline,
                "viewer hold stayed {:?}, expected {expected:?}",
                hold(terminal)
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    script(&fixture.destination.home.path().join(".cargo/bin/claude"),
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '2.1.302'; exit; fi\nprintf 'provider-ready\\n'\nexec /bin/bash --noprofile --norc -i < /dev/tty\n");
    let record: serde_json::Value = serde_json::from_str(&success(fixture.source_command(&[
        "--machine",
        &fixture.machine,
        "--repo",
        fixture.destination.repo.path().to_str().unwrap(),
        "--agent",
        "claude",
        "resume",
        "--prepare",
        "--json",
    ])))
    .unwrap();
    let session = record["id"].as_str().unwrap();
    let argv: Vec<&str> = record["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(&argv[1..4], &["session", "attach", "--"]);
    assert_eq!(record["workspace"]["machine_id"], fixture.machine);
    let mut pane = Pane::spawn(fixture.source_command(&argv[1..]));
    pane.until("provider-ready");
    let terminal = fixture.destination.terminal_of(session);
    fixture.destination.wait_held_controlled(&terminal);
    pane.input("export RETAINED=original; printf '%s' $$ > \"$HOME/shell-pid\"; printf 'ready:%s\\n' \"$RETAINED\"\r");
    pane.until("ready:original");
    let shell_pid: i32 = fixture.destination.wait_file("shell-pid").parse().unwrap();
    // The shell's terminal is the holder's; the holder outlives this Session.
    let mut ancestors = Vec::new();
    let mut pid = shell_pid;
    while pid > 1 && ancestors.len() < 8 {
        let (parent, command) = super::command_line(pid);
        ancestors.push(command);
        pid = parent;
    }
    assert!(
        ancestors
            .iter()
            .any(|command| command.contains("holder run")),
        "{ancestors:?}"
    );
    let mut owned = fixture.descendants(&[
        fixture.daemon.id() as i32,
        pane.child.id() as i32,
        shell_pid,
    ]);
    // Only live processes can own a terminal; zombies can retain its name in ps.
    // Nothing SSH started holds a terminal: the agent's is the holder's.
    let inventory = fixture.census();
    let remote = fixture.descendants(&[fixture.daemon.id() as i32]);
    let terminals: Vec<_> = inventory
        .iter()
        .filter(|(pid, _, state, tty)| {
            !state.starts_with('Z') && remote.contains(pid) && tty != "??" && tty != "?"
        })
        .collect();
    assert!(terminals.is_empty(), "extra SSH PTY: {terminals:?}");

    // Here the hold reads viewed while the pane is open.
    wait_hold(&terminal, "viewed");
    // Initial dimensions, then a real local resize, both traverse the same stream.
    pane.input("stty size\r");
    pane.until("24 80");
    let dimensions = libc::winsize {
        ws_row: 37,
        ws_col: 111,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: the fixture owns this terminal and dimensions is a live winsize.
    assert_eq!(
        unsafe {
            libc::ioctl(
                pane.terminal.as_raw_fd(),
                libc::TIOCSWINSZ as _,
                &dimensions,
            )
        },
        0
    );
    std::thread::sleep(Duration::from_millis(500));
    pane.input("stty size\r");
    pane.until("37 111");
    pane.input("\x1d");
    assert!(pane.exit().success());
    pane.assert_modes_restored();
    wait_hold(&terminal, "absent");
    fixture.destination.assert_held_hosted(&terminal);

    let mut returning = Pane::spawn(fixture.source_command(&[
        "--machine",
        &fixture.machine,
        "resume",
        session,
        "--json",
    ]));
    fixture.destination.wait_held_controlled(&terminal);
    returning.input("printf 'retained:%s:%s\\n' \"$RETAINED\" $$\r");
    returning.until(&format!("retained:original:{shell_pid}"));
    owned.extend(fixture.descendants(&[fixture.daemon.id() as i32, returning.child.id() as i32]));
    returning.input("exit 0\r");
    assert_eq!(
        returning.exit().code(),
        Some(0),
        "{}\nowner: {}",
        String::from_utf8_lossy(&returning.screen),
        std::fs::read_to_string(fixture.destination.home.path().join("logs/lf.log"))
            .unwrap_or_default()
    );
    returning.assert_modes_restored();
    let returned = String::from_utf8_lossy(&returning.screen).replace("\r\n", "\n");
    let json = returned
        .find("{\n")
        .expect("Session JSON follows the returned viewer");
    let returned: serde_json::Value = serde_json::from_str(returned[json..].trim()).unwrap();
    assert_eq!(returned["id"], session);
    assert_eq!(fixture.destination.terminal_of(session), terminal);
    fixture.destination.held_ends(&terminal);
    owned.extend(fixture.masters());
    fixture.disconnect();
    fixture.assert_baseline(&owned);
}
