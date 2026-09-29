# Research: Session wire and Desktop finish line

## System understanding

2026-09-29 · Bounded read-only research for Jack Heart, LOO-298. Confirmed cwd `/Users/jack/src/loopflow.data-model-one-table-per`; inspected HEAD `db60d989665f3c1787c36cd019a6f33dc6e30020`.
Read the governing guide, accepted contract, remaining-work §§3/5, questions, current documentation and `.lf/tmp/cut-i/desktop-ancestry-handback.md`. No tests, builds, provider launches, installed-Home access, PM writes or Git mutations ran. This artifact remains uncommitted.
Source receipt: `.lf/tmp/session-wire-review/source-hashes.json` contains full repository-relative paths and SHA-256s. Main changed `rust/loopflow/src/ops/human_session.rs`, `rust/loopflow/src/run_record.rs` and `rust/loopflow/src/run_record/active.rs` during inspection; initial/closing hashes are retained. Their public declarations were rechecked unchanged. Continuation/active implementation findings are bounded to inspected bytes, not an assessment of main's ongoing repair.

### Architecture and data flow

SQLite owners already differ from their public presentation: `agent_sessions` and immutable `agent_session_inputs` feed a stable conversation DTO; Session observations feed a per-input `RunSnapshot`; captured Flow state feeds a second, structural-key graph. Swift consumes these projections through `RegistryQuery`, then `PodiumModel`, workspace grouping, terminal stores and detail views. It does not read SQLite.

| Public JSON producer and source | Swift DTO and consumer | Remaining boundary |
| --- | --- | --- |
| `lf session list/connect/rename/bind --json`: `rust/loopflow/src/lf/commands/session.rs` → `rust/loopflow/src/ops/human_session.rs::SessionRecord/surface` | `swift/Loopflow/Models/SessionRecord.swift`; `swift/Loopflow/Services/RegistryQuery.swift`; `swift/LoopflowMac/PodiumModel.swift`, `swift/LoopflowMac/Views/SessionsView.swift` | Stable `id`, typed nullable `work/wave_id`, interactive mode, actions and membership exist. Required `run_id` is the current immutable input reference. Membership node remains string-valued. |
| `lf session history ID --json --after N --limit N`: same command module → `rust/loopflow/src/store/sqlite/session_events.rs::session_history` | `swift/Loopflow/Models/SessionEvent.swift` | DTO mirrors nullable thread/turn/generation/Exec/Task/Wave and lossless payload; no production Swift query or history view consumes it. |
| `lf runs`, detail, `lf usage`: `rust/loopflow/src/lf/commands/runs.rs`, `rust/loopflow/src/lf/commands/usage.rs` → SQL conversation inputs → `rust/loopflow/src/run_record.rs::RunSnapshot` | `RunSnapshot/RunUsageSnapshot/RunSubjectAttribution` in `swift/Loopflow/Services/RegistryQuery.swift`; `swift/LoopflowMac/Views/TaskRunsView.swift`, `swift/LoopflowMac/Views/TelemetryDashboardView.swift` | Per-input result/usage still presented as Run; string subjects and caller-input `parent_run_id` are not typed Exec ancestry. |
| `lf wave status --json`: `rust/loopflow/src/lf/commands/waves.rs::WaveDetailSnapshot.runs` | `WaveDetailSnapshot.runs: WorkEvidence<RunSnapshot>` in `swift/Loopflow/Services/RegistryQuery.swift` | Another consumer of the same historical projection; preserve unavailable/truncated evidence when replacing it. |
| `lf activity --json`: `rust/loopflow/src/lf/commands/activity.rs::WorkActivitySnapshot` | `swift/Loopflow/Models/WorkActivity.swift`; `swift/LoopflowMac/Views/WorkActivityView.swift` | Typed Work plus display `subject`; `run_started/run_finished` still name input IDs. Swift additionally accepts invocation/trace/Exec fields absent from this Rust producer. |
| Task status/roadmap and `lf flow list --json`: `rust/loopflow/src/ops/task_flow.rs`, `rust/loopflow/src/lf/commands/waves.rs`, `rust/loopflow/src/lf/commands/flow.rs`, `rust/loopflow/src/engine/flow_graph.rs` | `swift/Loopflow/Models/TaskFlow.swift`; `swift/LoopflowMac/Views/TaskFlowView.swift`, `swift/LoopflowMac/Views/WorkspaceBreadcrumbBar.swift` | Managed Flow projection/catalog, not saved-Flow inventory/history. Node/current/completed/return identifiers are structural strings. |
| `lf runs --active [--watch] --json`: `rust/loopflow/src/lf/commands/runs.rs` → `rust/loopflow/src/run_record/active.rs` | `swift/Loopflow/Models/ActiveRunsSnapshot.swift`; observation services and `PodiumModel.activeRuns` | Typed Work coexists with subjects and Run/input identity. Main owns active-reader replacement; coordinate its final wire separately. |
| `lf ps --json`: `rust/loopflow/src/lf/commands/top.rs::ActivitySnapshot` | `swift/Loopflow/Models/ActivitySnapshot.swift`; `RegistryQuery.processActivity` | Live Exec/provider-process tree, not durable Exec command history. No outcome/exit/signal, incoming `via_agent` or calling Session on this wire. |

