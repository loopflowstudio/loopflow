//! Descriptor changes and signal cleanup run only in disposable child processes.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::wire::{self, Decoder, Event, Incoming, Outgoing, Reply, Request, Tag};
use super::{attach_with_terminal, registry, terminal_pair, ViewerTerminal};
use crate::ghostty_vt::{Terminal, REVISION};
use crate::id::TerminalId;

const SESSION: &str = "session_5e55105e000040008000000000000001";
const MODES: &[u8] = b"\x1b[?2004h\x1b[?1h\x1b[>5u";

#[test]
fn snapshot_preview_fixture() {
    let Ok(case) = std::env::var("LOOPFLOW_VIEWER_FIXTURE") else {
        return;
    };
    ctrlc::set_handler(|| crate::agent::exit_on_interrupt()).unwrap();
    let home = crate::store::lf_home_dir();
    let mut saved = Terminal::new(80, 24).unwrap();
    saved.write(b"\x1b[?1049h");
    saved.write(MODES);
    saved.write(b"\x1b]4;1;rgb:12/34/56\x1b\\\x1b]10;rgb:ab/cd/ef\x1b\\\x1b[31msaved preview");
    let screens = home.join("screens");
    std::fs::create_dir_all(&screens).unwrap();
    let mut bytes = format!("lf-terminal-snapshot/1 ghostty={REVISION}\n").into_bytes();
    bytes.extend(saved.encode().unwrap());
    std::fs::write(screens.join(format!("{SESSION}.ghostsnp")), bytes).unwrap();

    if matches!(case.as_str(), "resume" | "reveal" | "provider_failure") {
        resume_fixture(&case);
        return;
    }
    let preview = ViewerTerminal::preview(Some(SESSION)).unwrap();
    assert!(preview.is_some());
    // This thread stands in for the server's held-terminal entrance.
    let terminal = TerminalId::new();
    let listener = UnixListener::bind(registry::entrance().unwrap()).unwrap();
    let execution = home.join("execution");
    std::fs::create_dir(&execution).unwrap();
    let reports_directory = case == "directory";
    let viewer_cwd = home.join("viewer-cwd");
    let peer = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut named = Vec::new();
        while !named.ends_with(b"\n") {
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).unwrap(), 1);
            named.push(byte[0]);
        }
        socket
            .write_all(
                format!(
                    "{{\"answer\":\"hosted\",\"revision\":{}}}\n",
                    registry::REVISION
                )
                .as_bytes(),
            )
            .unwrap();
        let mut decoder = Decoder::default();
        loop {
            let mut bytes = [0; 4096];
            let n = socket.read(&mut bytes).unwrap();
            assert_ne!(n, 0);
            decoder.extend(&bytes[..n]);
            if let Some((tag, payload)) = decoder.next().unwrap() {
                assert_eq!(tag, Tag::Request);
                let request: Incoming = serde_json::from_slice(&payload).unwrap();
                assert!(matches!(request.request, Request::Attach { .. }));
                break;
            }
        }
        if case == "cancel" {
            std::fs::write(home.join("pending"), "1").unwrap();
            let mut discarded = Vec::new();
            let _ = socket.read_to_end(&mut discarded);
            return;
        }
        if case == "startup_input" {
            std::fs::write(home.join("pending"), "1").unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !home.join("typed").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            // Give the relay a readable cooked line before acknowledging attach.
            std::thread::sleep(Duration::from_millis(250));
        }
        let mut live = Terminal::new(80, 24).unwrap();
        if case == "modes" {
            live.write(MODES);
        }
        let bytes = super::terminal::repaint(&live).unwrap();
        let reply = Outgoing {
            answers: 1,
            seq: 0,
            reply: Reply::Attached {
                viewer: "viewer".into(),
                nonce: "nonce".into(),
                token: Some("claim".into()),
                terminal: true,
                owner: Some("viewer".into()),
            },
        };
        socket
            .write_all(&wire::frame(
                Tag::Reply,
                &serde_json::to_vec(&reply).unwrap(),
            ))
            .unwrap();
        socket
            .write_all(&wire::frame(Tag::Snapshot, &bytes))
            .unwrap();
        if case == "completed" {
            // The agent finished before the viewer could acknowledge readiness.
            // Its final output and exit status are still waiting to be read.
            socket.shutdown(Shutdown::Read).unwrap();
            let mut output = wire::event(
                "nonce",
                1,
                Event::Return {
                    detach: "native-client".into(),
                },
            );
            output.extend_from_slice(b"provider finished\r\n");
            output.extend(wire::event("nonce", 2, Event::ChildExited { code: 0 }));
            socket.write_all(&wire::frame(Tag::Out, &output)).unwrap();
            return;
        }
        if reports_directory {
            socket
                .write_all(&wire::frame(
                    Tag::Out,
                    &wire::event("nonce", 1, Event::Directory { cwd: execution }),
                ))
                .unwrap();
        }
        if case == "startup_input" {
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut received = Vec::new();
            while !received.ends_with(b"\n") {
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).unwrap();
                assert_ne!(n, 0);
                decoder.extend(&bytes[..n]);
                while let Some((tag, payload)) = decoder.next().unwrap() {
                    if tag == Tag::Input {
                        received.extend(payload);
                    }
                }
            }
            assert_eq!(received, b"typed while opening\n");
            std::fs::write(home.join("forwarded"), received).unwrap();
        }
        // A marker outside the screen lets the parent inspect the blank handoff.
        socket
            .write_all(&wire::frame(Tag::Out, b"\x1b]777;live-ready\x07"))
            .unwrap();
        let mut input = Vec::new();
        socket.read_to_end(&mut input).unwrap();
        decoder.extend(&input);
        while let Some((tag, payload)) = decoder.next().unwrap() {
            assert_eq!(tag, Tag::Request, "preview input reached the new provider");
            let request: Incoming = serde_json::from_slice(&payload).unwrap();
            assert!(matches!(
                request.request,
                Request::Leave | Request::Controlled { .. }
            ));
        }
    });
    attach_with_terminal(&terminal, None, None, preview, true).unwrap();
    peer.join().unwrap();
    std::fs::write(
        viewer_cwd,
        std::env::current_dir()
            .unwrap()
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
}

