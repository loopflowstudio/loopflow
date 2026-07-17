# Session runner turn-usage review

## What was implemented

Task Sessions now construct trace capture around their in-process harness body,
record every `ConversationEvent`, and finish capture on terminal paths. A
runner-driving regression test proves emitted turn usage reaches `agent_turns`.

## Key choices

- Reused the harness-construction function value already exercised by Wave
  bodies; no new factory trait or capture abstraction was added.
- Kept capture provider/model metadata sourced from `PreparedHarnessTurn`.
- Drove the private runner in-crate with a real temporary git worktree, active
  PR, directive, lease, and journal context.
- Sabotaged only `capture.record_conversation(event.clone())`; the test failed
  at the persisted token assertion, then returned green after restoration.

## How it fits together

`run_task_session_inner` receives provider events, forwards each event to its
`CaptureHandle`, and closes that handle when the body settles. `TraceCapture`
persists the resulting usage on the launch's `agent_turns` row.

## Risks and bottlenecks

Capture remains best-effort so telemetry cannot take down a Task body. The test
uses the existing prepared-turn metadata contract and deliberately does not
make the Session's configured agent authoritative for trace metadata.

## What's not included

- Project Session runner coverage
- Capture metadata changes
- Parser unification or reader/store cutover
- Any Swift or UI changes
