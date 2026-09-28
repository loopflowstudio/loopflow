# Indexed discovery: remaining owner conversion

2026-09-28 · LOO-298 · Dirty source at HEAD `2958d289c0e4087118e6920390fa6c8975c216d8`;
main is editing concurrently, so citations identify inspected bytes, not a frozen revision.
Read AGENTS.md, TESTING.md and the working design. Only this report was written; no execution proof or Git mutations ran.
Paths below are relative to `rust/loopflow/src/` unless prefixed `swift/`.

## Current facts

- **Exec storage exists; discovery does not.** `store/sqlite.rs:2043` inserts one
  `execs` row from journal events and settles command outcome transactionally.
  `store/migrations/drafts/record_execs__8e1f10371bfa03133ea590b179afc4cf.sql:5`
  stores parent, caller Session, command, repo/cwd and times, but no Task context. Its
  indexes at line 21 cover parent/time, trace/time and recent time, plus the
  identity primary key; none covers repo, command or caller Session. `store/sqlite/execs.rs:41`
  exposes driver/connection operations, not an Exec inventory or detail reader.
- **Command coverage is incomplete.** `bin/lf.rs:1266` now wraps ordinary parsed
  dispatch, including reads. Parsing/help (`:1169`), screenshots (`:1199`) and
  installation (`:1208`) precede it. `journal/mod.rs:373` still records best-effort.
  Command is serialized argv (`:391`), not an indexed canonical verb. These are
  admission gaps a new list API cannot repair; no universal command-history claim yet.
- **AgentSession inventory already filters before enrichment.**
  `store/sqlite/sessions.rs:81` applies repo, mode, history, Task, search and
  LIMIT/OFFSET before `ops/human_session.rs:658` calls `surface`. However search
  is `instr(lower(title),lower(?)) OR instr(id,?)` (`sessions.rs:113`); ordinary
  title B-trees cannot seek that substring predicate. There is no skill filter.
  `conversation_mode__d2b08e73a45147e3b99c078dd60496a2.sql:14` (under the same
  drafts directory) indexes mode/completion/title/id, with repo-leading variant
  at line 19; history/all-mode queries omit leading constraints. Verify their plans.
  The SELECT still joins current `runs` (`sessions.rs:12`); Task/skill are Run fields.
  `surface` still reads provider files and full managed review state (`human_session.rs:1485`).
- **History has a usable bounded reader.** `store/sqlite/session_events.rs:98`
  pages before payload decode; `record_session_events__b3d8bbde097141a596c4562954131457.sql:22` indexes Session
  and native-turn history, not Task/Exec attribution. Keep this subordinate history;
  attribution comes from the matching start receipt, not a new product object.
- **FlowSession has no summary inventory.** `store/flows.rs:29` exposes exact
  Flow and managed-Task lookup; `store/sqlite/flows.rs:29` selects/deserializes the
  complete capture and still reads current Run outcome. The retained
  `retain_flow_invocations__b85b73f238426ffa86687ee09c801a0d.sql:29` indexes
  Task/updated-time/id. `lf/commands/flow.rs:51` lists definitions, not saved sessions.
- **The old history path is still unbounded before payload I/O.**
  `store/sqlite/runs.rs:359` uses nullable-OR filters, start-or-end window and no LIMIT.
  `lf/commands/runs.rs:63` caps only after `:99` hydrates all selected records.
  `run_record.rs:510` drops missing artifacts/fails on corrupt ones; `:585` scans directories for prefixes. Delete both readers.

## Minimum proposed changes, in dependency order

1. Finish admission metadata on the existing owners, then add read-only typed
   Exec/AgentSession/FlowSession summary and exact-detail APIs. Resolve public Task
   identifiers once against retained identities, including done Tasks. Record nullable
   **command context** on Exec separately from **work performed**: one Exec can drive
   several Sessions/Flows. Work-history Task filtering joins immutable attributed
   Session/Flow events, deduplicates Exec IDs, then pages; never join today's Session
   owner to rewrite earlier usage. Direct observational Task context must not set Started.
   Reuse the command writer for safe early dispatch coverage; retain explicit unavailable
   evidence where bootstrap/store failure prevents recording, without a second journal owner.
2. Add parameterized equality/range predicates and bounded pages: Exec by repo,
   Task context/work evidence, parent, identity and canonical command; AgentSession
   by repo/Task/mode/history/title/skill; FlowSession by identity/repo/Task/state.
   Reuse parent/identity indexes; add repo/time/id, command/time/id and typed Task
   context/time/id access paths, plus Task/Exec indexes on the existing history joins.
   Index Session Task and captured skill/history metadata after removing Run. Normalize command
   at dispatch; do not search argv JSON or include prompt text as the command key.
   Use explicit exact/prefix title/skill search with matching collation/range indexes;
   current contains-search is a separate scan contract, not silently equivalent behavior.
   Exact/prefix identity uses PK ranges and LIMIT 2 for ambiguity; no transcript FTS.
3. Return only summary columns; omit request/feedback bodies, graph JSON and transcripts.
   Index Flow name as an expression on the existing capture. Keep payload reads on detail. Missing payloads
   must retain visible row identity and explicit availability, not disappear. Preserve
   activity's end-in-window semantics separately from recent-start history; aggregate
   usage from retained receipt evidence without rescanning every Run transcript.
4. Replace OFFSET with bounded cursors over immutable keys (Exec start/id, Session
   creation/id, Flow identity); return next cursor and page bounds. Rename must not
   move the paging key. Reset filtered refreshes after membership changes; do not claim
   snapshot isolation across separate commands. Add only indexes justified by final SQL
   plans, including history/all-mode variants and Flow repo discovery after ancestry moves.
## Consumers to migrate together

`lf/commands/runs.rs:19,63,197,236`, `usage.rs:27`, `activity.rs:146`,
`waves.rs:528`, and `replay.rs:19`; Session prefix/actions at
`ops/human_session.rs:666,839,1431`; live inventory's manifest discovery at
`run_record/active/reader.rs:606` and Exec-tree reconstruction at `lf/commands/top.rs:596`.
Keep exact OS receipts for control, not inventory identity. Migrate Rust DTO fixtures
with `swift/Loopflow/Services/RegistryQuery.swift:149,184,402`, Session/Run models,
`swift/LoopflowMac/Views/TaskRunsView.swift:16` and
`swift/LoopflowMac/Services/RegistryQueryLocal.swift:14`. Desktop currently requests
`session list --limit 0` to avoid offset races: replace that caller with the bounded
page/refresh contract, retaining panes by Session ID. Do not merely rename Run DTOs to Exec.
## One decisive fixture, owned by main

Use one disposable materialized store: 100k Execs, 20k Sessions, 5k Flows, three repos,
1k Tasks, tied timestamps/titles, nested direct/agent parents, multi-Task Exec work,
pre/post-bind usage, done Tasks and large/missing payloads. Assert exact filter IDs,
distinct command-vs-work attribution, no duplicates/skips on a fixed dataset, rename-safe
page keys, ambiguous prefixes, default interactive scope and retained unknown evidence.
Assert zero transcript/graph reads for metadata lists, bounded enrichment and selected-only
detail payload reads. Capture EXPLAIN plans and first-process/warm median/p95 CLI list/detail
timings with scale, host load and source hashes; separate startup/schema, SQL and enrichment.
No result is claimed here.
