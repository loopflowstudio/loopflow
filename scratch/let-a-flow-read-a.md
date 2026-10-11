# LOO-450 — Claude Flow answers

Jack Heart requested one Claude turn-recording path and native JSON schema delivery
on October 9, 2026; October 10’s deletion steer extends that cut.
The engine/driver vocabulary strike is integrated at base be4a2b2af (#1520).
The former headless print path bypassed ClaudeHarness; it is removed.

## Delete — do not maintain

Jack Heart’s October 10 steer removes `run_batch`, `run_streaming`, their
exclusive stream parsers/renderers, obsolete Codex/OpenCode batch command branches
and their fixtures, the unknown-provider Claude
fallback, and `session_record::final_answer` / `FinalAnswer.exact`. Preserve
terminal launches, replay/run display types, native correlated answer history,
captures and all three headless harnesses. Removed the exclusive capture stream
writer too; usage fixtures now exercise harness conversation checkpoints. Shared
`StreamFormat` stays for run/replay; captured event schemas remain readable.
No migration or capture rewrite.

## Implementation

Headless Claude now uses the existing harness, preserving native skill
selection, context, resume and attachment ownership. The Flow output schema travels
through the ordinary CLI launch, capture/replay and correction turns to Claude. The existing
native answer reader is the only answer source. `FinalAnswer` collapses to a
String; missing native output returns absence, even when capture prose exists.
Unknown providers fail in both terminal and headless launch modes.

## Review findings

Clap's inferred JSON Value parser treated a schema as a JSON string; the launch
fixture exposed it. Explicit JSON parsing preserves the object. Claude publishes
its native Session ID after start; the result now captures that final identity.
The correction fixture also exposed
that Claude kept its native identity only in memory/capture, not the Session's
selected thread. Selection now records under the current attachment when Claude
announces it; stale attachments cannot replace it. This is required to resume the
same deciding conversation, not a new continuation policy.
Native skill commands retain declarations and non-query context on the surviving
harness. Non-query context and the correlated query share one write/error path;
a failed context write now tears down the process too. No store migration or
third answer reader is needed.
Deletion review found capture usage fixtures still calling the retired writer;
they now use conversation checkpoints and retain cumulative/retry/finality proofs.
Historical capture schemas and payloads remain intact; only answer inference is removed.

## Remaining acceptance

- Gate: affected launch/skill suites, including `context_launch_tests`, and
  capture/replay with a non-null schema. Replay forwards the saved schema in code;
  its existing fixture uses `None`, so it does not prove schema preservation.
- Demo: the configured `lf -a claude task run <task> pursue` reaches `pr-publish`
  without a hand-run step. The fake-provider test runs a taskless authored Flow
  ending in `publish-proof`, not the Task workflow or a real publication.
- Installed acceptance remains unproved; no installed store or live Session was
  changed by these source fixtures. Publication and landing belong to the caller.

The focused Flow fixture covers a valid first answer and an invalid first answer
corrected on native resume, with `started`, `output` and `completed` counts for
both. Terminal behavior and native/translated skill declarations remain required;
removing the print branch does not waive them. No new product decision is needed.
Release's entry-point lesson applies: Flow success is not Task-to-publication proof.

Checks: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass; network-isolated `cargo test -p loopflow` focused unknown-provider and Claude deciding-Flow tests pass, as do six native-answer/capture/usage/telemetry cases; 12 terminal-builder/capture checks pass. Gate owns wider launch/skill and schema replay checks; configured demo remains open.
