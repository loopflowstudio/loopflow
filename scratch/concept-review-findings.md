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

### Implementation status · captured-event checkpoint

Main implemented the event decision: reservation selects a captured sequence;
publication/readiness/completion compare it; Rust/Swift Session wire no longer
contains run_id; RunId and the runtime input catalog are deleted. Original SQL
and unresolved selectors remain immutable import evidence. Source/canonical and
consumer proofs are in [evidence](evidence.md). The general history reader is published at `142314682`. Jack's acceptance remains outstanding.

## 2. Recorder types still say Run (2026-09-29)

Jack Heart read that `RunManifest` and `RunLaunchRequest` remain and said they
"should probably be [Agent]SessionManifest and SessionLaunchRequest or
something." Direction: the remaining Run-named recorder types take Session
names. Exact spellings are not fixed by Jack.

Counts at `142314682`, Rust plus tests plus Swift: `RunFlowMembership` 57,
`RunManifest` 30, `RunLaunchRequest` 21, `RunFlowStep` 19, `RunWork` 16,
`RunCapture` 9, `RunContextRef` 4, `RunRecorder` 4, plus `RunAttribution`.

Constraints:
- A manifest is written once per captured input, not once per Session. One
  Session can hold several. The name should not read as one per conversation.
- `manifest.json` is an on-disk format with `schema_version`, and its fields
  `run_id`, `parent_run_id` and `subjects` are read from old directories by
  import and replay. Renaming a Rust type is free. Renaming serialized fields
  needs old files to keep decoding.
- `RunEventRow` and `run_events` are the command journal, keyed by trace. They
  stay out of this rename.

Unresolved until main implements it and Jack accepts the result.

### Vocabulary direction (2026-09-29)

Jack followed his launch-is-an-Exec decision with: "we should also try to use
the word exec instead of launch (or run etc) where possible." So the renames in
this finding prefer Exec over both Run and launch. Session-prefixed names remain
right for things that belong to the conversation and not to one process. A
record of imported history with no process must not be named as an Exec.

### Implementation status · naming cut · 2026-09-30

The [naming table](naming.md) records the implemented replacements.
`SessionCaptureManifest` names one capture, while `AgentExecRequest` names its
provider inputs. Session recorder, context and membership names no longer say
Run. Original manifest keys and old directory selectors still decode. The
journal's trace types and history-table names remain unchanged. Focused results
are in [evidence](evidence.md); Jack Heart's acceptance remains outstanding.