### Key abstractions

- `rust/loopflow/src/session.rs::AgentSession.input_id/caller_input_id` are captured-input references, typed with historical `RunId`; they have no resumable lifecycle. `SessionRecord.run_id = session.input_id`. No production Swift `.runId` use was found outside the DTO itself; fixtures explicitly require it. Removing an unused summary field must preserve input lookup, replay, historical attribution and caller relationships internally.
- `RunSnapshot.id` is also input identity; replacing it with Session ID would collapse failed/replaced members of one conversation. `parent_run_id` records the immutable caller input, not necessarily a distinct process. Neither field can be renamed to Exec ID. History needs its existing Session/event/input correlations without creating another public attempt object.
- `rust/loopflow/src/durable.rs::Author::Run(RunId)` and `swift/Loopflow/Models/WorkStatus.swift::WorkAuthor.run` also expose old identity in Steer provenance. Preserve original author evidence; no proof permits reinterpreting every old author as a process or current driver.
- Flow storage already uses numeric preorder nodes: `rust/loopflow/src/engine/invocation.rs::QueuedInvocation::node_id/location`, `AgentSession.node: Option<u32>` and `flow_events.node`. XOR alternatives participate in sorted preorder. The numeric ID is local to its captured Flow; iteration tuple and Flow identity remain necessary.
- Public `FlowNode.id` is an optional authored policy label, distinct from structural `key` and stored numeric node. Do not replace all three by parsing numeric-looking strings. Template-composition `parents` are provenance labels, not runtime FlowSession parents.

### Evidence and counterexamples

`rust/loopflow/src/ops/human_session.rs::surface` derives Task/Wave from Session columns, not roadmap, manifest subjects or active PRs. `swift/LoopflowMac/WorkspaceProjection.swift` joins planning for labels, retains typed ancestry for unmatched bound Sessions and excludes them from orphans. `subject(for:)` can use planning Task identity when matched and durable Work identity when unmatched; `PodiumModel` deliberately resolves both. This is presentation reconciliation, not authority to infer a missing binding.

Unknown Work remains nullable; missing Flow, independent Flow and known historical membership with unknown node/iterations are different wire cases. Existing fixtures cover them. Display `work_path`, title, cwd and `WorkActivityEntry.subject` must remain labels, never identity fallbacks.

`swift/LoopflowMac/Views/TelemetryDashboardView.swift::work` still parses `task:`, `project:` and `wave:` from `RunSnapshot.subjects`. Rust reconstructs those display selectors from retained input attribution. This redundant typed-owner → string-selector → UI-label path remains deletable after historical attribution is carried directly.

`rust/loopflow/src/engine/flow_graph.rs::node_at` converts numeric IDs back to structural keys for membership. `project_cursor` separately creates current/completed/return keys. Swift uses string prefixes to infer containing XOR/path and selection (`TaskFlowView.swift`, including `state`, `nodeButton`, `FlowNodeDetail`). Numeric conversion therefore includes those relationships; changing `String` to `UInt32` alone loses nested highlighting.

`PinnedTaskFlow.completed` means positions finished in the current pass, not all successful history. The diagram's labels/counts cannot prove which Session completion advanced a step. `flow_events` retains selected/consumed Session-event references and mechanical outcomes; no Swift Flow-history consumer was found. `SessionEvent` alone cannot represent mechanical boundaries or Flow consumption.

## Tensions

- Stable conversation identity versus per-input evidence: default grouping belongs to Session; old outcomes and usage retain their original input/event owners. The prospective-bind assumption remains unresolved policy, not permission to bulk relabel prior work.
- Complete inventory versus bounded pages: Desktop currently requests `--limit 0` to avoid offset races. `SessionsStore.reconcile` removes absent IDs, and `PodiumModel.refreshSessions` clears an absent selected Session. Passing a page as the whole inventory would discard selection/surfaces.
- Process result versus work result: the live `ps` tree and per-input Run projection cannot substitute for durable Exec completion. One Exec may drive several Sessions or mechanical boundaries; a successful provider turn does not prove command success.
- Managed Task diagram versus durable Flow history: the breadcrumb only opens nodes in the matching current managed invocation. Past and taskless Flow history remains unavailable through that UI; matching a node number in today's graph is invalid.

## Observations

