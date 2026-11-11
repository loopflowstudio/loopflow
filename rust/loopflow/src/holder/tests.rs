use std::io::Write;
use std::time::{Duration, Instant};

use super::client::Client;
use super::wire::{Answer, Frame, Request, Start};
use super::{recorded_exit, serve, Place, Served};

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    place: Place,
    holder: std::thread::JoinHandle<Served>,
    _directory: tempfile::TempDir,
}

/// A holder serving on its own thread from a short private directory.
fn holder() -> Fixture {
    let directory = tempfile::Builder::new()
        .prefix("lfh")
        .tempdir_in("/tmp")
        .unwrap();
    let place = Place {
        socket: directory.path().join("holder.sock"),
        lock: directory.path().join("holder.lock"),
        state: directory.path().join("state"),
    };
    let (ready, listening) = std::sync::mpsc::channel();
    let served = place.clone();
    let holder =
        std::thread::spawn(move || serve(&served, BUILD, move || ready.send(()).unwrap()).unwrap());
    listening.recv_timeout(Duration::from_secs(5)).unwrap();
    Fixture {
        place,
        holder,
        _directory: directory,
    }
}

fn shell(terminal: &str, request: &str, script: &str) -> Start {
    Start {
        terminal: terminal.into(),
        request: request.into(),
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        rows: 24,
        cols: 80,
        placement: None,
    }
}

/// Every frame until the stream's exit, with output joined.
fn read_to_exit(place: &Place, terminal: &str, from: u64) -> (Vec<Frame>, Vec<u8>) {
    let mut output = Client::connect(place)
        .unwrap()
        .read(terminal, from)
        .unwrap();
    let mut frames = Vec::new();
    let mut bytes = Vec::new();
    loop {
        let frame = output.frame().unwrap();
        if let Frame::Out(out) = &frame {
            bytes.extend_from_slice(out);
        }
        let last = matches!(frame, Frame::Exit(_));
        frames.push(frame);
        if last {
            return (frames, bytes);
        }
    }
}

#[test]
fn a_second_holder_of_a_build_leaves_the_first_its_socket() {
    let fixture = holder();
    let first = Client::connect(&fixture.place).unwrap().hello().unwrap();

    // The lock decides, even when the first holder is too busy to answer.
    let second = serve(&fixture.place, BUILD, || {
        panic!("second holder bound the socket")
    })
    .unwrap();

    assert_eq!(second, Served::AlreadyHeld);
    let still = Client::connect(&fixture.place).unwrap().hello().unwrap();
    assert_eq!(still, first);
    assert_eq!(still.build, BUILD);
}

#[test]
fn a_child_runs_is_read_from_a_position_and_reports_its_exit() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    let pid = client
        .start(shell("t1", "r1", "printf hello; sleep 0.2; exit 3"))
        .unwrap();
    assert!(pid > 1);

    let (frames, bytes) = read_to_exit(&fixture.place, "t1", 0);
    assert_eq!(bytes, b"hello");
    let Some(Frame::Exit(exit)) = frames.last() else {
        panic!("stream ended without an exit: {frames:?}");
    };
    assert_eq!((exit.code, exit.signal, exit.written), (Some(3), None, 5));
    assert_eq!(exit.status(), 3);
    assert_eq!(
        recorded_exit(&fixture.place, "t1").unwrap().as_ref(),
        Some(exit)
    );

    // A later reader continues from its own position; nothing is replayed.
    let (_, tail) = read_to_exit(&fixture.place, "t1", 3);
    assert_eq!(tail, b"lo");
}

#[test]
fn repeating_a_start_answers_the_same_child() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    let pid = client.start(shell("t1", "r1", "sleep 5")).unwrap();

    assert_eq!(client.start(shell("t1", "r1", "sleep 5")).unwrap(), pid);
    let other = client.start(shell("t1", "r2", "sleep 5")).unwrap_err();
    assert!(other.to_string().contains("another request"), "{other:#}");
    assert_eq!(client.list().unwrap().len(), 1);
}

#[test]
fn reading_a_terminal_the_holder_does_not_hold_is_an_error() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client
        .start(shell("t1", "r1", "printf abc; sleep 5"))
        .unwrap();

    let unknown = Client::connect(&fixture.place)
        .unwrap()
        .read("t2", 0)
        .unwrap_err();
    assert!(unknown.to_string().contains("does not hold terminal t2"));

    // A position from another terminal's life is never answered from byte 0.
    let future = Client::connect(&fixture.place)
        .unwrap()
        .read("t1", 1_000_000)
        .unwrap_err();
    assert!(future.to_string().contains("cannot read from 1000000"));
}

