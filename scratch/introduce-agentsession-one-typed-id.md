# LOO-442: provider conversation identity

Jack Heart requested AgentSession on 2026-10-09, stacked on LOO-441 / PR #1516.
The Task brief is accepted scope; the implementation selects its permitted
opaque typed-id path, without a table or migration. Jack's parallel-work steer
leaves engine/driver columns and Process ownership exclusively with LOO-443.
Jack's October 9 comment `70bad83a-1001-4247-ade5-430e0fddf4bc` requests
the deferred resume, account, Clippy and DTO checks, then publication stacked
on #1516; merging is explicitly excluded.

## Delete — do not maintain

- Bare-string `provider_thread` / `provider_session_id` code fields, parameters
  and accessors: replaced end to end by `agent_session: AgentSessionId`.
- Codex's duplicate getter and OpenCode's intermediate string parser: removed;
  retain opaque provider bytes, resume behavior and account attribution.
- Keep the SQL/JSON encodings and historical fixtures below, not parallel
  compatibility paths. Engine/driver columns remain outside this Task.

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

## Verification and remaining delivery

Jack's requested publication checks now pass. The nine focused Rust checks cover
fresh-process resume, opaque bytes, Codex identity discovery, durable identity
before spawn, account pinning/switch attribution, original identity ordering and
usage preservation across replacement. `agent_session_resumes_on_a_fresh_process`
uses two stand-in Claude processes, not a live provider. Its earlier final-only
fixture omitted streamed answer text; the repaired text-delta version now passes.
Rust and Swift DTO fixtures pass with unchanged wire keys.

The prior publication preflight stopped at 30.1 GiB free against the 32 GiB
reserve, reclaiming nothing. The October 9 retry passes at 35.8 GiB; recovery
retains active builds and reports the busy uv cache rather than forcing cleanup.

Review retains the Swift native-history `rawValue` projection so row ids do not
change when their type changes. No engine/driver columns or migrations changed.
Release's child guidance still separates source checks, publication and installed
acceptance; no release, installation or schedule change is authorized here.

Full materialized Rust and broader Swift suites remain with gate/CI. Publication
is authorized after these focused checks, stacked on #1516; no merge or Task
completion. No installed or live-provider acceptance is claimed.

Checks: `cargo fmt --check`, `cargo clippy --all-targets -j 4 -- -D warnings`, `cargo test -p loopflow --lib --no-run -j 4`, `cargo test -p loopflow --test dto_fixtures --no-run -j 4`, network-isolated lib filters (9) and DTO binary (22), `scripts/test_desktop.sh --jobs 2 -Xswiftc -gnone --filter DTOFixtureTests` (build + 23 tests), `git diff --check` and the retained SQL/JSON name audit pass; full matrix remains gate/CI-owned.