### Complexity and discovery

`rust/loopflow/src/store/sqlite/sessions.rs::inventory_query` filters repo/Task/mode/history/search before SQL LIMIT/OFFSET, ordered by title then ID. `--interactive` defaults true; false selects headless; `--history` includes completed and historical reviews; `--all` removes only repository scope. `limit=0` means unlimited. Contains search is SQLite `instr(lower(title),lower(query))` OR literal ID containment, not a prefix search or Unicode case-folding promise.

`RegistryQuery.sessions` exposes none of the mode/history/search controls and always sends `session list --json --limit 0`. `PodiumModel.readSessions` returns empty for no selected repo. Thus CLI all-repository/headless/history capability does not imply Desktop discovery. A future mode change must preserve open panes omitted by that filter.

Selected Session enrichment still reads more than summary columns and projects captured graphs; SQL row filtering alone does not establish bounded bytes/cost. `SessionEvent` detail already has sequence paging before payload decoding. `summary_for_input` excludes known transcript kinds in SQL but selects only observed input evidence; it is not a complete native-only usage reducer.

`lf runs --task` and `--parent` return complete matching history; the generic list uses seven days and caps at 50 after hydration. `TaskRunsView` and `RegistryQuery.taskRuns` still promise seven days/newest 50, and the view infers truncation from `count == 50`. Correct that contract with the owner conversion; do not silently restore a cap to make the text true.

### Quality and existing proofs to retain (source inspected; none executed here)

| Behavior | Exact existing proof locations |
| --- | --- |
| Required wire fields, unknown membership/native origin, current/earlier/past, nil node/iterations | `rust/loopflow/tests/dto_fixtures.rs::session_history_retains_receipts_and_unknown_driver`; `rust/loopflow/src/ops/human_session.rs::flow_membership_fixtures_cover_every_projection_and_are_required`; `swift/LoopflowTests/DTOFixtureTests.swift::{sessionHistoryFixture,sessionFlowMembershipFixture,sessionRequiresRun}`. Change the last test's obsolete product wording while preserving any retained input-reference requirement. |
| Missing planning is not unbound; cold discovery/restoration and sibling identity | `swift/LoopflowTests/WorkspaceNavigationTests.swift::{boundSessionRetainsAncestry,unboundSessionHasNoAncestry,orphansStayOutOfTheTree,unavailableIsNotEmpty,lastGoodSessionsSurviveRepositorySwitch}`. Four bound variants remove Task, current Project, readable planning or Wave. |
| Mounted pane/focus/draft and same native surfaces, rename failure, exact Flow link | `swift/LoopflowTests/WorkspaceNavigationProofTests.swift::namedSessionDrillDownRetainsTerminal`; it tests planning disappearance/return, repeated/nested skills, original draft/PTY replies and refusal to link a prior invocation into the current graph. `swift/LoopflowTests/SessionRenameTests.swift::{heldRenameSurvivesNavigationAndPolling,rejectionRetainsDraft}` retains mutation/poll ordering. |
| Nested numeric-to-wire occurrence and independent backward-edge counts | `rust/loopflow/src/ops/human_session.rs::stored_session_membership_resolves_nested_and_post_xor_graph_nodes` (stored nodes 3/4/0); `rust/loopflow/src/engine/flow_graph.rs::{repeated_decisions_keep_independent_counts_and_a_pass_scoped_completion,a_selected_xor_path_is_drawn_honestly_with_nested_keys}`; `swift/LoopflowTests/TaskFlowProofTests.swift::{flowFixtures,occurrenceStates,flowControlsRetainTerminals}`. |
| Scope before paging, complete changing inventory | `rust/loopflow/tests/session_cutover_tests.rs::inventory_scopes_before_paging_and_keeps_worktree_repository_identity` selects local rows beyond 110 foreign rows, exact Task/search and offset; `swift/LoopflowTests/RegistryQueryTests.swift::sessionsReadCompleteInventory` retains 101 rows across rename/insertion/removal through a mocked complete response. This is not concurrent SQL paging or latency proof. |
| Interactive/headless import visibility and historical ownership | `rust/loopflow/tests/session_cutover_tests.rs::{import_stores_each_old_session_once_with_its_name,import_retains_replaced_inputs_without_rebinding_their_history,binding_starts_the_task_once_without_reattributing_prior_work}`; first combines explicit true/default and false inventories with `--all`. Preserve existing Started and pre-bind unknown/zero usage. |
| Lazy history, late replies and terminal retention | `swift/LoopflowTests/TaskRunsProofTests.swift::runsFollowTheirTask`; `rust/loopflow/src/lf/commands/runs.rs::history_window_keeps_boundary_usage_and_gaps_before_payload_reads`; `rust/loopflow/src/lf/commands/activity.rs::{filters_apply_before_ordering_and_cap,ordering_has_a_stable_tie_breaker}`. Activity includes end-in-window evidence independently of recent-start history. |
| Active observation stays separate from completion | `rust/loopflow/tests/dto_fixtures.rs::active_runs_preserve_identity_waiting_clients_and_incomplete_evidence`; `swift/LoopflowTests/DTOFixtureTests.swift::{activeRunsFixture,activityFixtureRoundTrips}` and `swift/LoopflowTests/ActiveRunsObservationTests.swift`/`ActiveRunsLifetimeTests.swift` remain downstream obligations for main's active-reader cut. The latter suites were identified, not deeply audited. |

