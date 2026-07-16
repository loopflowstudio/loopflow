//! Evidence receipts: the stable pointer from a curated claim to the raw record
//! that justifies it.
//!
//! A curated claim (a `MEMORY.md` fact, a KR proof, an executive Wave report)
//! is a human-readable summary. Left alone it becomes a second, unaccountable
//! truth. A [`Receipt`] binds one claim to one canonical record already in the
//! system — a chat turn, a worker report, a trace turn, a PM change, or a PR —
//! by that record's own **durable id**. The receipt is deliberately small: it
//! points, it never copies the transcript.
//!
//! Identity is chosen so a receipt survives the perturbations curation outlives:
//! Markdown edits (the receipt is structural, not a line number), branch
//! deletion (a `pr` reference carries the merge commit sha), Session succession
//! (references name journal/trace/Linear ids, never a session id), and database
//! migration (the journal is JSONL, trace rows are UUID-keyed, Linear ids are
//! external, the PM snapshot is a rebuildable read model).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Which kind of raw record a receipt points at. The wire token (snake_case) is
/// also the CLI token in `kind:reference` authoring.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A wave chat turn (journal `turn_id`) — a report or assistant statement.
    ChatTurn,
    /// A worker/child run report (`run_id` in the journal / `run_events`).
    WorkerReport,
    /// A trace turn (`agent_turns` UUID) — one provider-facing exchange.
    Trace,
    /// A project-management change (Linear issue UUID; survives renumbering).
    Pm,
    /// A pull request (`owner/repo#N`, optionally `@<merge_sha>`).
    Pr,
}

impl EvidenceKind {
    /// The canonical wire/CLI token.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::ChatTurn => "chat_turn",
            Self::WorkerReport => "worker_report",
            Self::Trace => "trace",
            Self::Pm => "pm",
            Self::Pr => "pr",
        }
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_token())
    }
}

impl FromStr for EvidenceKind {
    type Err = ReceiptParseError;

    fn from_str(token: &str) -> Result<Self, Self::Err> {
        match token {
            "chat_turn" => Ok(Self::ChatTurn),
            // `run` is an ergonomic alias for the `run_id` that names a report.
            "worker_report" | "run" => Ok(Self::WorkerReport),
            "trace" => Ok(Self::Trace),
            "pm" => Ok(Self::Pm),
            "pr" => Ok(Self::Pr),
            other => Err(ReceiptParseError::UnknownKind(other.to_string())),
        }
    }
}

/// A stable pointer from a curated claim to the record that justifies it.
///
/// Wire type: every field is required, no serde defaults. A claim carries a
/// `Vec<Receipt>`; one-to-one is the degenerate case and many-to-one stays
/// compact because each entry is an id, not a copied record.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub kind: EvidenceKind,
    /// The canonical durable id for `kind`: a `turn_id`, `run_id`, trace UUID,
    /// Linear issue UUID, or `owner/repo#N[@sha]`. Never a line number, a copied
    /// raw record, or a session id.
    pub reference: String,
    /// The wave that owns the referenced record. Present so a receipt pointing
    /// outside the claim's own wave is detectable; authoring defaults it to the
    /// claim's wave.
    pub wave: String,
}

impl Receipt {
    pub fn new(kind: EvidenceKind, reference: impl Into<String>, wave: impl Into<String>) -> Self {
        Self {
            kind,
            reference: reference.into(),
            wave: wave.into(),
        }
    }

    /// Parse a `kind:reference` authoring token, stamping `wave` as the owning
    /// wave of the referenced record (the claim's wave at authoring time).
    ///
    /// The reference is opaque per kind, so only the first `:` splits — a
    /// `pr:owner/repo#N@sha` reference keeps its own colons intact.
    ///
    /// # Errors
    /// [`ReceiptParseError`] when the token has no `:`, an unknown kind, or an
    /// empty reference.
    pub fn parse(token: &str, wave: &str) -> Result<Self, ReceiptParseError> {
        let (kind, reference) = token
            .split_once(':')
            .ok_or_else(|| ReceiptParseError::Malformed(token.to_string()))?;
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(ReceiptParseError::EmptyReference(kind.to_string()));
        }
        Ok(Self::new(kind.parse::<EvidenceKind>()?, reference, wave))
    }

    /// The `kind:reference` token, the drill target for `lf receipt show`.
    pub fn token(&self) -> String {
        format!("{}:{}", self.kind.as_token(), self.reference)
    }
}

