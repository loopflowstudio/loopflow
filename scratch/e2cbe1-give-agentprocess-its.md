# e2cbe1: Give AgentProcess its own ID type

Status: implemented on main (2026-11-06) except the #1813-dependent schema
regeneration; see Remaining.

## Problem

An AgentProcess (the provider OS process, a `processes` row of kind `agent`)
and an LfProcess (one `lf` process) share `LfProcessId`. Field names alone keep
them apart: `SessionRecord.agent_process_id`, `AgentCaller.agent_process_id`,
`SessionAttachment.agent_process_id`, `Steer.agent_process_id`,
`LfProcess.caller_agent_process_id` are all `LfProcessId`. The compiler can't
catch a swap, and wire readers see one schema for both.

The gap is deeper than the fields. The inventory row `process::LfProcess` holds
both kinds: `id: LfProcessId` plus `kind: ProcessKind`, so every agent row's id
is typed as an LfProcess. The flat struct is the cross-product of both kinds'
columns (`via_agent`/`caller_*` are always null on agent rows,
`agent_session_id` always null on lf rows), and consumers branch at runtime on
`kind` (`journal::recorded_process_receipt`, `recorded_process_evidence`,
`top::collect_activity`, `top::command_label`, `rollover`, `task_execution`).
`docs/architecture/reference-models.md` already names this as unfinished: "The
common `LfProcess` wire projection still carries both lf and agent rows during
the AgentProcess cutover." This Task finishes that cutover.

## Demo

```bash
lf history list --json | jq '.entries[] | {kind, id}'   # both kinds, same ids; only always-null keys dropped
lf server describe --json | jq '."$defs" | keys[] | select(test("ProcessId"))'
# "AgentProcessId"
# "LfProcessId"
lf server describe --json | jq '."$defs".SessionRecord.properties.agent_process_id'
# references AgentProcessId, not LfProcessId
```

And the developer-facing win: passing `session.agent_process_id` where an
`&LfProcessId` is expected (e.g. `store.process(..)`, `attach_in(.., process)`)
is a compile error.

## Chosen approach

1. **`AgentProcessId` newtype.** `uuid_id!(AgentProcessId)` in `id.rs`, beside
   `LfProcessId`. Same canonical lowercase UUID string in JSON and SQLite; no
   migration. It joins `ids_round_trip_as_uuid_strings` and the canonical-ID
   table test.

2. **The inventory row becomes a tagged enum.** `process::ProcessEntry` replaces
   the dual-kind use of `LfProcess`:

   ```rust
   #[serde(tag = "kind", rename_all = "snake_case")]
   pub enum ProcessEntry { Lf(LfProcess), Agent(AgentProcess) }
   ```

   - `LfProcess` keeps its lf fields, drops `kind` and `agent_session_id`.
   - `AgentProcess` (public wire type) has `id: AgentProcessId`, `pid`,
     `os_started_at`, `agent_session_id`, `trace_id`, `parent_lf_process_id`,
     `command`, `repo`, `cwd`, `started_at`, `completed_at`, `outcome`,
     `exit_code`, `signal`, `error`. No `via_agent`/`caller_*`.
   - The `kind` tag carries the same `"lf"`/`"agent"` values; `ProcessKind` is
     deleted. JSON for each row changes only by dropping keys that are always
     null for that kind.
   - The current crate-private `AgentProcess { process: LfProcess, attached_.. }`
     becomes `AgentProcessState { process: AgentProcess, attached_lf_process_id,
     attachment_token, provider, interactive, spawn_reserved }`.
   - `read_process` dispatches on the `kind` column. `PROCESS_SELECT` is
     unchanged.
   - Readers that only ever meant one kind narrow their return type:
     `Store::process(&LfProcessId) -> Option<LfProcess>` filters `kind='lf'`;
     `agent_processes()` returns `AgentProcessState`. Mixed readers
     (`processes_since`, `unfinished_processes`, `resolve_process`, the history
     page, `TaskWork.processes`) return `ProcessEntry`.
   - Runtime `kind ==` branches become `match` arms: an agent row has no
     receipt, so `recorded_process_receipt` takes `&LfProcess`;
     `recorded_process_evidence` matches; `command_label` takes the entry.

