# LOO-924 follow-through: one owner for "managed" and for the agent choice

Direction from concept review of PR #1966 (`scratch/concept-review.html`).
Jack, 2026-11-17, quoting the review: "Two product ideas still lack one owner in
the code: whether a launch is managed, and which agent was chosen." / "this
seems worth fixing". Both go in this PR.

## 1. Holder decided once, in `transport::delegate`

Today `delegate` reads Work through `works()` to choose `Place`, and
`transport::holder()` is called separately at four sites with different inputs:
`lf/commands/run.rs` lines 59, 196, 252 and `harness/native_session.rs:177`.
The brief says mode and holder are "resolved once, at `transport::delegate`".

- `Delegation::Here { mode, holder }`. `delegate` reads Work once: named Work,
  the reopened Session's `is_work`, or an inherited declaration (`LF_AS`,
  `WorkSource::Declared`). From that one reading it derives `holder` and
  `place`.
- Keep today's behavior exactly: an inherited declaration makes the holder
  `Server` but does not delegate (`place` stays `None`). This is pinned by
  `transport::tests::inherited_declaration_keeps_server_reporting_without_naming_work`.
  The run stays where it is and its turns are recorded by the server. Checkout
  binding stays `Caller`.
- A store that can't be read counts as Work for both answers, as `works()`
  already does. The resume path stops failing the reopen with `?` at
  `native_session.rs:177`.
- Delete `transport::holder` and its four call sites. Callers take
  `holder` from `Delegation::Here`. In-process callers that never go through
  `delegate` (commit message, PR text, replay) keep their fixed Headless +
  Caller.
- If implicit checkout binding in `run.rs` has to run before `delegate` to
  supply the source, move that read earlier. It is cwd plus `LF_AS`.

## 2. The agent choice parsed once

Today `AgentConfig.agent: Option<String>` (`"claude:opus"`) is re-parsed by
`parse_agent` at 44 production sites and `HarnessKind::parse` at 36, with
`claude_model` and `codex_service_tier` sitting on the shared config.
`agent/mod.rs:1394` can still fail at headless launch with "unsupported
session harness".

- Add `Agent { harness: HarnessKind, model: Option<String> }`, parsed once
  where config resolves. An unknown harness becomes a config/selection error.
- `AgentConfig` carries the resolved `Agent`, and `Launch.harness` comes from
  it. Delete the re-parses. `HarnessKind::parse` remains only at input
  boundaries: CLI `-a`, config files, stored Session provider rows.
- Fold `claude_model` into `Agent.model` where it means the same thing (the
  `resolved_agent` special case). If a per-harness option still has to live on
  the config, keep it, and say in the PR why.
- Wire shapes (`--json`, stored rows) keep their string form. Parse at the
  boundary; don't change DTOs.

## Checks

- `cargo test -p loopflow-execution --lib transport::` and
  `harness::launch_matrix` (16-cell matrix unchanged).
- `cargo test -p loopflow-execution --lib config::` for agent selection and
  fallback order.
- `cargo clippy --all-targets -- -D warnings`.
