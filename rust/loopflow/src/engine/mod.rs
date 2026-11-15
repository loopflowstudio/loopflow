//! The operations both adapters reach. An operation is a function with a typed
//! input, a typed result and an [`Error`] that names the code clients branch
//! on. The CLI parses argv into the input and prints the result; the server
//! decodes a request into the same input and sends the result as `response`.
//! Nothing here reads argv, writes to stdout or knows a socket.
//!
//! The engine sits at the crate root beside its two adapters, `lf` and
//! `server`, because both depend on it and it depends on neither's framing.
//! `ops` keeps the domain functions; an operation composes them into the unit
//! a client names (`task.run`), one module per namespace.

pub mod account;
pub mod checkout;
pub mod config;
pub mod creation;
pub mod graph;
pub mod launch;
pub mod pr;
pub mod repo;
pub mod select;
pub mod session;
pub mod skill;
pub mod task;
pub mod wave;

use serde::{Deserialize, Serialize};

/// What an operation returns.
pub type Result<T> = std::result::Result<T, Error>;

/// The progress a served operation reports: status and warnings go to the
/// request's diagnostics, and nothing can confirm, so a step that needs a
/// person's yes is refused unless the input already says so (`force`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Diagnostics;

impl crate::ops::Progress for Diagnostics {
    fn status(&self, msg: &str) {
        crate::journal::request::diagnostic(format_args!("{msg}"));
    }

    fn error(&self, msg: &str) {
        crate::journal::request::diagnostic(format_args!("{msg}"));
    }

    fn confirm(&self, _msg: &str) -> bool {
        false
    }
}

/// What a client can branch on. Nothing ran for any code but `outcome_unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No protocol revision in common; `data` names the side to update.
    Incompatible,
    /// The message or request is malformed or its arguments are refused.
    Invalid,
    /// The named Task, Session, owner or subscription does not exist; `data` names it.
    NotFound,
    /// The request contradicts current state: a reused id or a stale revision.
    Conflict,
    /// A dependency is missing or failed: the Machine registry, a watch, a provider.
    Unavailable,
    /// The operation may have taken effect; inspect state before retrying.
    OutcomeUnknown,
    /// The server is at capacity or the client fell behind; retry shortly.
    Busy,
}

/// The wire name, so a message reads `outcome_unknown: …` on every client.
impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = serde_json::to_value(self).expect("an error code serializes as a string");
        f.write_str(name.as_str().expect("an error code serializes as a string"))
    }
}

/// A refused or failed operation. Shared functions return it inside
/// `anyhow::Error` to name their code; the server sends it as an `error`
/// message and the CLI prints its message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(deny_unknown_fields)]
#[error("{message}")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    /// Structured detail a client can act on, such as the missing resource.
    pub data: Option<serde_json::Value>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// The [`Error`] a shared function named anywhere in `error`'s chain, else
    /// `otherwise` with the error's text.
    pub fn refused(error: &anyhow::Error, otherwise: ErrorCode) -> Self {
        match error.chain().find_map(|cause| cause.downcast_ref::<Self>()) {
            Some(named) => named.clone(),
            None => Self::new(otherwise, error.to_string()),
        }
    }

    /// The code a failed write names, else the one its error's type implies.
    /// A write that failed for another reason may have taken partial effect.
    pub fn written(error: &anyhow::Error) -> Self {
        use crate::store::StoreError;
        use crate::work::selection::SelectionError;
        if let Some(named) = error.chain().find_map(|cause| cause.downcast_ref::<Self>()) {
            return named.clone();
        }
        let code = error.chain().find_map(|cause| {
            if let Some(store) = cause.downcast_ref::<StoreError>() {
                return match store {
                    StoreError::NotFound => Some(ErrorCode::NotFound),
                    StoreError::Stale(_) => Some(ErrorCode::Conflict),
                    StoreError::InvalidData(_) | StoreError::Serde(_) => Some(ErrorCode::Invalid),
                    _ => None,
                };
            }
            if cause.is::<SelectionError>() || cause.is::<serde_json::Error>() {
                return Some(ErrorCode::Invalid);
            }
            None
        });
        match code {
            Some(code) => Self::new(code, format!("{error:#}")),
            None => Self::outcome_unknown(error),
        }
    }

    pub fn outcome_unknown(error: &anyhow::Error) -> Self {
        Self::new(
            ErrorCode::OutcomeUnknown,
            format!("{error:#}; inspect the affected work before retrying"),
        )
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{checkout, graph, pr, repo, task};
    use crate::lf::{Cli, Commands, TaskCommand};

    #[test]
    fn every_child_spelling_parses() {
        let parse = |args: Vec<String>| {
            Cli::try_parse_from(std::iter::once("lf").chain(args.iter().map(String::as_str)))
                .and_then(Cli::checked)
                .unwrap_or_else(|error| panic!("{args:?}: {error}"))
        };
        for verb in ["ship", "hold", "claim", "unassign"] {
            let cli = parse(task::spell_task("LOO-1", verb));
            if verb == "ship" {
                assert!(matches!(
                    cli.command,
                    Some(Commands::Ship { json: true, .. })
                ));
            }
        }
        for populated in [false, true] {
            let optional = |value| populated.then_some(value);
            let cli = parse(task::spell_run(
                "LOO-1",
                optional("implement"),
                optional("code"),
                optional("codex"),
                populated,
            ));
            assert!(matches!(
                cli.command,
                Some(Commands::Task {
                    cmd: TaskCommand::Run { json: true, .. }
                })
            ));
            for noun in ["graph", "skill"] {
                let cli = parse(graph::spell(
                    noun,
                    "example",
                    optional("Keep it small"),
                    optional("codex"),
                ));
                assert!(cli.batch);
            }
            parse(repo::spell_release(optional("minor"), optional("lf")));
            parse(pr::spell_land(&pr::LandInput {
                strict: populated,
                local: populated,
                wait_and_fix: populated,
                worktree: optional("checkout").map(str::to_owned),
                message: optional("Commit message").map(str::to_owned),
                title: optional("PR title").map(str::to_owned),
                body: optional("PR body").map(str::to_owned),
                agent: optional("codex").map(str::to_owned),
            }));
            parse(checkout::spell_sync(&checkout::SyncInput {
                onto: optional("main"),
                manual: populated,
                preview: false,
                continue_sync: false,
                abort: false,
                adopt: false,
            }));
        }
    }
}
