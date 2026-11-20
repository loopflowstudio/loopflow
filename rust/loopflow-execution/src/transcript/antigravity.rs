//! Antigravity's `transcript_full.jsonl`. Not read yet: a turn stays as its
//! driver recorded it, with no usage.

use serde_json::Value;

use super::Turn;

pub fn read(_rows: &[Value]) -> Vec<Turn> {
    Vec::new()
}
