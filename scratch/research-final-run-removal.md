# Research: final Run removal

2026-09-29 · Bounded source audit for LOO-298; no executable edits, builds, tests, or service mutations.
Observed HEAD: `24ea61517cc5cbcfc4cf29452364fe6efcd37123`. Main concurrently owns dirty `store/migrations.rs`; its unfinished proof is not evaluated here. `scratch/import-preservation.md` and `parallel-work.md` were also dirty before this audit. Rust paths below are relative to `rust/loopflow/src/`. Hashes identify pivotal inspected bytes, not a whole-tree validation receipt.

## System understanding

Admission and Flow settlement already use AgentSession/FlowSession/Exec. `CaptureHandle::record_row` creates a Session; mechanical capture creates neither Run nor Session. Flow reservation selects a Session input, publication reads that Session, and completion joins exact native history. `require_turn_authority` joins caller Exec, selected history, provider generation and caller token, never `runs`.

The remaining Run surface means four different things: obsolete SQL CRUD, historical import evidence, immutable input references, and live file-backed lifecycle/discovery. Dropping the table is smaller than removing the Run product. Deleting every Run-named symbol would also delete required input and process evidence.

### Dependency table

| Caller / source anchor | Real owner | Exact blocker to deletion | Smallest proof, not run here |
| --- | --- | --- | --- |
| `Store::{run,create_run,runs}` → `store/sqlite/runs.rs:310`; `end_run:359` | Obsolete public CRUD | No production repository caller found outside wrappers; remaining calls are tests. `task_wave_in:294` still serves Session/Flow constructors. External public-API consumers remain unverified. | Remove CRUD/model/list export while retaining ancestry lookup; convert constructor tests to Session/Flow behavior and zero-row assertions to table absence. Compile callers; document exported API removal. |
| `ops/session_import.rs:232` → `historical_session_inputs` (`store/sqlite/sessions.rs:140`) | SQL history → Session observations | Unconditionally queries `runs`; inner join requires `agent_session_inputs.input_id=r.id`. Unmapped rows are excluded even if they retain Work/Flow/outcome facts. Absent table fails explicit import before filesystem inputs. | Populated prior-schema conversion with mapped/unmapped inputs, missing artifacts, conflicting and repeated replay; repeat import after table removal. Preserve unknown membership/process. |
| Started/history-deletion triggers (`retain_imported_session_evidence` lines 32–49) | Task Started; Session/Flow history | `validate_task_started_update` and `retain_task_agent_turn` still query `runs`. New Session/turn/operation writes execute them. `retain_task_first_run` also refuses deleting the last old Task row. | Post-drop admission, bind, mechanical start, history deletion and monotonic Started, including Started-only historical facts. Exercise writes, not only schema inspection. |
| Project/Task/Flow ancestry updates (`own_sessions_and_runs` lines 109–119) | Typed Session/Flow ancestry and immutable history | Surviving `validate_invocation_run_parents`, `validate_project_run_parents`, `validate_task_run_parents` query `runs`; no later drop found in draft sources. They belong to other tables. | Forward migration plus valid same-Wave Task transfer, invalid ancestry change and historical-owner preservation; materialized foreign-key and trigger execution. |
| `FLOW_SELECT`, `select_input_in`, review publication (`store/sqlite/flows.rs:31,827`; `sessions.rs:972`) | Selected immutable input, Session publication and exact completion | `current_run_id` is now a text input reference: admission drops its Run FK and validates against AgentSession. SQL still names the column and projects `FlowAttempt { run_id,published,outcome }`. Dropping it loses reservation/review fences. | Coordinated column/API conversion retaining saved reservation, replacement, stale review rejection and selected-success consumption. No Run row required. |
| `CaptureHandle` / `RunRecorder` (`run_record.rs:434,1942`) | Session admission/history; immutable payload | Still writes manifest/events/terminal files and RunBinding intervals. SQL observation errors warn and continue; other consumers retain file-based lifecycle truth. Renaming the recorder does not delete competing result readers. | Launch/retry/finish through SQL owners with retired mutable files unavailable; failed publication before provider effects; preserved immutable input and distinct provider/Exec results. |
| Lists, usage, telemetry, activity, landing conclusions (`lf/commands/runs.rs:85`; `store/sqlite/sessions.rs:207`) | Input references + Session observations | No `runs` table read. Per-input `RunSnapshot` wire remains. `conversation_snapshot` (`run_record.rs:547`) skips non-Observed native history; native-only recovery is outside that projection. `cap_runs` limits after enrichment. | Public usage after driver loss from native-only retained completion/usage with original attribution and missingness; separate dense bounded-query proof. Never restore old writes for telemetry. |
| `lf runs ID/--final/--resume`; `lf replay` (`lf/commands/runs.rs:199,238`; `replay.rs:18`) | Session/history; immutable launch input; native connection | `resolve_manifest` scans directory names; detail re-reduces terminal/events; resume reads provider-session sidecar. Replay needs payload but also takes identity/inherited subjects from manifest. SQL-listed rows can lack usable detail. | SQL summary/detail survives missing unrelated artifacts; ambiguous prefix remains explicit; exact payload/account replay creates fresh Session work without Flow-settlement authority. |
| Session lookup/open/review replacement (`ops/human_session.rs:634,705,1073,1307,1359,1432,1740`) | Session identity/driver/connection; Flow review fence | Exact identity is SQL, prefix fallback scans manifests. `NativeRun::history` requires provider-session files before even live Codex connect. Surface state reads sidecar/client facts; replacement reads file terminal outcome; prepare/resume/stop resolve manifests. | Same Session through open/connect/review retry after removing retired mutable sidecars, across supported providers; saved answer, exact stop ownership, no duplicate launch. Immutable payload availability remains separate. |
| Active/watch → `ActiveRunReader` (`run_record/active.rs:35,203`; `active/reader.rs:199`) | Exec/process evidence + Session attribution | Publishes `run-bindings/*.json`, joins interval to exact Exec/process receipts, then reads manifest before Session Work. Clients still use `runs/*/*/provider-clients`. Missing manifest skips observed process row. | Missing-manifest active view, retained Work, PID/start reuse rejection and handoff; retain unknown-owner gaps and shared-engine siblings. Causality never authorizes signals. |
| Journal/top/doctor → `RunEventRow` / `run_events` (`store/sqlite.rs:2111–2152`) | Existing command/trace journal alongside Exec | Separate event table, not product `runs`: process_id identifies Exec, run_id identifies trace. Live process/diagnostic callers remain. Vocabulary-based deletion loses required evidence. | Command completion/error and exact process lookup retain trace correlation through naming cleanup. |
| Rust `RunSnapshot`/`ActiveRun`; Swift `RegistryQuery.swift:435`, `Models/ActiveRunsSnapshot.swift` | CLI/Desktop wire projections | Per-input Run/subject vocabulary remains, without requiring SQL Run table. `RunId` also names immutable inputs/old selectors; `RunWork` means typed launch ancestry. | Coordinated Rust/Swift fixtures and history/ancestry UI proofs; preserve IDs/input lookup while replacing public Run concepts. |

