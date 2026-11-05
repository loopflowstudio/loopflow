//! A store lf cannot reach is a recording gap: the conversation goes on, and lf
//! logs once what it could not record and once more when its records resume.

use std::fmt::Display;
use std::path::PathBuf;

/// One line per changed store failure, however often a poll, retry or relay
/// meets it, then one line when the store answers again. A hook, which is a
/// fresh process at every boundary, keeps the open gap in a marker file.
#[derive(Debug, Default)]
pub struct RecordingGap {
    open: Option<GapSpan>,
    marker: Option<PathBuf>,
}

/// An open gap: what lf could not record, and when it first and last met the
/// failure, in Unix seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapSpan {
    pub what: String,
    pub first_miss: i64,
    pub last_miss: i64,
}

impl RecordingGap {
    /// A gap kept at `marker` across processes: open while the file exists.
    pub fn marked(marker: PathBuf) -> Self {
        let open = std::fs::read_to_string(&marker)
            .ok()
            .and_then(|text| parse(&text));
        Self {
            open,
            marker: Some(marker),
        }
    }

    /// `what` lf could not record and why. Repeating the same failure logs at
    /// debug; a different one warns again.
    pub fn note(&mut self, what: &str, error: &dyn Display) {
        let now = crate::store::rows::now_unix();
        let what = format!("{what}: {error}");
        if self.open.as_ref().is_some_and(|open| open.what == what) {
            tracing::debug!("recording gap continues: {what}");
        } else {
            tracing::warn!("recording gap: {what}; the conversation goes on without lf's records");
        }
        let first_miss = self.open.as_ref().map_or(now, |open| open.first_miss);
        let open = self.open.insert(GapSpan {
            what,
            first_miss,
            last_miss: now,
        });
        if let Some(marker) = &self.marker {
            if let Some(parent) = marker.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                marker,
                format!("{}\n{}\n{}\n", open.first_miss, open.last_miss, open.what),
            );
        }
    }

    pub fn span(&self) -> Option<&GapSpan> {
        self.open.as_ref()
    }

    /// The store answered: close the gap, if one was open.
    pub fn recovered(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        if let Some(marker) = &self.marker {
            let _ = std::fs::remove_file(marker);
        }
        tracing::info!("recording gap closed ({}); lf records from here", open.what);
    }
}

fn parse(text: &str) -> Option<GapSpan> {
    let mut lines = text.lines();
    let first_miss = lines.next()?.parse().ok()?;
    let last_miss = lines.next()?.parse().ok()?;
    Some(GapSpan {
        what: lines.next().unwrap_or_default().to_owned(),
        first_miss,
        last_miss,
    })
}

#[cfg(test)]
mod tests {
    use super::RecordingGap;

    #[test]
    fn a_marked_gap_survives_processes_and_closes_with_its_file() {
        let home = tempfile::tempdir().unwrap();
        let marker = home.path().join("deliveries").join("thread.store-gap");
        let mut gap = RecordingGap::marked(marker.clone());
        assert!(gap.span().is_none());
        gap.note("turn evidence", &"database is locked");
        let first = gap.span().unwrap().clone();
        assert_eq!(first.first_miss, first.last_miss);

        let mut later = RecordingGap::marked(marker.clone());
        later.note("steers", &"no such column: route");
        let span = later.span().unwrap();
        assert_eq!(span.first_miss, first.first_miss);
        assert!(span.last_miss >= span.first_miss);
        assert_eq!(span.what, "steers: no such column: route");

        later.recovered();
        assert!(later.span().is_none());
        assert!(!marker.exists());
        assert!(RecordingGap::marked(marker).span().is_none());
    }

    #[test]
    fn an_in_memory_gap_closes_once() {
        let mut gap = RecordingGap::default();
        gap.recovered();
        gap.note("task updates", &"busy");
        gap.note("task updates", &"busy");
        assert!(gap.span().is_some());
        gap.recovered();
        assert!(gap.span().is_none());
    }
}