/// Extract the PR number from a `pr:` receipt reference (`owner/repo#N[@sha]`).
/// Returns `None` when the reference lacks a `#N` segment.
pub fn parse_pr_number(reference: &str) -> Option<u32> {
    let after_hash = reference.split_once('#')?.1;
    let number_str = after_hash.split('@').next()?;
    number_str.parse().ok()
}

impl fmt::Display for Receipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (wave {})", self.token(), self.wave)
    }
}

/// Why a `kind:reference` authoring token could not be parsed. A user error, not
/// a bug — surfaced to whoever typed the `--receipt` flag.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReceiptParseError {
    #[error("receipt '{0}' must be written as 'kind:reference' (e.g. chat_turn:turn-3)")]
    Malformed(String),
    #[error(
        "unknown receipt kind '{0}'; expected one of chat_turn, worker_report (run), trace, pm, pr"
    )]
    UnknownKind(String),
    #[error("receipt kind '{0}' has an empty reference")]
    EmptyReference(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_kind_with_the_claim_wave() {
        let cases = [
            ("chat_turn:turn-3", EvidenceKind::ChatTurn, "turn-3"),
            ("worker_report:run-9", EvidenceKind::WorkerReport, "run-9"),
            ("run:run-9", EvidenceKind::WorkerReport, "run-9"),
            ("trace:0c2f-uuid", EvidenceKind::Trace, "0c2f-uuid"),
            ("pm:issue-uuid", EvidenceKind::Pm, "issue-uuid"),
        ];
        for (token, kind, reference) in cases {
            let receipt = Receipt::parse(token, "product").expect("parse");
            assert_eq!(receipt.kind, kind);
            assert_eq!(receipt.reference, reference);
            assert_eq!(receipt.wave, "product");
        }
    }

    #[test]
    fn pr_reference_keeps_its_own_colons_and_sha() {
        // Only the first ':' splits, so a PR reference with '@sha' survives whole.
        let receipt = Receipt::parse("pr:loopflow/loopflow#912@abc123", "product").expect("parse");
        assert_eq!(receipt.kind, EvidenceKind::Pr);
        assert_eq!(receipt.reference, "loopflow/loopflow#912@abc123");
    }

    #[test]
    fn token_round_trips_through_parse() {
        let receipt = Receipt::new(EvidenceKind::ChatTurn, "turn-7", "product");
        let reparsed = Receipt::parse(&receipt.token(), "product").expect("parse");
        assert_eq!(receipt, reparsed);
    }

    #[test]
    fn rejects_missing_colon_unknown_kind_and_empty_reference() {
        assert!(matches!(
            Receipt::parse("turn-3", "product"),
            Err(ReceiptParseError::Malformed(_))
        ));
        assert!(matches!(
            Receipt::parse("bogus:x", "product"),
            Err(ReceiptParseError::UnknownKind(_))
        ));
        assert!(matches!(
            Receipt::parse("chat_turn:  ", "product"),
            Err(ReceiptParseError::EmptyReference(_))
        ));
    }

    #[test]
    fn parse_pr_number_extracts_from_owner_repo_hash_n() {
        assert_eq!(parse_pr_number("loopflow/loopflow#912"), Some(912));
        assert_eq!(parse_pr_number("loopflow/loopflow#912@abc1234"), Some(912));
        assert_eq!(parse_pr_number("no-number"), None);
        assert_eq!(parse_pr_number("owner/repo"), None);
        assert_eq!(parse_pr_number("owner/repo#abc"), None);
        assert_eq!(parse_pr_number("owner/repo#0"), Some(0));
    }
}
