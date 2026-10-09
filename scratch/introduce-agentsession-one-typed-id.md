# LOO-442: provider conversation identity

Jack Heart requested AgentSession on 2026-10-09, stacked on LOO-441 / PR #1516.
The Task brief is accepted scope; the implementation selects its permitted
opaque typed-id path, without a table or migration. Jack's parallel-work steer
leaves engine/driver columns and Process ownership exclusively with LOO-443.

## Implemented boundary

`AgentSessionId` preserves provider-issued strings, including non-UUID ids.
Rust resume configuration, harnesses, native turn keys, capture evidence and
account routing now carry it; Swift mirrors SessionEvent and native history
references. The former `provider_thread` / `provider_session_id` code fields
and accessors are gone, as are Codex's duplicate getter and OpenCode's
intermediate string parser. Provider protocol names remain vendor-owned.

The LfSession row holds the current connection; a provider announcing a new
conversation replaces that selection. Immutable turn/capture events retain
earlier native ids and recorded account attribution. Current selection and
per-input history are different facts, not reasons for another table or a new
replacement event. No engine/driver column changed.

## Deliberate persistence boundary (PR notes)

- Keep SQL `provider_thread`: existing Session selection and event keys retain
  their bytes and indexes without a naming-only migration.
- Keep SQL `provider_session_id`: existing provider-account routing keys stay intact.
- Keep JSON `provider_thread` and `provider_session_id`: captured native evidence
  and the Rust/Swift DTO retain one existing encoding, not dual-read aliases.
- Released migration SQL and historical schema fixtures remain immutable.

## Remaining acceptance

Gate/CI still own the Rust suites, current Clippy, account attribution,
Rust/Swift DTO fixtures and the repaired fresh-process resume fixture.
`agent_session_resumes_on_a_fresh_process` uses two stand-in Claude processes
and checks remembered content through the typed id; it is not a live-provider
proof. Its first version emitted only a final result, which the reader correctly
did not treat as streamed answer text. The text-delta repair remains unrun after
verification stopped below the repository's disk reserve.

The affected account checks include `session_resume_is_pinned_by_account_only`
and `usage_follows_the_switch_log_only_where_a_running_agent_does`. Existing
native-identity ordering and replacement-history checks remain relevant; a new
current selection must not rewrite old usage. Swift's prior build remains
applicable, but compilation alone does not establish DTO round trips.

Review found that typing Swift's native history reference changed interpolated
row ids; the projection explicitly uses `rawValue` to preserve them. The code
and docs agree on this boundary. The 2026-10-09 realign inspection found no
further implementation mismatch. Release's child goal and complete memory were
read; their distinction between source proof and installed acceptance remains
applicable, with no new release or schedule direction.

Publication, parent-first landing and Task completion are separate delivery
work. No installed acceptance is claimed; no new product decision is needed.

Checks: `git diff --check` and the `rg 'provider_session_id|provider_thread' rust/loopflow/src` audit pass (only retained SQL/JSON); prior `cargo fmt --check` and Swift build apply; `33317fcd3` records Clippy and 3 passing agent_session tests, while current Clippy, repaired resume, Rust/account and Rust/Swift DTO suites remain with gate/CI after the disk-reserve stop.