fn resume_fixture(case: &str) {
    let succeeds = case != "provider_failure";
    let home = crate::store::lf_home_dir();
    let repo = loopflow_test_support::TestRepo::new();
    let binary = std::env::var("LOOPFLOW_TEST_LF").unwrap();
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let provider = bin.join("claude");
    std::fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
printf '%s\n' "$*" >> "$HOME/launches"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = || {
        let mut command = Command::new(&binary);
        command
            .env_clear()
            .env("HOME", &home)
            .env("LF_HOME", &home)
            .env("LF_BIN", &binary)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("RUST_LOG", "off")
            .current_dir(repo.path());
        command
    };
    let initial = command()
        .args(["-i", "--agent", "claude", ":", "original input"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(initial.status.success(), "{initial:?}");
    let sql = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let session: String = sql
        .query_row("SELECT id FROM agent_sessions", [], |r| r.get(0))
        .unwrap();
    // Only a scope's conversation, which stays open across exits, resumes
    // held behind the preview; any other runs bare in the terminal.
    sql.execute(
        "UPDATE agent_sessions SET primary_scope='repository', completed_at=NULL WHERE id=?1",
        [&session],
    )
    .unwrap();
    std::fs::rename(
        home.join(format!("screens/{SESSION}.ghostsnp")),
        home.join(format!("screens/{session}.ghostsnp")),
    )
    .unwrap();
    if !succeeds {
        std::fs::remove_file(&provider).unwrap();
    } else {
        std::fs::write(
            &provider,
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
stty -echo < /dev/tty
printf '%s\n' "$*" >> "$HOME/launches"
printf 'live primary\033]777;live-ready\007'
read -r answer < /dev/tty
"#,
        )
        .unwrap();
    }
    // Resuming a repository conversation starts it under the Machine server.
    let _server = loopflow_test_support::MachineServer::start(&binary, &home);
    let mut resume = command();
    resume.args(["resume", &session]);
    if case == "reveal" {
        resume.arg("--reveal-existing");
    }
    let result = resume.status().unwrap();
    assert_eq!(result.success(), succeeds);
    let launches = std::fs::read_to_string(home.join("launches")).unwrap();
    assert_eq!(launches.lines().count(), if succeeds { 2 } else { 1 });
    if succeeds {
        let resumed = launches.lines().nth(1).unwrap();
        assert!(resumed.contains("--resume"), "{resumed}");
        let after_name = resumed
            .strip_prefix("--name original input ")
            .expect("resume preserves the Session's title");
        assert!(
            !after_name.contains("original input"),
            "input replayed: {resumed}"
        );
    }
}

fn mode_replies(terminal: &mut Terminal) -> Vec<u8> {
    terminal.take_replies();
    terminal.write(b"\x1b[?2004$p\x1b[?1$p\x1b[?u");
    terminal.take_replies()
}

struct ViewerProcess(std::process::Child);

impl Drop for ViewerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn preview_case(case: &str) -> (tempfile::TempDir, Vec<u8>) {
    let resumes = matches!(case, "resume" | "reveal");
    // Prepare the source CLI before the isolated fixture, outside any Cargo run.
    let binary = if resumes || case == "provider_failure" {
        loopflow_test_support::lf_binary()
    } else {
        ""
    };
    let home = tempfile::tempdir().unwrap();
    let (mut master, slave) = terminal_pair().unwrap();
    let mut child = ViewerProcess(
        Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .env("LF_HOME", home.path())
            .env("LOOPFLOW_TEST_LF", binary)
            .env("LOOPFLOW_VIEWER_FIXTURE", case)
            .args([
                "--exact",
                "transport::viewer_tests::snapshot_preview_fixture",
                "--nocapture",
            ])
            .stdin(slave.try_clone().unwrap())
            .stdout(slave.try_clone().unwrap())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut display = Terminal::new(80, 24).unwrap();
    let theme = [0x87, 0x65, 0x43];
    display.set_theme(theme);
    display.write(b"shell prompt> ");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut output = Vec::new();
    let mut cancelled = false;
    let mut inspected = false;
    let mut typed = false;
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "fixture timed out: {}",
            String::from_utf8_lossy(&output)
        );
        let mut fd = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll receives one live descriptor.
        if unsafe { libc::poll(&mut fd, 1, 20) } > 0 {
            let mut bytes = [0; 16384];
            let n = master.read(&mut bytes).unwrap();
            display.write(&bytes[..n]);
            output.extend_from_slice(&bytes[..n]);
        }
        if case == "startup_input" && !typed && home.path().join("pending").exists() {
            master.write_all(b"typed while opening\n").unwrap();
            std::fs::write(home.path().join("typed"), "1").unwrap();
            typed = true;
        }
        if case == "cancel" && !cancelled && home.path().join("pending").exists() {
            assert!(display.alternate_screen());
            assert!(display.text().unwrap().contains("saved preview"));
            assert_eq!(display.foreground(0, 0), [0x12, 0x34, 0x56]);
            // SIGTERM exercises the registered cleanup; Rust Drop does not run.
            // SAFETY: this is the live child created by this fixture.
            unsafe { libc::kill(child.0.id() as i32, libc::SIGTERM) };
            cancelled = true;
        }
        if !inspected
            && output
                .windows(b"live-ready".len())
                .any(|w| w == b"live-ready")
        {
            assert!(!display.alternate_screen());
            if resumes {
                assert!(display.text().unwrap().contains("live primary"));
                assert!(!display.text().unwrap().contains("saved preview"));
            } else {
                assert!(display.text().unwrap().trim().is_empty());
                assert_eq!(display.cursor(), (0, 0));
            }
            let mut expected = Terminal::new(80, 24).unwrap();
            if case == "modes" {
                expected.write(MODES);
            }
            assert_eq!(mode_replies(&mut display), mode_replies(&mut expected));
            display.write(b"\x1b[H\x1b[31mX\x1b[0mY");
            assert_eq!(display.foreground(0, 0), theme);
            assert_eq!(display.foreground(1, 0), theme);
            master
                .write_all(if resumes { b"done\n" } else { &[0x1d] })
                .unwrap();
            inspected = true;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            // Drain cleanup written immediately before the child exited.
            // SAFETY: poll receives the same live descriptor as above.
            while unsafe { libc::poll(&mut fd, 1, 0) } > 0 {
                let mut bytes = [0; 16384];
                let n = master.read(&mut bytes).unwrap();
                if n == 0 {
                    break;
                }
                display.write(&bytes[..n]);
                output.extend_from_slice(&bytes[..n]);
            }
            break status;
        }
    };
    let mut error = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert_eq!(
        status.code(),
        Some(if case == "cancel" { 130 } else { 0 }),
        "{error}\n{}",
        String::from_utf8_lossy(&output)
    );
    assert!(!display.alternate_screen());
    let mut fresh = Terminal::new(80, 24).unwrap();
    assert_eq!(mode_replies(&mut display), mode_replies(&mut fresh));
    display.write(b"\x1b[H\x1b[31mX\x1b[0mY");
    assert_eq!(display.foreground(0, 0), theme);
    assert_eq!(display.foreground(1, 0), theme);
    // SAFETY: tcgetattr writes a termios for this live PTY.
    let attrs = unsafe {
        let mut attrs = std::mem::zeroed::<libc::termios>();
        assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut attrs), 0);
        attrs
    };
    assert_ne!(attrs.c_lflag & libc::ICANON, 0);
    assert_ne!(attrs.c_lflag & libc::ECHO, 0);
    assert!(output
        .windows(b"saved preview".len())
        .any(|w| w == b"saved preview"));
    assert!(matches!(case, "provider_failure" | "completed") || cancelled || inspected);
    (home, output)
}

