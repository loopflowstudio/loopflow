# LOO-442: provider conversation identity

Jack Heart requested AgentSession on 2026-10-09, stacked on LOO-441. The Task
brief is accepted scope; this implementation selects the permitted typed-id path.

AgentSessionId is the provider-issued opaque conversation id, not a UUID generated
by Loopflow. No table: current connections belong to the LfSession row, native references and historical
selections to immutable turn/capture events, account routing to existing mappings.
A provider announcing a replacement changes the current selection; earlier turn
keys and captured references remain in history. No new replacement event is needed.
Engine/driver columns and Process ownership belong exclusively to LOO-443.

## Delete — do not maintain

- Rust `provider_thread` / `provider_session_id` fields, parameters and accessors;
  replace with `agent_session` and AgentSessionId end to end, including resume,
  captured evidence, turn keys and account routing. No string alias or parallel owner.
- Swift `providerThread: String?`; replace with typed `agentSession`.

## Deliberate persistence boundary (PR notes)

- Keep SQL `provider_thread`: existing Session selection and event keys retain
  their bytes and indexes without a naming-only migration.
- Keep SQL `provider_session_id`: existing provider-account routing keys stay intact.
- Keep JSON `provider_thread` and `provider_session_id`: captured native evidence
  and the Rust/Swift DTO retain one existing encoding, not dual-read aliases.
- Released migration SQL and historical schema fixtures remain immutable.

## Remaining

The typed cut is implemented across harnesses, resume configuration, account
routing, captured evidence and native turn keys. Swift mirrors both SessionEvent
and native history references without changing their wire strings. No engine or
driver column changed. The residual-name audit matches only retained SQL/JSON.

Gate still owns the full Rust suites, Rust/Swift DTO fixture execution, account
attribution checks and rerunning the repaired fresh-process resume fixture.
The first fixture emitted only Claude's final result; the reader correctly did
not treat that as streamed answer text. It now emits a text delta before the
result. That repaired fixture has not run: disk fell below the 32 GiB reserve
during builds, and supported recovery retained active/recent builds; uv cache
pruning refused its busy lock. No installed store or other active work was changed.
Publication, parent-first landing and Task completion remain outside implementation.

Review found that typing Swift's native history reference changed string
interpolation of its identity; the projection now explicitly uses rawValue to
preserve existing row ids. Current connection evidence and per-input native
references remain distinct facts, not a second conversation table.
Release child memory retains the same source-versus-installed evidence boundary;
no release or schedule direction changed.

Checks: cargo fmt / cargo clippy --all-targets -- -D warnings and swift build --package-path swift pass; network-isolated lib agent_session filter: 3 pass, resume fixture failure repaired but rerun deferred below disk reserve; full Rust, attribution and Rust/Swift DTO tests remain with gate/CI.