3. **Every AgentProcess reference uses `AgentProcessId`.** Rust sites (from
   `grep -rnE "agent[a-z_]*: *(&|Option<&?)?(crate::id::)?LfProcessId"` plus the
   harness and store):
   - `process.rs`: `AgentCaller.agent_process_id`,
     `SessionAttachment.agent_process_id`, `LfProcess.caller_agent_process_id`.
     `SessionAttachment.provider_lf_process_id` and
     `AgentCaller.origin_lf_process_id` are LfProcesses and stay.
   - `session.rs` (both fields), `steer.rs`, `lf/commands/session.rs:292`,
     `ops/human_session.rs:299,1563`.
   - `store/sqlite/processes.rs`: `UnrecordedAncestry.agent_process_id`,
     `agent_process_progress_in`, `attach_in` (mints `AgentProcessId::new()`),
     `record_agent_process_{launch,exit,identity}`, `agent_endpoint`,
     `record_agent_endpoint`, `settle_agent_process`,
     `prepare/replace_session_agent_process`, `release_unrecorded_ancestry`.
   - `store/sqlite/{program_status,sessions,session_events,steers}.rs`.
   - `harness/agent_process.rs` (`AgentProcessCleanup.id` and the evidence
     closure), `harness/{claude,codex,codex_connection,opencode,dispatch,
     fact_writer}.rs`, `transport*`, `terminal_title.rs`, `os_process.rs`,
     `lf/commands/top.rs` (`ActivityNode.id` stays a string; the
     `AgentProcess` node kind already exists).
   - The `--agent-process` CLI argument for Program Status parses
     `AgentProcessId`.
   Work through `cargo check` errors after step 1–2; each error is a site.

4. **Cursor and filter over the mixed inventory.** `LfProcessCursor` /
   `LfProcessFilter` / `LfProcessPage` are renamed `ProcessCursor` /
   `ProcessFilter` / `ProcessPage` (they page `ProcessEntry`). The cursor's `id`
   is the `processes.id` column position, an opaque continuation, so it becomes
   `String`; its JSON stays `{started_at, id}`. `ProcessFilter.id` likewise
   matches the column as a string (it already does in SQL). `--parent` resolves
   the selector: an LfProcess filters `parent_lf_process_id`; an AgentProcess
   filters `caller_agent_process_id` (the lf processes that agent issued), via
   a new `caller_agent_process_id: Option<AgentProcessId>` filter field.

5. **Server schema and Swift.** With #1813 the `uuid_id!` macro derives
   `JsonSchema`, so `AgentProcessId` appears in `$defs` and every agent field
   references it; `ProcessEntry` appears as a `kind`-tagged `oneOf`.
   Regenerate `tests/fixtures/dto/server_schema.json`.
   Swift mirrors the split:
   - `AgentProcessId: RawRepresentable, Codable, Sendable, Hashable` beside
     `AgentSessionId` (`Models/SessionEvent.swift`), used by
     `SessionRecord.agentProcessId`, `SessionEvent.agentProcessId`,
     `LfProcess.callerAgentProcessId` and
     `LoopflowMac/Services/Ghostty/ProgramStatusObservation.swift`.
   - `enum ProcessEntry { case lf(LfProcess), agent(AgentProcess) }` decoded on
     `kind`; `TaskWork.processes: [ProcessEntry]`; `TaskWorkView` renders both
     arms; Swift `ProcessKind` deleted.
   Swift keeps `LfProcessId` fields as `String`, as today; typing those is not
   in this Task.

6. **Docs.** `reference-models.md`: LfProcess row names `LfProcessId`,
   AgentProcess row names `AgentProcessId`; delete the "still carries both lf
   and agent rows during the AgentProcess cutover" paragraph.
   `tests/fixtures/dto/README.md` if it lists `ProcessKind`.

## Risk findings

