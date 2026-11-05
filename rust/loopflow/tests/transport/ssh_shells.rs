//! Real loopback OpenSSH, separate Machine stores, and the public routed shell CLI.
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

use super::{Fixture, ShellOwnerGuard};
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

    fn command(&self, id: &str, action: &str) -> Command {
        let mut command = self.source.command(&[
            "--machine",
            &self.machine,
            "task",
            "shell",
            TASK,
            "--checkout",
            self.destination.repo.path().to_str().unwrap(),
            "--id",
            id,
            action,
        ]);
        // Outside a repository: the destination resolves the Task itself.
        command.current_dir(self.source.home.path()).env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.bin.display()),
        );
        command
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
        // Startup can fail before the shell's per-test guard is constructed.
        // Stop its owner while the receipt store still exists, then let it settle
        // its terminal jobs before TempDir removes the store.
        if let Ok(sql) =
            rusqlite::Connection::open(self.destination.home.path().join("loopflow.db"))
        {
            let _ = sql.execute("UPDATE task_shells SET close_requested=1", []);
            if let Ok(mut query) = sql.prepare("SELECT p.pid FROM task_shells s JOIN processes p ON p.id=s.owner WHERE p.completed_at IS NULL AND p.pid IS NOT NULL") {
                let owners: Vec<i32> = query.query_map([], |r| r.get(0)).unwrap().filter_map(Result::ok).collect();
                for pid in &owners {
                    // SAFETY: these are the owners launched in this fixture's store.
                    unsafe { libc::kill(*pid, libc::SIGTERM) };
                }
                let deadline = Instant::now() + Duration::from_secs(10);
                while !owners.is_empty() && Instant::now() < deadline {
                    let live = self.census().into_iter().any(|(pid, _, state, _)| owners.contains(&pid) && !state.starts_with('Z'));
                    if !live { break; }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
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
        }
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
    for cycle in 0..20 {
        let id = format!("ssh-shell-{cycle}");
        let mut command = fixture.command(&id, "--create");
        command.args([
            "--",
            "/bin/sh",
            "-c",
            "printf 'launch\\n' >> \"$HOME/launches\"; printf 'remote-ready\\n'",
        ]);
        let mut pane = Pane::spawn(command);
        pane.until("remote-ready");
        let owner = fixture.destination.shell_owner(&id);
        let _stop = ShellOwnerGuard(&fixture.destination, owner.clone());
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
        let owner_pid: i32 = fixture
            .destination
            .sql()
            .query_row("SELECT pid FROM processes WHERE id=?1", [&owner], |r| {
                r.get(0)
            })
            .unwrap();
        owned.insert(owner_pid);
        // Include the actual SSH client/master, server helpers, inner shell jobs
        // and their PTY names in the inventory, rather than counting receipts.
        let inventory = fixture.census();
        owned.extend(fixture.descendants(&[
            fixture.daemon.id() as i32,
            pane.child.id() as i32,
            owner_pid,
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
        assert!(fixture.destination.running(&owner));
        let mut returning = Pane::spawn(fixture.command(&id, "--create"));
        fixture.destination.wait_controlled(&owner);
        returning.input("printf 'retained:%s:%s\\n' \"$RETAINED\" $$\r");
        returning.until(&format!("retained:destination:{shell}"));
        assert_eq!(fixture.destination.shell_owner(&id), owner);
        assert_eq!(
            fixture.destination.wait_file("launches").lines().count(),
            cycle + 1
        );
        owned.extend(fixture.descendants(&[
            fixture.daemon.id() as i32,
            returning.child.id() as i32,
            owner_pid,
        ]));
        if cycle == 0 {
            returning.input("printf 'machine:'; lf machine id\r");
            returning.until(&format!("machine:{}", fixture.machine));
        }
        success(fixture.command(&id, "--close"));
        returning.exit();
        fixture.destination.ends(&owner);
        owned.extend(fixture.masters());
        fixture.disconnect();
        fixture.assert_baseline(&owned);
        assert!(!fixture.destination.endpoint(&owner).exists());
        let settled: bool = fixture
            .destination
            .sql()
            .query_row(
                "SELECT jobs_settled FROM task_shells WHERE id=?1",
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
            "SELECT count(*),sum(jobs_settled) FROM task_shells",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((receipts, settled), (20, 20));
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
fn ssh_shell_failures_never_fall_back_or_recreate_ended_execution() {
    let fixture = Ssh::new();
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

    let checkout = fixture.destination.repo.path();
    let moved = checkout.with_extension("unavailable");
    std::fs::rename(checkout, &moved).unwrap();
    let mut stale = Pane::spawn(fixture.command("stale", "--create"));
    let status = stale.exit();
    std::fs::rename(&moved, checkout).unwrap();
    assert!(!status.success());
    assert!(
        String::from_utf8_lossy(&stale.screen).contains("checkout is unavailable"),
        "{}",
        String::from_utf8_lossy(&stale.screen)
    );
    assert_eq!(fixture.destination.shell_count(), 0);

    // Authentication and checkout recovery allow a deliberate new shell only.
    let mut command = fixture.command("exit", "--create");
    command.args(["--", "/bin/sh", "-c", "printf 'ready-to-exit\\n'"]);
    let mut pane = Pane::spawn(command);
    pane.until("ready-to-exit");
    let owner = fixture.destination.shell_owner("exit");
    let _stop = ShellOwnerGuard(&fixture.destination, owner.clone());
    pane.input("exit 7\r");
    assert_eq!(pane.exit().code(), Some(7));
    fixture.destination.ends(&owner);
    let mut retry = Pane::spawn(fixture.command("exit", "--create"));
    assert!(!retry.exit().success());
    assert!(String::from_utf8_lossy(&retry.screen).contains("Shell ended"));
    assert_eq!(fixture.destination.shell_owner("exit"), owner);
    assert_eq!(fixture.source.shell_count(), 0);
    assert_eq!(fixture.destination.shell_count(), 1);
    fixture.disconnect();
    fixture.assert_baseline(&BTreeSet::new());
    assert!(!fixture.destination.endpoint(&owner).exists());
}