## Tensions and observations

- **Nearly inert table, active schema:** obsolete CRUD has no observed production repository caller, but triggers on live tables and explicit import still require it.
- **Different list/detail truth:** SQL retains per-input attribution and missing payloads; detail/resume/active views still use files. This is a concrete conversion boundary.
- **Selected input remains necessary:** FlowAttempt packages existing-owner facts. Comments at `durable.rs:197` and `store/sqlite/flows.rs:1,519` still describe Run fallback authority absent from current FLOW_SELECT.
- **History projection is incomplete:** observed envelopes support current usage; native-only history is skipped. Generic recorder success cannot establish recovery after that recorder dies.
- **Old tests are not runtime writers:** `store/sqlite/durable.rs:704` starts a test module containing many Run mutations. `ops/run.rs:686` tests old listing CRUD; newer public CLI tests cover conversation history. Preserve behavior rather than obsolete-owner scaffolding.

## Concrete deletion opportunities

1. Delete `Run`, `RunEnd`, `ListedRun`, SQL CRUD and async wrappers together after test/export reconciliation. Move still-used `task_wave_in` to its current owner. This bounded reduction is independent of native retry repair.
2. Complete historical preservation before a forward table drop; remove/rewrite cross-table Run triggers and convert explicit import in that boundary. Keep applied drafts immutable. Assert table absence rather than only zero rows.
3. Move detail/resume/review/active consumers to existing owners, then delete replaced mutable file-state readers and RunBinding projection. Keep immutable launch input and exact native process evidence until their responsibilities have evidenced destinations.
4. Coordinate Flow column/FlowAttempt/DTO cleanup across consumers. None requires retaining `runs` or introducing an attempt table. Keep original-turn authority unchanged.

## Missing proof / open questions

No behavioral result is claimed here. Main's recorded scoped receipts retain their own limits. Decisive gaps: populated prior-schema and unmapped/member preservation; SQL/terminal disagreement; post-drop trigger execution; artifact-independent public detail/recovery; native-only recovered usage; coordinated wire removal. External public-API consumers were not inspected. No all-provider, installed or configured acceptance follows. The known valid-native-retry failure was neither investigated nor repaired.

## Pivotal inspected hashes (SHA-256)

```text
24af4ae8b157fab610ace61fef06afbd9af357cc9a4fb21db5fc74bf202311ac  store/sqlite/runs.rs
0afe725dfa423ee04145097a8f4e97ff77b4e74fd374ff5279ce5889981564ee  store/sqlite/sessions.rs
3a9890c92ed3c3b1dec9b7b1eaf1f681541e75388f70c5d5c04f81cbc123ebfb  store/sqlite/flows.rs
1162f23b3794b46ba66be4979f0a90e628a2fe0d82273d3661aaf0fa97493e3a  store/sessions.rs
676688e10581dfa85cabe164c57ad7fd595e4b7624567d8a158c38faacb7da42  run_record.rs
38c785bc1c01bd94ab51ffa8f36de08e930bbe9d4bcec3b70f223265324fab5b  run_record/active.rs
4bea152bc0d7f9715906884db65d8d0e1a10ccc427b1105e7fc102acf5524519  ops/human_session.rs
55cb7576d400b24e8f7d0e14c1647b878fe21895612b8e1b8fd46ad2635d9cf0  ops/session_import.rs
7c7554f981a23622be6d80078af9ad304a3d46014e572b77a9d7fd67b55f7443  lf/commands/runs.rs
cc59e59c6e1a407f46b19b84d40c09f2c8dd35af1ff5f29b17ec7b81685b7bb0  lf/commands/replay.rs
14aaac30bfa971fcb6b2e7667facd5e05b8f72496eb5543f6a5699b22e4b7a04  store/migrations/drafts/agent_session_admission__a89e13f5c5d2439da777324d31101db8.sql
b2dfb0f2d5a14c9fcfe1353afbb6f952ec2ad6ea2d3dc08b3983ddf65717dd14  store/migrations/drafts/retain_imported_session_evidence__4a7eb8b1d3e14f7c8e925db0134fd9b8.sql
```