Shared fixture edits must cover `tests/fixtures/dto/{session,sessions,session_actions,session_memberships,session_history,task_flow,task_flow_stalled,flow_catalog,roadmap_snapshot,wave_detail,active_runs,activity_snapshot,work_activity_snapshot}.json` as applicable; nested copies and inline test JSON also matter. Rust/Swift fields remain required or explicitly optional, without defaults that hide drift.

The Desktop ancestry handback is already integrated in inspected source; this research does not reestablish its recorded passes. `PodiumModel.refreshSessions` already avoids publishing equal readings and rejects stale generations. Preserve that behavior; equality of DTOs is not a measured repaint/latency guarantee.

### Potential

The stable Session-keyed terminal pool and typed Work references already support most presentation conversion without replacing pane ownership. Existing Session sequence history and captured numeric locations supply the foundations for history/detail. Remaining work is joining/exposing accepted owners and removing redundant projections, not introducing a new lifecycle.

## Recommendations

### 1. Convert the Session summary and Desktop discovery together
**Observation:** stable identity/ancestry exists; unused required `run_id`, missing Desktop filters and whole-inventory reconciliation are the boundaries.
**Cost:** medium; Session producer/DTO/query, fixtures and reconciliation policy. Main owns connect implementation; Desktop still invokes its `open` alias and `--replace`, while current parser names `connect` and has no `--restart` flag.
**Benefit/verdict:** retain Session IDs, nullable ancestry, action/readiness semantics, last-good values and panes across filters. Add the accepted Desktop bind confirmation through the same owner: no Swift bind query/action was found. Removing unused public input fields is reasonable only after internal selectors/history remain proven. Paging/contains semantics require explicit coordinated treatment, not a new API invented by this audit.

### 2. Convert numeric graph identity as one cross-language boundary
**Observation:** storage, graph projection, membership and Swift navigation currently represent the same occurrence twice.
**Cost:** medium; numeric graph/current/completed/return/membership fields, captured-ID mapping, Swift nested traversal/selection and all graph fixtures in one change.
**Benefit/verdict:** delete numeric-to-path `node_at` adaptation and wire-only structural-key/prefix inference after consumers use captured numeric identity and graph containment. Keep authored IDs, FlowSession identity, unknown node/tuple and iteration counts; do not delete runtime cursor operations merely because some still use path strings.

### 3. Replace historical Run presentation through its actual evidence owners
**Observation:** `runs`, usage, Wave detail and activity converge on input observations, while Swift already has an unused SessionEvent mirror.
**Cost:** larger, separable from graph conversion; typed historical attribution, exact provider outcomes/usage and Flow selected/consumed/mechanical history must be carried through their query/view/fixture consumers. Durable Exec inventory is still a separate missing producer, not a rename of `ps`.
**Benefit/verdict:** delete `RunSnapshot`/subject parsing and `WorkActivityRunIdentity` fallback only after all consumers migrate, including telemetry/Wave detail and retained old selectors/authors. Reuse command outcomes from Exec; never copy them into Session completion. Retain distinct failed/successful histories and missingness. Coordinate ActiveRun replacement with main; do not redesign its process ownership here.

## Open questions and acceptance gaps

No source proof here establishes Desktop bind confirmation/terminal retention through bind, headless/history/all-repository discovery, Session restart identity, native-history rendering, or mechanical/exact-consumption step history. Existing Flow restart tests exercise Task Flow controls, not AgentSession engine restart. Configured Desktop/provider acceptance remains separate.
No complete public durable Exec summary/detail wire or Swift mirror was found; FlowSession inventory/history likewise lacks a Desktop query. Existing live process trees and managed Task diagrams do not close those requirements.
Final paging/refresh contract and summary/detail fields are unresolved implementation choices; preserve literal contains search and all-repository meaning. Fixed-data pages, ambiguous selectors, mutation between reads, bounded enrichment and dense cold/warm timing remain owed. Do not count the 101-row mock as these proofs.
Prospective bind attribution remains the labeled supervisor assumption. Native decision transport remains unresolved and outside this audit. No new policy, retry interface, launch authority or Flow edge is selected.