- **`lf server describe` exists only on #1813** (open, 457 files, branch
  `jack/loo-593-decide-the-desktop-servers-roles-and-long-term-architecture`,
  head `77ab719b1` on 2026-11-06). There `uuid_id!` derives
  `schemars::JsonSchema`, `$defs.LfProcessId` is `{"type":"string"}`, and
  `agent_process_id`/`caller_agent_process_id` both reference it. So the
  schema half of Done-when falls out of steps 1–3 once both are on main; no
  schema code is needed here. #1813 also touches `id.rs`, `process.rs`,
  `session_record.rs`, `server/*` and many Swift models: expect conflicts in
  whichever lands second. Implementation proceeds on main; the schema
  regeneration and Swift `ServerProtocolTests` run after rebasing onto a main
  that contains #1813. Recorded in `scratch/questions.md`.
- **No stored data changes.** Both IDs are lowercase UUID text in the same
  `processes.id` column; `kind` decides which type a row reads as. The
  `LF_AGENT_CALLER` environment JSON keeps `agent_process_id`/`origin_exec_id`
  as strings, so environments installed by a released `lf` still parse.
- **Wire change is key removal only.** Agent rows lose three always-null keys,
  lf rows lose one. Swift decodes those with `decodeIfPresent`, so a released
  Desktop reading a branch `lf` keeps working, and the new Swift enum reads
  released `lf` output (the dropped keys were null).
- **Mixed-kind SQL is unaffected.** `FORGETTABLE`, `task_work` membership
  unions and `relocated_task_processes.process_id` compare columns in SQL; they
  never construct a typed ID.

## Alternatives considered

- **Field-only newtype, keep the flat `LfProcess` row** and convert at the
  boundary (`AgentProcessId::from(row.id)`). Smallest diff, but every agent row
  still reads as an `LfProcessId`, which is the gap the Task names, and the
  runtime `kind` branches stay. Rejected.
- **`enum ProcessId { Lf(..), Agent(..) }`** carried on one flat row. Keeps the
  cross-product struct and adds a third ID type whose JSON can't round-trip
  (both arms are bare UUIDs). Rejected.
- **Keep wire byte-identical** by carrying the null cross-kind keys in both
  variants. Keeps meaningless fields to avoid a harmless key removal. Rejected.

## Delete — do not maintain

- `process::ProcessKind` (Rust) and `ProcessKind` (Swift `Models/LfProcess.swift`);
  the `kind` serde tag replaces them.
- `LfProcess.kind`, `LfProcess.agent_session_id` (Rust and Swift).
- The crate-private `AgentProcess { process: LfProcess, .. }` shape.
- `ProcessKind` arguments: `top::command_label(.., kind)`,
  `linear/request_log.rs:101` passes `ProcessKind::Lf`.
- The cutover paragraph in `docs/architecture/reference-models.md`.

Behavior that survives: activity tree shows agent nodes under their parent;
receipts are read only for lf rows; agent liveness uses recorded pid/start;
history paging order (`started_at DESC, id ASC`) and its cursor JSON.

## Forbidden outcomes

- Any `From<LfProcessId> for AgentProcessId` (or reverse) conversion. The only
  place one becomes the other is `read_process` choosing by `kind`.
- A flat row type with both kinds' fields surviving beside `ProcessEntry`.
- A schema `$defs.LfProcessId` still referenced by an agent field.
- A migration.

## Exclusions

- Typing Swift's `LfProcessId` fields; typing `ActivityNode.id`.
- Renaming SQL columns or the `origin_exec_id` environment key.
- Exposing `attached_lf_process_id` on the wire.

## Done when

- `rg -n 'agent[a-z_]*: *(&|Option<&?)?(crate::id::)?LfProcessId' rust/` and
  `rg -n 'LfProcessId::new\(\)' rust/loopflow/src/store/sqlite/processes.rs`
  find no AgentProcess site; `rg 'ProcessKind' rust swift` finds nothing.
- `lf history list --all --json` shows both kinds with `kind`, same IDs as before
  on an existing store.