#[test]
fn snapshot_preview_cancellation_before_acknowledgement_restores_terminal() {
    preview_case("cancel");
}
#[test]
fn snapshot_preview_hands_off_to_a_fresh_primary_screen() {
    preview_case("blank");
}
#[test]
fn snapshot_preview_hands_off_to_mode_only_live_state() {
    preview_case("modes");
}

#[test]
fn readiness_after_agent_exit_keeps_final_output_and_status() {
    let (_, output) = preview_case("completed");
    assert!(String::from_utf8_lossy(&output).contains("provider finished"));
}

#[test]
fn snapshot_preview_resume_uses_live_primary_output() {
    preview_case("resume");
}
#[test]
fn snapshot_preview_resume_provider_failure_restores_terminal() {
    preview_case("provider_failure");
}

#[test]
fn snapshot_preview_reveal_resumes_history_without_replaying_input() {
    preview_case("reveal");
}

const CLEAR_DIRECTORY: &[u8] = b"\x1b]7;\x1b\\";

/// Returns whether the viewer retired its report, after proving it entered the
/// execution directory: hosts read both, and they must not disagree.
fn directory_case(case: &str) -> bool {
    let (home, output) = preview_case(case);
    let entered = std::fs::read_to_string(home.path().join("viewer-cwd")).unwrap();
    assert_eq!(
        Path::new(&entered),
        home.path().join("execution").canonicalize().unwrap()
    );
    output
        .windows(CLEAR_DIRECTORY.len())
        .any(|w| w == CLEAR_DIRECTORY)
}

#[test]
fn viewer_taking_a_terminal_enters_the_execution_and_retires_its_report() {
    assert!(directory_case("directory"));
}

#[test]
fn snapshot_preview_keeps_typing_until_initial_attachment_is_acknowledged() {
    let (home, _) = preview_case("startup_input");
    assert_eq!(
        std::fs::read(home.path().join("forwarded")).unwrap(),
        b"typed while opening\n"
    );
}
