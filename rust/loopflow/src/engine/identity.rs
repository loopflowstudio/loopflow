//! Wave/worker identity: the decoupled source of truth for how a unit of work
//! names itself on disk and on the remote.
//!
//! One thing, two deliberately non-derivable projections:
//! - [`WaveId::dir_component`] — flat, author-free, local:
//!   `bugs.fix-auth.20260706_0801`
//! - [`WaveId::branch`] — author-scoped, remote/glob-friendly:
//!   `jack/bugs.fix-auth.20260706_0801`
//!
//! The chain is lineage *as a hint*. Durable relationships live in explicit
//! Wave/Project/Task records and are never parsed back out of a name.
//!
//! Input is liberal, output is strict. [`WaveId::parse`] is the single funnel:
//! it accepts either surface form (with or without the `user/` prefix, with or
//! without the trailing worker stamp) and normalizes. Whatever you hand it, the
//! emitters produce the right shape for each projection.

use crate::engine::worktrees::WorktreeSegment;

/// A worker freshness stamp, `YYYYMMDD_HHMM`. Its presence on a [`WaveId`] is the
/// worker/subwave marker: a worker is ephemeral and stamped; a wave or subwave is
/// persistent and stamp-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp(String);

impl Timestamp {
    /// Parse the `YYYYMMDD_HHMM` shape. Returns `None` for anything else, which
    /// is how [`WaveId::parse`] tells a stamp from an ordinary chain segment.
    fn parse(raw: &str) -> Option<Self> {
        let (date, time) = raw.split_once('_')?;
        let shaped = date.len() == 8
            && date.bytes().all(|b| b.is_ascii_digit())
            && time.len() == 4
            && time.bytes().all(|b| b.is_ascii_digit());
        shaped.then(|| Self(raw.to_string()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// The identity of one node in the recursive wave tree — a wave, a subwave, or a
/// worker. `chain[0]` is the wave name; deeper segments are subwave/worker
/// lineage. `user` scopes the remote branch; `stamp` marks a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveId {
    chain: Vec<WorktreeSegment>,
    user: String,
    stamp: Option<Timestamp>,
}

impl WaveId {
    /// A bare, persistent wave: single-segment chain, no worker stamp.
    pub fn wave(user: impl Into<String>, wave: WorktreeSegment) -> Self {
        Self {
            chain: vec![wave],
            user: user.into(),
            stamp: None,
        }
    }

    /// A bare wave from a raw name, sanitized to a single segment. `None` if the
    /// name can't be a segment (empty, or contains a `.`).
    pub fn for_wave(user: impl Into<String>, wave_name: &str) -> Option<Self> {
        Some(Self::wave(user, WorktreeSegment::parse(wave_name).ok()?))
    }

    /// Descend one level: append `segment` to the chain and set the child's stamp
    /// (`Some` for a worker, `None` for a subwave). The parent's own stamp is
    /// dropped — the chain is stamp-free lineage, the stamp always the trailing
    /// leaf marker. So `a.b@ts` spawning worker `c` becomes `a.b.c@<new ts>`.
    pub fn child(&self, segment: WorktreeSegment, stamp: Option<Timestamp>) -> Self {
        let mut chain = self.chain.clone();
        chain.push(segment);
        Self {
            chain,
            user: self.user.clone(),
            stamp,
        }
    }

    /// The single input funnel. Accepts either projection:
    /// - branch form `jack/bugs.fix-auth.20260706_0801` (user before the `/`)
    /// - dir-component form `bugs.fix-auth.20260706_0801` (no user; `fallback_user`
    ///   fills it, since a dir omits the author)
    ///
    /// A trailing `YYYYMMDD_HHMM` segment is recognized as the worker stamp;
    /// otherwise the id is a wave/subwave. Returns `None` if nothing usable
    /// remains (empty, or a stamp with no chain).
    pub fn parse(raw: &str, fallback_user: &str) -> Option<Self> {
        let raw = raw.trim();
        let (user, rest) = match raw.split_once('/') {
            Some((user, rest)) => (user.trim(), rest),
            None => (fallback_user.trim(), raw),
        };
        if user.is_empty() {
            return None;
        }

        let mut parts: Vec<&str> = rest.split('.').filter(|s| !s.trim().is_empty()).collect();
        let stamp = parts.last().and_then(|last| Timestamp::parse(last));
        if stamp.is_some() {
            parts.pop();
        }
        if parts.is_empty() {
            return None;
        }

        let chain = parts
            .iter()
            .map(|s| WorktreeSegment::parse(s).ok())
            .collect::<Option<Vec<_>>>()?;

        Some(Self {
            chain,
            user: user.to_string(),
            stamp,
        })
    }

    /// The wave name — keys `wave/<name>/`, chat, pm. Never carries user or stamp.
    pub fn wave_name(&self) -> &str {
        self.chain[0].as_str()
    }

    /// The last chain segment — this node's own name within its parent.
    /// `bugs.fix-auth` → `fix-auth`; a bare wave → the wave name.
    pub fn leaf(&self) -> &str {
        self.chain
            .last()
            .map(WorktreeSegment::as_str)
            .unwrap_or_default()
    }

    /// The dot-joined lineage, stamp-free: `bugs.fix-auth`. Sorts into a
    /// pre-order tree walk, so ordering by it groups children under parents.
    pub fn chain_str(&self) -> String {
        self.chain
            .iter()
            .map(WorktreeSegment::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }

    /// A worker (ephemeral, stamped) vs. a wave/subwave (persistent).
    pub fn is_worker(&self) -> bool {
        self.stamp.is_some()
    }

    /// Chain length: 1 for a bare wave, +1 per subwave/worker level.
    pub fn depth(&self) -> usize {
        self.chain.len()
    }

    /// The parent branch name — the chain minus its last segment, unstamped.
    /// `None` for a bare wave (single-segment chain).
    pub fn parent(&self) -> Option<String> {
        if self.chain.len() < 2 {
            return None;
        }
        let parent = Self {
            chain: self.chain[..self.chain.len() - 1].to_vec(),
            user: self.user.clone(),
            stamp: None,
        };
        Some(parent.branch())
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    /// The worker stamp, if any — `Some` for a worker, `None` for a wave/subwave.
    pub fn timestamp(&self) -> Option<&str> {
        self.stamp.as_ref().map(Timestamp::as_str)
    }

    /// Flat, author-free local projection: `bugs.fix-auth.20260706_0801`.
    /// The worktree directory is `<repo>.<dir_component>`.
    pub fn dir_component(&self) -> String {
        let mut out = self
            .chain
            .iter()
            .map(WorktreeSegment::as_str)
            .collect::<Vec<_>>()
            .join(".");
        if let Some(stamp) = &self.stamp {
            out.push('.');
            out.push_str(stamp.as_str());
        }
        out
    }

    /// Author-scoped remote projection: `jack/bugs.fix-auth.20260706_0801`.
    /// `/` scopes the branch (glob-protectable `jack/**`) and never reaches the
    /// worktree path, which is built from [`dir_component`](Self::dir_component).
    pub fn branch(&self) -> String {
        format!("{}/{}", self.user, self.dir_component())
    }
}

#[cfg(test)]
mod tests {
    use super::{Timestamp, WaveId};
    use crate::engine::worktrees::WorktreeSegment;

    fn seg(s: &str) -> WorktreeSegment {
        WorktreeSegment::parse(s).unwrap()
    }

    #[test]
    fn timestamp_parses_only_the_shape() {
        assert!(Timestamp::parse("20260706_0801").is_some());
        assert!(Timestamp::parse("fix-auth").is_none());
        assert!(Timestamp::parse("2026_0801").is_none());
        assert!(Timestamp::parse("20260706_081").is_none());
    }

    #[test]
    fn wave_has_no_stamp_and_both_projections() {
        let id = WaveId::wave("jack", seg("bugs"));
        assert!(!id.is_worker());
        assert_eq!(id.wave_name(), "bugs");
        assert_eq!(id.dir_component(), "bugs");
        assert_eq!(id.branch(), "jack/bugs");
    }

    #[test]
    fn worker_child_appends_segment_and_stamps() {
        let wave = WaveId::wave("jack", seg("bugs"));
        let worker = wave.child(seg("fix-auth"), Timestamp::parse("20260706_0801"));
        assert!(worker.is_worker());
        assert_eq!(worker.wave_name(), "bugs");
        assert_eq!(worker.dir_component(), "bugs.fix-auth.20260706_0801");
        assert_eq!(worker.branch(), "jack/bugs.fix-auth.20260706_0801");
    }

    #[test]
    fn subworker_restamps_at_the_tail_not_after_the_parent_stamp() {
        // a.b@ts launching child c -> a.b.c@new_ts, never a.b.ts.c
        let worker = WaveId::wave("jack", seg("bugs"))
            .child(seg("fix-auth"), Timestamp::parse("20260706_0801"));
        let sub = worker.child(seg("retry"), Timestamp::parse("20260706_0930"));
        assert_eq!(sub.dir_component(), "bugs.fix-auth.retry.20260706_0930");
        assert_eq!(sub.branch(), "jack/bugs.fix-auth.retry.20260706_0930");
    }

    #[test]
    fn subwave_child_stays_stamp_free() {
        let sub = WaveId::wave("jack", seg("bugs")).child(seg("triage"), None);
        assert!(!sub.is_worker());
        assert_eq!(sub.dir_component(), "bugs.triage");
        assert_eq!(sub.branch(), "jack/bugs.triage");
    }

    #[test]
    fn parse_accepts_branch_form() {
        let id = WaveId::parse("jack/bugs.fix-auth.20260706_0801", "fallback").unwrap();
        assert_eq!(id.user(), "jack");
        assert_eq!(id.wave_name(), "bugs");
        assert!(id.is_worker());
        assert_eq!(id.dir_component(), "bugs.fix-auth.20260706_0801");
    }

    #[test]
    fn parse_accepts_dir_form_and_fills_user() {
        let id = WaveId::parse("bugs.fix-auth.20260706_0801", "jack").unwrap();
        assert_eq!(id.user(), "jack");
        assert_eq!(id.wave_name(), "bugs");
        // Same id regardless of which form we were handed.
        assert_eq!(id.branch(), "jack/bugs.fix-auth.20260706_0801");
    }

    #[test]
    fn parse_round_trips_both_projections() {
        let branch = "jack/bugs.fix-auth.20260706_0801";
        let dir = "bugs.fix-auth.20260706_0801";
        let from_branch = WaveId::parse(branch, "x").unwrap();
        let from_dir = WaveId::parse(dir, "jack").unwrap();
        assert_eq!(from_branch, from_dir);
        assert_eq!(from_branch.branch(), branch);
        assert_eq!(from_dir.dir_component(), dir);
    }

    #[test]
    fn leaf_and_chain_str_expose_lineage_for_the_tree_view() {
        let id = WaveId::parse("jack/bugs.fix-auth.20260706_0801", "x").unwrap();
        assert_eq!(id.leaf(), "fix-auth");
        assert_eq!(id.chain_str(), "bugs.fix-auth"); // stamp-free, sorts as a tree
        assert_eq!(id.depth(), 2);

        let wave = WaveId::parse("bugs", "jack").unwrap();
        assert_eq!(wave.leaf(), "bugs");
        assert_eq!(wave.chain_str(), "bugs");
    }

    #[test]
    fn parse_bare_wave_name() {
        let id = WaveId::parse("bugs", "jack").unwrap();
        assert!(!id.is_worker());
        assert_eq!(id.wave_name(), "bugs");
        assert_eq!(id.branch(), "jack/bugs");
    }

    #[test]
    fn parse_rejects_empty_and_stamp_only() {
        assert!(WaveId::parse("", "jack").is_none());
        assert!(WaveId::parse("20260706_0801", "jack").is_none());
        assert!(WaveId::parse("jack/", "fallback").is_none());
    }
}
