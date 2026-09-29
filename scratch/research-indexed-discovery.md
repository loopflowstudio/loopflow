# Research: LOO-298 indexed discovery

## System understanding

2026-09-29. Research only; no source edits, builds, tests, branch executables, Home access, Git mutations or delegation.
Placement mismatch: supplied `/Users/jack/src/loopflow` has empty scratch and the earlier model. Read-only audit instead inspected `/Users/jack/src/loopflow.data-model-one-table-per` (all source citations below); this artifact and private probe remain in the supplied checkout.
Read that checkout's `scratch/remaining-work.md` §3, `scratch/data-model-one-table-per.md`, `docs/architecture/execution.md`, and prior `.lf/tmp/scratch-consolidation-20260928/scratch/parallel-discovery.md`. The supplied AGENTS and Infrastructure memory were read. Prior research is evidence, not an accepted prefix-search or cursor contract.

### Architecture and data flow

- **Exec:** `store/sqlite/execs.rs` exposes driver/connection/ancestry operations, not typed command inventory/detail/search. `execs` retains command, repo, parent, caller Session, times and outcome; it lacks Task command-context columns. Existing primary-key, parent/time/id, trace/time/id and recent-time indexes do not establish public discovery. No repo/command/caller-Session index was found. These indexes alone justify no new index before final reader SQL exists.
- **Existing process consumer:** `lf/commands/top.rs:473` first selects live exact process receipts, reads each Exec's journal events, then reconstructs `ExecRecord`; `ps/top` is live process observation, not historical `execs` list/detail. Session history references Execs separately. Do not replace exact OS evidence with SQL outcome when migrating discovery.
- **AgentSession SQL:** `store/sqlite/sessions.rs:12,87,590` selects conversation columns plus one input-reference join. Repo, interactive mode, completion/current-review membership, Task selector and search precede LIMIT/OFFSET and Rust row decoding. The old Run join is gone; Task/skill are Session columns. SELECT still copies request/ready-summary bodies and decodes iterations; it is not a narrow summary DTO.
- **Session filters:** Task accepts retained internal/public/external issue identity without PR/launch eligibility; there is no skill, parent, Wave, kind or rendered-state filter. `--history` removes completion and current-review restrictions; it does not select only completed rows. Rendered Closed also covers launched conversations without active clients, so Closed and `completed_at` are not interchangeable.
- **CLI defaults:** `lf/mod.rs:618` and `lf/commands/session.rs:60` select interactive=true, history=false, limit=100, offset=0. `--interactive false` selects headless; API `interactive=None` can combine modes but CLI cannot. `--all` removes repo filtering only; outside a discovered repository, repo=None likewise leaves all repositories eligible. No count/next-cursor envelope is returned.
- **Pagination:** `ORDER BY s.title,s.id LIMIT ? OFFSET ?`; limit=0 becomes -1. Fixed data has a deterministic tie-breaker, but rename changes position, completion changes membership, and separate commands have no common snapshot. Offset still walks discarded candidates even though Rust decoding follows SQL selection.
- **Contains search:** `instr(lower(s.title),lower(?))>0 OR instr(s.id,?)>0`. Title uses SQLite lower; ID is case-sensitive literal containment, not prefix. `%` and `_` are literal, empty text matches everything, and SQLite's default lower is not Unicode case folding. The private probe confirmed ASCII case behavior and É/é non-equivalence. No transcript, skill or command search occurs; title indexes cannot seek arbitrary containment.

### Payload loading and consumers