- `lf server describe --json` (after #1813) lists `AgentProcessId` and
  `LfProcessId` as separate `$defs`; `SessionRecord.agent_process_id`,
  `SessionEvent.agent_process_id`, `LfProcess.caller_agent_process_id` and
  `AgentProcess.id` reference `AgentProcessId`.
- Swift decodes `process_page.json` into `.lf` and `.agent` entries.

## Checks

Focused (implement):
- `cargo test -p loopflow --lib id::` — new ID round-trips JSON and SQL.
- `cargo test -p loopflow --lib store::sqlite::processes::` — reads rows of
  both kinds into the right variant; agent attachment/resume tests unchanged.
- `cargo test -p loopflow --lib lf::commands::top::` — activity tree still has
  `AgentProcess` nodes.
- `cargo test -p loopflow --test integration dto_fixtures::` —
  `process_page.json` round-trips with the dropped null keys.

Gate (once, after rebase onto main containing #1813):
- `uv run python scripts/test.py --reuse-passing` — affected suites pass.
- `cargo test -p loopflow --lib server::schema::` — checked-in
  `server_schema.json` equals `describe`, every fixture validates.
- Swift `DTOFixtureTests` and `ServerProtocolTests` (headless `swift test`
  suites) — process page and schema `$defs` read.
- `cargo clippy --all-targets -- -D warnings`.

## Implementation notes (2026-11-06)

- Agent sites that passed an agent id to `store.process` / `journal::process_evidence`
  worked only because `process()` didn't filter `kind`. They now use
  `SqliteStore::agent_process` and `journal::agent_evidence`
  (`harness/{agent_process,native_exit,task_updates}.rs`, `transport.rs`,
  `ops/human_session.rs`). `store.process` reads `kind='lf'` only.
- `journal::recorded_process_evidence` takes `&ProcessEntry`; `lf_process_evidence`
  is the lf-only rule; `process_entry_evidence(store, entry)` reads a receipt for
  lf rows. `task_open_processes`, `execution_blockers` and `unresolved_processes`
  carry `ProcessEntry`, so agent liveness no longer goes through an lf lookup.
- `read_lf_process` / `read_agent_process` are strict: the other kind is a
  conversion error, not a silent skip. They serve Graph inventory, Task Graph
  runs, `agent_process` and `agent_processes`.
- `top::command_label(&ProcessEntry)` dispatches to `lf_command_label` /
  `agent_command_label`; `linear/request_log.rs` calls `lf_command_label`.
- Swift: `AgentProcessId` (canonicalizing decode) in `Models/SessionEvent.swift`;
  `ProcessEntry` / `AgentProcess` in `Models/LfProcess.swift`; `TaskWorkView`
  labels agent rows "Agent". Fixtures `process_page.json`, `task_work.json`,
  `task_lifecycle.json` drop always-null cross-kind keys.

## Remaining

- After #1813 lands: rebase, regenerate `tests/fixtures/dto/server_schema.json`,
  confirm `$defs.AgentProcessId` and the four references; Swift
  `ServerProtocolTests`.

## Check results

- compress: `cargo clippy -p loopflow --all-targets -- -D warnings` clean; `cargo test -p loopflow --lib -- store::sqlite::processes:: lf::commands::top:: ops::task_execution:: harness::agent_process:: lf::commands::doctor:: store::sqlite::retention::` — 50 pass; `swift build --target Loopflow` — builds. `TaskWorkView` (LoopflowMac): gate.

- `cargo test -p loopflow --lib -- id:: store::sqlite::processes:: lf::commands::top::` — pass (store module rerun 18/18 with the `caller_agent_process_id` filter assertion).
- `cargo test -p loopflow --lib -- harness::agent_process:: session_record::runtime:: ops::task_execution:: store::sqlite::task_work:: … lf::commands::util:: ops::task::tests` in one run — 105 passed, 8 `lf::commands::util` failed; `lf::commands::util::tests::` alone — 11/11 pass. Gate reruns the affected suites.
- `cargo test -p loopflow --test integration dto_fixtures::` — 48 pass.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — clean.
- `swift build --target Loopflow` — builds. LoopflowMac and Swift `DTOFixtureTests`: gate.
- Done-when greps: no `agent*: LfProcessId` field, no `ProcessKind` in rust/swift; remaining `LfProcessId::new()` in `processes.rs` are lf fixture rows.
