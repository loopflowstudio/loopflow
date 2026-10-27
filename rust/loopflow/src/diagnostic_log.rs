//! `logs/lf.log` in the selected home: where every lf process writes its
//! diagnostics, each line ending with the process and binary that wrote it.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use crate::build_info::{short_revision, source_revision, BUILD_VERSION};

/// How much of lf.log readers take back: hours of warnings, not the history.
const TAIL_BYTES: u64 = 16 * 1024 * 1024;

pub fn log_path() -> PathBuf {
    crate::store::lf_home_dir().join("logs").join("lf.log")
}

/// Appends to lf.log, opened per event so a command that logs nothing creates no home.
#[derive(Debug)]
pub struct DiagnosticLog;

impl Write for DiagnosticLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let path = log_path();
        let log = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
            });
        match log {
            Ok(mut log) => log.write_all(bytes),
            // An unwritable home still runs; its diagnostics stay visible.
            Err(_) => std::io::stderr().write_all(bytes),
        }?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Ends every diagnostic line with the process and binary that wrote it, so a
/// lock failure from one of dozens of concurrent lf processes is attributable.
#[derive(Debug)]
pub struct IdentifiedLines;

impl<S, N> FormatEvent<S, N> for IdentifiedLines
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let mut line = String::new();
        tracing_subscriber::fmt::format().format_event(ctx, Writer::new(&mut line), event)?;
        writeln!(
            writer,
            "{} pid={} lf={} rev={}",
            line.trim_end_matches('\n'),
            std::process::id(),
            BUILD_VERSION,
            short_revision(source_revision()),
        )
    }
}

/// The last `name=value` on a line; identity fields close every line.
pub(crate) fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let start = line.rfind(&format!(" {name}="))? + name.len() + 2;
    line[start..].split_whitespace().next()
}

/// The end of the log at `path`, starting at a line boundary. A missing log is empty.
pub(crate) fn read_tail(path: &Path) -> std::io::Result<String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error),
    };
    let offset = file.metadata()?.len().saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if offset == 0 {
        return Ok(text);
    }
    Ok(text
        .split_once('\n')
        .map(|(_, rest)| rest.to_owned())
        .unwrap_or_default())
}