- **List enrichment is serial and unbounded per selected row:** `ops/human_session.rs:597,611,1429` reads managed Task Flow, placement/Home when applicable, provider-client receipts/liveness, provider identity, member Flow and ancestry labels. Managed members can decode their captured Flow twice: `task_flow` then `flow`. Graph construction maps the saved numeric node for the output DTO.
- **Hidden transcript scan:** `run_record.rs:825` reads `provider-session.json`; absence falls back to every line of `events.jsonl`. `surface` invokes this even for completed Sessions before choosing state. Missing both files retains the row, generally Waiting unless completed/ready; malformed/unreadable provider evidence can fail the whole list. A page limit therefore bounds row count, not bytes or process checks. This is a stronger remaining counterexample than the prior audit's generic “provider files.”
- **Desktop:** `swift/Loopflow/Services/RegistryQuery.swift:203` requests `session list --json --limit 0`, intentionally avoiding offset races. It offers no headless/history/search parameters. `RegistryQueryLocal.swift` runs reads off-main; that does not bound total work. `SessionsView.swift:157` treats incoming IDs as complete membership and removes absent IDs; feeding individual pages into this reconciliation would drop retained conversations. Replace paging and refresh reconciliation together, preserving surfaces keyed by Session ID.
- **FlowSession:** `store/sqlite/flows.rs:31,146` has exact ID and managed-Task-pointer reads, no summary inventory, repo/state/text search or pagination. `FLOW_SELECT` fetches full invocation/review/claim/failure JSON, then `read_flow` decodes it. It now derives selected outcome from Session events, not Run. Taskless cwd is stored; Task cwd can come from today's Task worktree. `task_invocation_history(task_id,updated_at,id)` remains; no pending_session_id index exists. `lf flow list/show` operate on reusable definitions, not saved FlowSessions. Desktop receives saved progress through Task and Session projections.
- **Historical reads:** `sessions.rs:205` filters immutable input attribution, caller and start-or-end time in SQL before `session()` + `history_for_input()` + `conversation_snapshot()`. It has no SQL LIMIT, repo or text predicate. Per-input history is unlimited; SQLite JSON inspection precedes filtering, and selected history is decoded/re-serialized/reduced afterward. `lf runs` generally caps 50 only after hydration; explicit Task/parent paths use since=0 and do not cap. Activity's end-in-window behavior must survive a shared query reduction.
- **Desktop discrepancy:** `RegistryQuery.swift:151` and `TaskRunsView.swift:65` still describe seven-day/newest-50 Task history, while `lf/commands/runs.rs:134` returns complete Task history. Update the consumer contract with the shared reader; do not infer bounded cost from that UI wording.

### Identity, detail and ambiguity

- `session_in` uses the Session primary key; `session_for_run` uses the input-reference primary key then Session primary key. `session history ID` requires exact Session ID and uses `seq > after ORDER BY seq LIMIT`; payload decoding follows the limit, with 0 unlimited (`session_events.rs:112`). This is an existing usable cursor pattern.
- `human_session.rs:625` first resolves exact Session/input ownership, then falls back to `run_record::resolve_manifest`. That fallback enumerates all artifact directories, matches full or stripped `run_` prefixes, rejects multiple matches, then loads one manifest. There is no SQL Session-prefix resolver, and an absent artifact can prevent prefix resolution even when exact indexed ownership survives.
- `sessions.rs:181` separately resolves historical input selectors: exact Session wins, otherwise UNION input_id/caller_input_id, substring-prefix test, ORDER BY id LIMIT 2. Ambiguity is rejected; caller-only historical identities remain selectable. This uses literal prefixes without the manifest resolver's stripped-prefix behavior. LIMIT 2 bounds returned identities, not the underlying UNION work.
- `lf runs ID` detail still resolves a manifest and reads selected snapshot/events/final-answer payload (`runs.rs:238`). Exact rows and missing-payload history are not yet one shared list/detail contract. A generic prefix rewrite must preserve exact precedence, ambiguity and caller-only identities deliberately.

## Observations

### SQL experiment, not CLI or dense-data acceptance

Private reproducible probe: `.lf/tmp/indexed-discovery-review/probe.py`; exact extracted Session SELECT/schema fragments plus reconstructed filters/binds, Flow schema excerpts and a pruned Task stub. Foreign keys/triggers disabled; small synthetic captures, no transcripts. Fixture: 20,000 Sessions, 5,000 Flows, 1,000 Tasks, three repositories, **zero Execs**. No application schema materialization or Rust decoding.
SQLite 3.50.4, macOS 26.0.1 arm64; recorded load average 5.82/5.19/4.40. Five in-process fetch-all samples per query, uncontrolled OS cache; these are SQL-only medians, not cold/warm CLI percentiles.

| Query | Observed plan/result |
| --- | --- |
| Repo/default, 50 rows | `session_repo_inventory` equality seek; correlated `SCAN f`; 0.250 ms median |
| Repo/history, 50 rows | Repo/mode seek, temporary ordering B-tree; 1.573 ms |
| All-mode/history, 50 rows | Session scan plus temporary ordering B-tree; 3.291 ms |
| Repo contains miss | Scans eligible index range; 0 rows, 0.619 ms; not a substring seek |
| Desktop repo/unlimited | 5,000 rows, correlated Flow scan; 116.614 ms |
| Proposed existing Flow-ID predicate | Same 5,000 fixture rows; Flow primary-key seek, 8.947 ms; no new index |
| Exact Session | Session/input primary-key seeks; 0.007 ms |
| Historical input prefix | Both UNION arms scan, temporary UNION/order B-trees, only 2 returned; 1.295 ms |

