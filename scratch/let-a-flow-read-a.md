# LOO-450 — Claude Flow answers

Jack Heart requested one Claude turn-recording path and native JSON schema delivery.
The engine/driver vocabulary strike is integrated at base be4a2b2af (#1520).
The former headless print path bypassed ClaudeHarness; it is removed.

## Implementation

Headless Claude now uses the existing harness, preserving native skill
selection, context, resume and attachment ownership. The Flow output schema travels
through the ordinary CLI launch, capture/replay and correction turns to Claude. The existing
native answer reader remains; no new capture format or migration.

## Delete — do not maintain

- `_run_agent_once`'s Claude-only stdin tempfile and print launch branch.
- Exclusive expectations that headless Claude receives a text stdin file.
Preserve terminal launches and native/translated skill arguments and declarations.

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
harness. No store migration or third answer reader is needed.

## Remaining acceptance

Gate owns wider affected launch/skill fixtures and capture/replay acceptance.
The configured Claude Task pursue demo and installed acceptance remain unproved.
Publication and landing are outside this implementation step.

Checks: `cargo test -p loopflow --test flow_tests claude_deciding_step_records_native_answer_and_advances`, `--test default_conversation_tests`, `--lib returning_to_the_same_lf_process_does_not_revive_its_previous_attachment`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` passed; gate owns wider launch/skill and replay checks.
