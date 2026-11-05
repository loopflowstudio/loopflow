//! Force policy/descriptor ordering in the real serve loop, in isolated children.

use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{bail, Result};

use super::lifetime::{Activity, Change, Ending};
use super::output::Output;
use super::wire::{Incoming, Request, Tag};
use crate::id::LfProcessId;

thread_local! {
    static CASE: RefCell<Option<Case>> = const { RefCell::new(None) };
}

struct Case {
    name: String,
    idle: bool,
    end: mpsc::Sender<Ending>,
    ending: Option<mpsc::Receiver<Ending>>,
    activity: mpsc::Sender<Activity>,
    changes: mpsc::Receiver<Activity>,
    viewer: Option<UnixStream>,
}

impl Case {
    fn queue(&self) {
        self.end
            .send(Ending {
                change: (!self.idle).then(|| (Change::Replaced, "replaced".into())),
                activity: Activity::default(),
            })
            .unwrap();
    }
}

pub(super) fn channels() -> Option<(mpsc::Sender<Activity>, mpsc::Receiver<Ending>)> {
    CASE.with_borrow_mut(|case| {
        let case = case.as_mut()?;
        Some((case.activity.clone(), case.ending.take().unwrap()))
    })
}

pub(super) fn before_read(
    owner: &LfProcessId,
    readers: &mut Vec<(u8, File)>,
    output_told: &mut Option<Instant>,
) -> Result<()> {
    CASE.with_borrow_mut(|case| -> Result<()> {
        let Some(case) = case else { return Ok(()) };
        if case.name == "throttled" {
            *output_told = Some(Instant::now());
        }
        if case.name != "unreported" {
            case.queue();
        }
        if case.name == "attach" {
            let mut viewer = UnixStream::connect(super::endpoint(owner)?)?;
            let incoming = Incoming {
                id: 1,
                request: Request::Attach {
                    control: false,
                    replace: true,
                    own: None,
                    rows: 24,
                    cols: 80,
                    name: None,
                    moved: None,
                    session: None,
                    placement: None,
                },
            };
            viewer.write_all(&super::wire::frame(
                Tag::Request,
                &serde_json::to_vec(&incoming)?,
            ))?;
            case.viewer = Some(viewer);
        } else {
            // A provider stream is already readable when the candidate is consumed.
            let (reader, mut writer) = UnixStream::pair()?;
            super::nonblocking(reader.as_raw_fd())?;
            writer.write_all(b"output after judgment\r\n")?;
            readers.push((1, File::from(OwnedFd::from(reader))));
        }
        Ok(())
    })
}

pub(super) fn before_judgment() {
    CASE.with_borrow(|case| {
        if let Some(case) = case.as_ref().filter(|case| case.name == "unreported") {
            // The read has finished; its notification has not reached the policy.
            // A candidate made from that older evidence must still be vetoed.
            case.queue();
        }
    });
}

pub(super) fn after_judgment(output: &Output, observed: Activity) -> Result<()> {
    CASE.with_borrow(|case| -> Result<()> {
        let Some(case) = case else { return Ok(()) };
        if case.name == "attach" {
            assert!(observed.viewed, "queued ending overtook the attach request");
        } else {
            assert!(output
                .terminal()
                .unwrap()
                .text()?
                .contains("output after judgment"));
            assert!(observed.output > 0);
        }
        // Veto flushes the newest evidence even if output reporting was throttled.
        assert_eq!(case.changes.try_iter().last(), Some(observed));
        bail!("ending candidate vetoed")
    })
}

#[test]
fn queued_endings_drain_output_and_accept_viewers() {
    for (name, idle) in [
        ("ready", true),
        ("ready", false),
        ("unreported", true),
        ("unreported", false),
        ("throttled", true),
        ("throttled", false),
        ("attach", true),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "transport::lifetime_tests::ending_order_fixture",
                "--nocapture",
            ])
            .env("LOOPFLOW_ENDING_FIXTURE", name)
            .env("LOOPFLOW_ENDING_IDLE", idle.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}/{idle}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn ending_order_fixture() {
    let Ok(name) = std::env::var("LOOPFLOW_ENDING_FIXTURE") else {
        return;
    };
    let guard = crate::journal::TestLedgerGuard::new();
    let (end, ending) = mpsc::channel();
    let (activity, changes) = mpsc::channel();
    CASE.set(Some(Case {
        name,
        idle: std::env::var("LOOPFLOW_ENDING_IDLE").unwrap() == "true",
        end,
        ending: Some(ending),
        activity,
        changes,
        viewer: None,
    }));
    crate::journal::with_runtime(guard.home(), &["lf".into(), "fixture".into()], || {
        let owner = crate::journal::current_lf_process_id().unwrap();
        let store = crate::journal::process_ledger()?;
        store.test_session("session", &crate::session_record::new_artifact_key());
        rusqlite::Connection::open(guard.home().join("loopflow.db"))?.execute(
            "UPDATE agent_sessions SET interactive=1, transport_lf_process_id=?1 WHERE id='session'",
            [owner.as_str()],
        )?;
        assert_eq!(store.owner_retention(&owner)?, None);
        let result = super::serve(&["/bin/sleep".into(), "30".into()], true);
        assert_eq!(result.unwrap_err().to_string(), "ending candidate vetoed");
        let _ = std::fs::remove_file(super::endpoint(&owner)?);
        Ok(())
    }).unwrap();
    CASE.set(None);
}