Fixed-data offset pages matched all 20,000 ordered IDs. Renaming the first row after page one caused the original row 51 to be skipped by page two. Adding `f.id=s.flow_session_id` to the existing pending-review EXISTS preserved this fixture's results; it has not established equivalence for imported unknown/mismatched memberships.

### Quality and tensions

`store/sqlite/durable.rs:1770` already protects retained names/history and direct discovery alongside an unrelated corrupt capture; `:2624` checks Task/mode/history selection. Those source assertions were inspected, not run. They do not establish bounded CLI enrichment, fixed-data paging, substring/prefix semantics or representative latency. Captured graph/node display and provider-client actions currently couple lightweight discovery to expensive detail; title ordering couples display preference to paging identity.

## Recommendations: smallest implementation and proof changes

1. **Remove repeated detail work first.** Share a metadata-only summary query between CLI/Desktop; omit request/capture/history bodies. Derive managed/member Flow identity and ancestry once, using existing IDs, then load graph/native payload only for selected detail or explicitly requested live actions. Preserve missing evidence as a visible row. Cost: query/DTO/consumer changes; benefit: actual byte bounds instead of speculative indexes.
2. **Try the existing Flow key before another index.** Add the owning Flow-ID predicate to pending-review selection only after proving historical membership equivalence. The SQL experiment identifies a concrete scan reduction; it is not migration acceptance. If retained unknown membership is required, determine that contract before deleting its discovery.
3. **One typed query contract per existing owner, shared consumers.** Add the missing Exec and Flow summary/exact-detail readers. Keep Exec command context separate from work performed across Session/Flow history; deduplicate before paging. Reuse PK/parent/Task indexes first and collect final plans before choosing more. Keep contains search unchanged; prefix identity lookup is a separate operation, not a substitute.
4. **Change pagination with Desktop reconciliation.** An immutable ordering key/cursor is a proposal, not Jack's accepted title-sort replacement. Prove fixed data, ties, rename/completion/insert between refreshes, ambiguous/exact/caller-only selectors, default/headless/history/repo scope, and retained selected panes. A cursor alone does not provide a snapshot across separate commands.
5. **Owner's decisive proof:** materialized disposable store with 100k actual-shaped Execs/20k Sessions/5k Flows across three repos/1k Tasks, large/missing/corrupt payloads, multiple Sessions per Exec and pre/post-bind history. Measure first-process and warm CLI list/detail median/p95 separately for startup/schema, SQL and payload; record plans, bytes opened and enrichment counts. Include all-history/contains miss/deep-page variants. This audit neither builds nor runs that proof.

## Source identity and open boundaries

Full SHA-256 manifest: `.lf/tmp/indexed-discovery-review/source-hashes.json` = `6d7f65572bc9772dc0210925df84333cccd28881180f5948a1a8eef300e30e3d`; private source copies preserve inspected bytes.
- `store/sqlite/sessions.rs`: `798b80b32732a66730fb6a0573236bd5703ca09d1134516d5183241f0787e00d`
- `store/sqlite/flows.rs`: `c32158aae2701bd1158db9f276f900a014d41e04fd022e9939653adb2fd840dc`
- `store/sqlite/execs.rs`: `602fa95bf80a14af34eb45c84669033fd812e3a81e6bb2daf64ee9aaac083ee1`
- `ops/human_session.rs`: `959856810f21627292e43752c5517f202f00c70dff2b8d04db22b48e5c2310d6`
- `run_record.rs`: `e161e4c67d466fec713021f2bed1039d9f5686fd3df40cc7e7773b56b9a5d3cf`
- `swift/Loopflow/Services/RegistryQuery.swift`: `289cb6246b9976d807495ebcd0420cc5ae16864633477cc0df4349c334c7bd44`
Probe evidence: `.lf/tmp/indexed-discovery-review/results.json` SHA-256 `36fe097afd944d18e5781b9d0a14145f8210eaf614ee74b639c28f70d2f60140`. `drift.json` records a final source comparison; no audited-file drift was detected at inspection close.
Unproven: full materialized schema/planner parity with bundled Rust SQLite; complete Exec API; all-provider/installed behavior; cold/warm application costs and p95; zero transcript/graph reads on list; final Desktop paging and payload retention. No code-complete or acceptance claim follows.

## Supervisor handback

Returned with exit0 and copied into the LOO-298 checkout on 2026-09-29, including
probe, schema, results, hash manifest and inspected source snapshots. Original
placement above records where the research ran; this copy is the working artifact.
Generated SQLite databases were not copied; the probe rebuilds its private fixture.
Post-rebase source comparison and transfer hashes: `.lf/tmp/indexed-discovery-review/handoff.json`.
