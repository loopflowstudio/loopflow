# Concept review findings

LOO-298 · Findings Jack Heart raised while reading the diff. Each is unresolved
until main implements it and Jack accepts the result.

## 1. `RunId` still names the captured input (2026-09-29)

Jack read `AgentSession.input_id: RunId` and said the naming should keep being
polished. The accepted model has no Run object; the identifier that remains is
an immutable captured-input reference with no outcome or lifecycle.

Counts at `1fe26d25e`, Rust `src` plus Swift:

| Layer | Evidence |
| --- | --- |
| Rust type | `RunId`: 231 occurrences in 33 files; minted by `durable_id!(RunId, "run_")` at `durable.rs:68` |
| Field names, same concept | `input_id` 350 · `current_run_id` 45 · `caller_input_id` 39 · `session_run_id` 38 · `parent_run_id` 38 · `expected_run` 32 · `caller_run_id` 5 |
| One signature, two names | `bind_session(id, expected_run: &RunId, …)` compares against `session.input_id` |
| SQL columns | `flow_sessions.current_run_id` beside `agent_sessions.input_id` and `agent_session_inputs.input_id` |
| Wire | `SessionRecord.run_id = session.input_id`; Swift requires it and no production Swift code reads it |
| Third meaning | `journal/mod.rs` and `run_events.run_id` hold a `TraceId` |

`run_id` therefore means a captured input, a trace, or a legacy selector
depending on the file.

Constraints any rename keeps:

- Stored `run_…` identifiers stay valid selectors. Interactive Session IDs are
  themselves `run_…` strings.
- `run_events` is the command journal. Renaming its `run_id` column is a
  separate trace-naming change.
- `RunSnapshot`, `RunManifest`, `RunRecorder` and the other file-recorder types
  belong to the larger Run removal in `remaining-work.md` §1, not to this finding.

Unresolved, Jack's to choose:

- The replacement name for the type and fields.
- Whether newly minted identifiers keep the `run_` prefix.
- Whether `SessionRecord.run_id` is renamed or removed from the wire.

### Resolution direction (2026-09-29)

Jack decided the identifier names no object. A captured input becomes an event
in AgentSession history, referenced by its `session_events` sequence. `RunId`
and `agent_session_inputs` are deleted; `flow_sessions.current_run_id` becomes a
Session event reference; `SessionRecord.run_id` leaves the wire. Stored `run_…`
selectors keep resolving. `run_events.run_id` remains a separate trace-naming
change. This finding stays unresolved until main implements it and Jack accepts
the result.
