# LOO-450 — Claude Flow answers

Jack Heart requested one Claude turn-recording path and native JSON schema delivery
on October 9, 2026. Reconciled October 10 against `4c79cccf7`.
The engine/driver vocabulary strike is integrated at base be4a2b2af (#1520).
The former headless print path bypassed ClaudeHarness; it is removed.

## Implementation

Headless Claude now uses the existing harness, preserving native skill
selection, context, resume and attachment ownership. The Flow output schema travels
through the ordinary CLI launch, capture/replay and correction turns to Claude. The existing
native answer reader remains; no new capture format or migration.

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

Checks: network-isolated `cargo test -p loopflow` with `--test flow_tests claude_deciding_step_records_native_answer_and_advances`, `--test default_conversation_tests`, and `--lib claude_stream_context_keeps_large_instructions_and_reply_guidance_off_argv` passed; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed; attachment-rejection proof remains applicable; gate owns wider launch/skill and replay checks.
