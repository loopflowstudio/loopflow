//! Ops tracing for parity testing.
//!
//! When LF_TRACE=1, ops commands emit JSON traces instead of executing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;

/// Check if tracing is enabled via LF_TRACE env var.
pub fn trace_enabled() -> bool {
    env::var("LF_TRACE")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// A single traced operation.
#[derive(Debug, Clone, Serialize)]
pub struct OpTrace {
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(flatten)]
    pub args: serde_json::Value,
}

impl OpTrace {
    pub fn new(op: &str) -> Self {
        Self {
            op: op.to_string(),
            result: None,
            args: serde_json::Value::Null,
        }
    }

    pub fn with_result(mut self, result: &str) -> Self {
        self.result = Some(result.to_string());
        self
    }

    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.args = args;
        self
    }
}

/// Collects operation traces during ops command execution.
#[derive(Debug, Clone, Serialize)]
pub struct Tracer {
    pub command: String,
    pub options: serde_json::Value,
    pub operations: Vec<OpTrace>,
}

impl Tracer {
    pub fn new(command: &str, options: serde_json::Value) -> Self {
        Self {
            command: command.to_string(),
            options,
            operations: Vec::new(),
        }
    }

    pub fn trace(&mut self, op: &str) {
        self.operations.push(OpTrace::new(op));
    }

    pub fn trace_result(&mut self, op: &str, result: &str) {
        self.operations.push(OpTrace::new(op).with_result(result));
    }

    pub fn trace_args(&mut self, op: &str, args: serde_json::Value) {
        self.operations.push(OpTrace::new(op).with_args(args));
    }

    pub fn trace_full(&mut self, op: &str, result: Option<&str>, args: serde_json::Value) {
        let mut trace = OpTrace::new(op).with_args(args);
        if let Some(r) = result {
            trace = trace.with_result(r);
        }
        self.operations.push(trace);
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("tracer serialization")
    }
}

/// Hash a prompt for trace comparison.
pub fn hash_prompt(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8])
}

/// Mock responses for trace mode.
pub struct MockResponses;

impl MockResponses {
    pub fn git_status() -> &'static str {
        "dirty"
    }

    pub fn git_diff_cached() -> &'static str {
        "has_changes"
    }

    pub fn git_has_upstream() -> bool {
        true
    }

    pub fn gh_pr_exists() -> bool {
        false
    }
}
