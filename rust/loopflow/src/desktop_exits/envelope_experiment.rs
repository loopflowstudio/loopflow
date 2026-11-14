//! Isolated reporter comparison; not connected to doctor until reporter review.
//! Reads envelope JSON and minidump metadata, never stack/context/memory streams.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;

#[derive(Debug, PartialEq)]
struct Crash {
    envelope: PathBuf,
    attachment: String,
    executable: String,
    pid: u32,
    created: u32,
    at: u32,
    exception: u32,
}

#[derive(Debug)]
struct Launch<'a> {
    executable: &'a str,
    pid: u32,
    created: u32,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl Crash {
    fn describe(&self) -> String {
        let exception = match self.exception {
            1 => "EXC_BAD_ACCESS".to_owned(),
            code => format!("Mach exception {code}"),
        };
        format!(
            "{exception}; minidump attachment {} in {} (not extracted)",
            self.attachment,
            self.envelope.display()
        )
    }
}

fn correlate<'a>(launch: &Launch<'_>, crashes: &'a [Crash]) -> Option<&'a Crash> {
    let mut matches = crashes.iter().filter(|crash| {
        let at = DateTime::<Utc>::from_timestamp(i64::from(crash.at), 0)
            .expect("a u32 epoch second fits chrono's range");
        crash.executable == launch.executable
            && crash.pid == launch.pid
            && crash.created == launch.created
            // Dump timestamps have one-second precision. The whole bucket must
            // fit the launch interval, or this evidence cannot choose a launch.
            && at >= launch.start
            && at + TimeDelta::seconds(1) <= launch.end
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn read_at(
    reader: &mut (impl Read + Seek),
    base: u64,
    size: u64,
    at: u32,
    length: usize,
) -> Option<Vec<u8>> {
    if u64::from(at).checked_add(length as u64)? > size {
        return None;
    }
    reader
        .seek(SeekFrom::Start(base.checked_add(u64::from(at))?))
        .ok()?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn metadata(reader: &mut (impl Read + Seek), base: u64, size: u64) -> Option<(u32, u32, u32, u32)> {
    let header = read_at(reader, base, size, 0, 32)?;
    if header.get(..4)? != b"MDMP" || word(&header, 4)? & 0xffff != 0xa793 {
        return None;
    }
    let count = word(&header, 8)?;
    if count > 128 {
        return None;
    }
    let directory = read_at(reader, base, size, word(&header, 12)?, count as usize * 12)?;
    let mut process = None;
    let mut exception = None;
    for entry in directory.as_chunks::<12>().0 {
        let kind = word(entry, 0)?;
        let length = word(entry, 4)?;
        let rva = word(entry, 8)?;
        match kind {
            15 if length >= 24 && process.is_none() => {
                let misc = read_at(reader, base, size, rva, 24)?;
                // MDMiscInfo must carry both process ID and process times.
                if word(&misc, 0)? < 24 || word(&misc, 4)? & 3 != 3 {
                    return None;
                }
                process = Some((word(&misc, 8)?, word(&misc, 12)?));
            }
            6 if length >= 168 && exception.is_none() => {
                // Only the exception code. Context/stack pointers are ignored.
                exception = Some(word(&read_at(reader, base, size, rva, 12)?, 8)?);
            }
            6 | 15 => return None,
            _ => {} // In particular 3/5/9 (thread/memory lists) are never read.
        }
    }
    let (pid, created) = process?;
    Some((pid, created, word(&header, 20)?, exception?))
}

fn line(reader: &mut impl Read) -> Option<String> {
    let mut bytes = Vec::new();
    for _ in 0..8192 {
        let mut byte = [0];
        reader.read_exact(&mut byte).ok()?;
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            return String::from_utf8(bytes).ok();
        }
    }
    None
}

fn read_envelope(reader: &mut (impl Read + Seek), path: &Path) -> Option<Crash> {
    let header = line(reader)?;
    if !serde_json::from_str::<Value>(&header).ok()?.is_object() {
        return None;
    }
    let end = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(header.len() as u64)).ok()?;
    let mut event = None;
    let mut dump = None;
    while reader.stream_position().ok()? < end {
        let item: Value = serde_json::from_str(&line(reader)?).ok()?;
        let length = item.get("length")?.as_u64()?;
        let base = reader.stream_position().ok()?;
        let next = base.checked_add(length)?;
        if next > end {
            return None;
        }
        match item.get("type")?.as_str()? {
            "event" if event.is_none() && length <= 1024 * 1024 => {
                let mut body = vec![0; length as usize];
                reader.read_exact(&mut body).ok()?;
                event = Some(serde_json::from_slice::<Value>(&body).ok()?);
            }
            "attachment"
                if item.get("attachment_type").and_then(Value::as_str)
                    == Some("event.minidump") =>
            {
                if dump.is_some() {
                    return None;
                }
                let name = item.get("filename")?.as_str()?.to_owned();
                dump = Some((name, metadata(reader, base, length)?));
            }
            "event" => return None,
            _ => {}
        }
        reader.seek(SeekFrom::Start(next)).ok()?;
        if next < end {
            let mut separator = [0];
            reader.read_exact(&mut separator).ok()?;
            if separator != *b"\n" {
                return None;
            }
        }
    }
    let event = event?;
    if event.get("level")?.as_str()? != "fatal" {
        return None;
    }
    // The main image is emitted first by sentry-native's Apple module finder.
    let executable = event
        .pointer("/debug_meta/images/0/code_file")?
        .as_str()?
        .to_owned();
    let (attachment, (pid, created, at, exception)) = dump?;
    Some(Crash {
        envelope: path.into(),
        attachment,
        executable,
        pid,
        created,
        at,
        exception,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Seek, SeekFrom};
    use std::path::Path;

    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::{correlate, read_envelope, Crash, Launch};

    const EXE: &str = "/Applications/Loopflow.app/Contents/MacOS/Loopflow";

    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn envelope() -> (Vec<u8>, std::ops::Range<u64>) {
        let event = serde_json::to_vec(
            &json!({"level":"fatal", "debug_meta":{"images":[{"code_file":EXE}]}}),
        )
        .unwrap();
        let mut dump = vec![0; 384];
        dump[..4].copy_from_slice(b"MDMP");
        for (at, value) in [
            (4, 0xa793),
            (8, 3),
            (12, 32),
            (20, 100),
            (32, 15),
            (36, 24),
            (40, 68),
            (44, 6),
            (48, 168),
            (52, 92),
            (56, 5),
            (60, 124),
            (64, 260),
            (68, 24),
            (72, 3),
            (76, 42),
            (80, 50),
            (100, 1),
        ] {
            put(&mut dump, at, value);
        }
        let mut bytes =
            format!("{{}}\n{{\"type\":\"event\",\"length\":{}}}\n", event.len()).into_bytes();
        bytes.extend(event);
        bytes.extend(format!("\n{{\"type\":\"attachment\",\"attachment_type\":\"event.minidump\",\"filename\":\"crash.dmp\",\"length\":{}}}\n", dump.len()).as_bytes());
        let base = bytes.len() as u64;
        bytes.extend(dump);
        (bytes, base + 260..base + 384)
    }

    fn launch() -> Launch<'static> {
        Launch {
            executable: EXE,
            pid: 42,
            created: 50,
            start: Utc.timestamp_opt(51, 0).unwrap(),
            end: Utc.timestamp_opt(101, 0).unwrap(),
        }
    }

    fn crash() -> Crash {
        read_envelope(
            &mut Cursor::new(envelope().0),
            Path::new("synthetic.envelope"),
        )
        .unwrap()
    }

    #[test]
    fn fatal_native_metadata_correlates_without_reading_process_memory() {
        // Instrument every read, including accidental read-ahead into memory.
        struct NoMemory {
            cursor: Cursor<Vec<u8>>,
            memory: std::ops::Range<u64>,
        }
        impl Read for NoMemory {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                let start = self.cursor.position();
                assert!(
                    start >= self.memory.end || start + bytes.len() as u64 <= self.memory.start
                );
                self.cursor.read(bytes)
            }
        }
        impl Seek for NoMemory {
            fn seek(&mut self, at: SeekFrom) -> std::io::Result<u64> {
                self.cursor.seek(at)
            }
        }
        let (bytes, memory) = envelope();
        let found = read_envelope(
            &mut NoMemory {
                cursor: Cursor::new(bytes),
                memory,
            },
            Path::new("synthetic.envelope"),
        )
        .unwrap();
        assert_eq!(found.exception, 1); // EXC_BAD_ACCESS, from metadata, not exit status.
        assert_eq!(
            found.describe(),
            "EXC_BAD_ACCESS; minidump attachment crash.dmp in synthetic.envelope (not extracted)"
        );
        assert_eq!(
            correlate(&launch(), &[found]).unwrap().envelope,
            Path::new("synthetic.envelope")
        );
    }

    #[test]
    fn unrelated_apps_dev_and_reused_pids_stay_unknown() {
        for executable in [
            "/Applications/Ghostty.app/Contents/MacOS/ghostty",
            "/Applications/Loopflow Dev.app/Contents/MacOS/Loopflow Dev",
        ] {
            let mut other = crash();
            other.executable = executable.into();
            assert!(correlate(&launch(), &[other]).is_none());
        }
        let mut reused = crash();
        reused.created = 60;
        assert!(correlate(&launch(), &[reused]).is_none());
        let mut other = crash();
        other.pid = 43;
        assert!(correlate(&launch(), &[other]).is_none());
    }

    #[test]
    fn outside_interval_and_ambiguous_evidence_stay_unknown() {
        for at in [50, 101] {
            let mut other = crash();
            other.at = at;
            assert!(correlate(&launch(), &[other]).is_none());
        }
        assert!(correlate(&launch(), &[crash(), crash()]).is_none());
        assert!(correlate(&launch(), &[]).is_none()); // rotated/missing
        let mut partial = launch();
        partial.start = Utc.timestamp_opt(100, 1).unwrap();
        assert!(correlate(&partial, &[crash()]).is_none());
    }

    #[test]
    fn torn_or_unrecognized_envelopes_stay_unknown() {
        let bytes = envelope().0;
        for length in 0..bytes.len() {
            assert!(read_envelope(&mut Cursor::new(&bytes[..length]), Path::new("torn")).is_none());
        }
        let mut bad = bytes.clone();
        let offset = bytes.windows(4).position(|part| part == b"MDMP").unwrap();
        bad[offset] = 0;
        assert!(read_envelope(&mut Cursor::new(bad), Path::new("bad")).is_none());
        let mut nonfatal = bytes.clone();
        let level = bytes.windows(5).position(|part| part == b"fatal").unwrap();
        nonfatal[level..level + 5].copy_from_slice(b"error");
        assert!(read_envelope(&mut Cursor::new(nonfatal), Path::new("error")).is_none());
    }
}