#[test]
fn a_reader_behind_the_ring_is_told_where_it_resumes() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client
        .start(shell(
            "t1",
            "r1",
            "head -c 3000000 /dev/zero | tr '\\0' a; sleep 0.3",
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while client.list().unwrap()[0].exit.is_none() {
        assert!(Instant::now() < deadline, "child did not exit");
        std::thread::sleep(Duration::from_millis(50));
    }

    let (frames, bytes) = read_to_exit(&fixture.place, "t1", 0);
    let Frame::Gap(gap) = &frames[0] else {
        panic!("no gap before output: {:?}", frames.first());
    };
    assert_eq!(gap.ring_start, 3_000_000 - super::RING as u64);
    assert_eq!(bytes.len(), super::RING);
}

#[test]
fn input_and_size_reach_the_child() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client
        .start(shell(
            "t1",
            "r1",
            "stty -echo; read line; echo \"got $line\"; stty size",
        ))
        .unwrap();
    client.resize("t1", 30, 100).unwrap();

    let mut input = Client::connect(&fixture.place)
        .unwrap()
        .write("t1")
        .unwrap();
    input.write_all(b"hi\n").unwrap();

    let (_, bytes) = read_to_exit(&fixture.place, "t1", 0);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("got hi"), "{text}");
    assert!(text.contains("30 100"), "{text}");
}

#[test]
fn output_is_marked_with_the_line_discipline_it_was_written_under() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client
        .start(shell(
            "t1",
            "r1",
            "printf cooked; sleep 0.3; stty raw; printf raw; sleep 0.3; stty sane; sleep 0.3; printf bye",
        ))
        .unwrap();

    let (frames, _) = read_to_exit(&fixture.place, "t1", 0);
    let marked: Vec<(bool, String)> = frames
        .iter()
        .scan(false, |raw, frame| {
            Some(match frame {
                Frame::Mode(mode) => {
                    *raw = mode.raw;
                    None
                }
                Frame::Out(bytes) => Some((*raw, String::from_utf8_lossy(bytes).into_owned())),
                _ => None,
            })
        })
        .flatten()
        .collect();
    assert_eq!(
        marked,
        [
            (false, "cooked".to_owned()),
            (true, "raw".to_owned()),
            (false, "bye".to_owned())
        ]
    );

    // A reader that starts inside the raw stretch is told so first.
    let mut later = Client::connect(&fixture.place)
        .unwrap()
        .read("t1", 7)
        .unwrap();
    assert_eq!(
        later.frame().unwrap(),
        Frame::Mode(super::wire::Mode { raw: true })
    );
    assert_eq!(later.frame().unwrap(), Frame::Out(b"aw".to_vec()));
}

#[test]
fn one_terminal_ending_leaves_the_others_running() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client.start(shell("ends", "r1", "exit 0")).unwrap();
    client.start(shell("stays", "r2", "sleep 5")).unwrap();
    read_to_exit(&fixture.place, "ends", 0);

    let listed = client.list().unwrap();
    let exited: Vec<_> = listed
        .iter()
        .map(|held| (held.terminal.as_str(), held.exit.is_some()))
        .collect();
    assert_eq!(exited, [("ends", true), ("stays", false)]);

    assert!(client.forget("stays").is_err());
    client.forget("ends").unwrap();
    assert_eq!(client.list().unwrap().len(), 1);
}

#[test]
fn a_holder_exits_after_its_last_child() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    client.start(shell("t1", "r1", "exit 0")).unwrap();
    drop(client);

    assert_eq!(fixture.holder.join().unwrap(), Served::Finished);
    assert!(!fixture.place.socket.exists());
    assert!(Client::connect(&fixture.place).is_err());
}

#[test]
fn an_unreadable_request_is_answered_and_the_connection_stays() {
    let fixture = holder();
    let mut client = Client::connect(&fixture.place).unwrap();
    let answer = client.request(&Request::Resize {
        terminal: "missing".into(),
        rows: 1,
        cols: 1,
    });
    assert!(matches!(answer.unwrap(), Answer::Error { .. }));
    assert!(client.list().unwrap().is_empty());
}
