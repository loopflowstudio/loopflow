# Historical import preservation map

2026-09-28 · LOO-298 · Bounded read-only contribution for the managed worker.
Inspected dirty checkout at `d07e569330c8dedceb5dd238ce9b22a7b6137006` after
reading `data-model-one-table-per.md` and `parallel-work.md`. No builds, tests,
provider calls, Home access, patches, or executable edits. Fixtures below are
source evidence, **not passing results**. Only this note was written.

## Contract and source vocabulary

Destination names below describe accepted ownership, not proposed SQL columns.
Current source still has `Run`, `runs`, `sessions.current_run_id` and
`flow_invocations.current_run_id`; these are historical inputs to conversion.
Exec means an actual lf process. AgentSession owns conversation and subordinate
provider history. FlowSession owns capture, navigation and mechanical results,
and references the exact successful agent-history completion. Neither a Run nor
a provider retry establishes another Exec. No replacement attempt object is needed.

Paths in the map are relative to `rust/loopflow/src/`; tests are under
`rust/loopflow/tests/`. `I` means `ops/session_import.rs`; `M` means
`store/migrations.rs`; `C` means `session_cutover_tests.rs::import_stores_each_old_session_once_with_its_name`.

## Input → owner, invariant, distinguishing assertion

| Persisted input and current path | Destination / lifetime and invariant | Existing fixture → concrete missing assertion |
| --- | --- | --- |
| Interactive `manifest.json`: Run ID, creation, surface, cwd/repo, subjects, parent; name `{title,source}`; resolution timestamp; native reference. `I::run`. | AgentSession keeps established conversation selector, human title precedence and closure; agent history retains outcome/caller and native/account evidence. Closure is not provider success. | C checks old ID/name/Task. Add closed interactive with terminal failure and parent: preserve all three independently, plus repository scope. Current constructor drops parent/end and sets repo NULL. |
| Ask JSON: `id`, `parent_run_id`, `work`, `prompt`, `skill`, cwd/model, optional `session_run_id`, ready summary, Waiting/Completed summary. `I::ask`. | AgentSession retains request, exact keyed identity and completed answer; caller is causality. No prepared Run means a reservation, not an observed process. File mtime is inferred completion/creation evidence, not measured execution time. | C has a waiting unkeyed Ask. Add completed `ask_once_<hash>` and retry through the keyed path: same answer, no launch, no invented Exec or successful turn; retain timestamp provenance. |
| Released `task_flow_positions`: invocation JSON, root indices, version/generation, claim/failure/review JSON, session Run ID, ready summary, updated time. `retain_flow_invocations` → `own_sessions_and_runs`. | FlowSession keeps capture, exact nested cursor/return counts, claim and review association; AgentSession keeps review feedback/history. Preserve unmapped historical fields until explicitly accounted for. | M `retaining_invocations_preserves_populated_execution_and_review_bytes`, `session_ownership_import_preserves_nested_reviews_and_nullable_parent_constraints`. Carry their byte/identity assertions through the final target schema, including flat/unmapped input; C's Task review is seeded through current APIs, not a released-schema upgrade. |
| Existing SQL Sessions, all member Runs, selected current Run; `replace_session_run`, `rename_session`, ready/complete writers in `store/sqlite/sessions.rs`. | One AgentSession survives replacement; ordered member history and each outcome remain. A current pointer is not proof of successful completion. Human name/feedback never become per-attempt copies. | Existing replacement/store fixtures cover current-Run fences. Add import with two failed/replaced members and one success: every member retained, stale completion rejected, one exact Flow completion reference. `I::review_evidence` enriches only the current member. |
| Taskless `flows/<id>/position.json`: ID, flow, cwd, captured steps/cursor, message/model, selectors, active boundary/run/feedback/completed, finished; historical failure where present. `I::FlowFile`, `flow_review`. | FlowSession exists regardless of review state; source-independent capture and unknown failure/completion evidence survive. Never reconstruct history from today's cursor or catalog. | C covers only an active unfinished human review. Add autonomous pending, failed and finished Flows, nested cursor, and a never-opened completed/ready review. Current parser omits failure; importer skips non-review/finished Flows and the no-Run branch drops saved boundary feedback/completion. |
| Headless manifests and earlier review Runs; `flow` is absent, Independent, or Step with invocation/node/iteration tuple; node/tuple may individually be absent. `I::headless`, `RunFlowStep`. | AgentSession history for agent work; FlowSession association only to evidenced historical position. Missing membership stays unknown, not Independent. A moved cursor does not erase a recorded earlier position. | C explicitly expects the historical step's invocation to become NULL. Replace that expectation with preserved known association and separate unknown-node/tuple cases; add headless native conversation identity. Current import creates no headless Session and nulls all position fields. |
| `events.jsonl`: envelope sequence/time; ProviderAttemptStarted/Finished, SessionObserved; provider/model/account, attempt key, native ID, turn/usage-stream IDs, sequence/counter kind, start-known/final flags and optional counters. `run_record::Recorder`, engine retry writer. | Subordinate AgentSession history retains failed and successful provider continuations and account/native changes. Namespace local attempt keys by their original owner; `attempt-0` is not globally unique. Missing counters, observed zero and partial/final coverage stay distinct. | `run_record` tests `retry_usage_keeps_provider_cumulative_values_in_distinct_streams`, `usage_keeps_omissions_and_resets_sequence_for_each_provider_turn`. Add imported multi-Run/multi-account retry history: equal totals/coverage, distinct keys, exact successful completion reference, no extra Execs. |
| `terminal.json` outcome/time/result reference, Run SQL outcome/end, Result and provider-finished events. `Recorder::finish` writes terminal before SQL. | Agent-history completion or mechanical Flow history as appropriate; preserve conflicting/missing evidence explicitly. A terminal receipt, Session closure and command completion are different facts. | Existing terminal immutability tests cover capture. Add interrupted SQL-end write and conflicting file/row outcomes during conversion: no silent winner or invented success. Current Ask/review/interactive import sets Run end NULL despite available terminal evidence. |
| Mechanical Runs (`provider=loopflow`, captured Op node), outcome or missing receipt; `store/sqlite/flows.rs::settle_attempt_in`. | Mechanical result belongs directly to FlowSession history. In-process Op creates neither AgentSession nor new Exec; missing result retains inspect-before-retry uncertainty. | `an_interrupted_operation_blocks_for_inspection_instead_of_replaying`. Add populated import with multiple Ops in one actual Exec, including effect-without-receipt; preserve blocked recovery and distinguish agent completion. |
| Command `run_events`: `process_id`, trace in `run_id`, parent process, command/repo/worktree, node/event/seq/time/error. `record_execs` draft; journal runtime. | Exec identity comes from process_id, trace remains grouping, outer run-node terminal establishes command result. Multiple agent/mechanical boundaries can share one Exec. Parentage grants no signaling/settlement. | `journal` nested-runtime tests and `exec_ownership_tests` inspection/failure/interruption tests cover new writers. Add historical mixed-node journal import: one process/many Runs, child process, completion-only history, missing outer end, unknown exit/signal and absent via-agent evidence. |
| Exact native client PID/start, host/boot, account/native reference, worker owner trace/Exec/PID/start; current driver/provider generations. | Operational evidence remains distinct from historical identity. Old manifest Run parent is not proven direct/agent Exec edge. Unknown historical process stays unknown; import must not acquire driver claim or invent a provider generation. | M `shared_flow_driver_preserves_claim_authority_and_current_attempt`; `exec_ownership_tests::actual_engine_children_follow_driver_handoff_but_not_provider_replacement`. Add imported missing/reused PID plus retained claim: passive read cannot control, stale provider cannot gain replacement authority. |
| Historical Task/Wave/source per Run, Task `started_at`, Started events; current `bind_session_runs_in` bulk-updates Session Runs. | Preserve recorded usage owners and monotonic start evidence. Exec observation alone never starts Task work. Existing retroactive bind bytes cannot prove original attribution; report conflicts instead of inventing earlier owners. | M nested-review/first-assignment assertions and Exec inspection fixture. Add pre/post-bind usage with replacement, done Task, unpublished reservation and Started-only history. Prospective attribution is the supervisor's assumption, **not Jack's approval**; do not rewrite historical usage on that basis. |

## Reachable gaps to fix before deleting inputs

1. **Import idempotence currently hides conflicts.** `I::store` returns Unchanged
   on Session-ID existence alone. A changed answer/member/capture can be skipped.
   Ask/review IDs enter `claimed` before their import succeeds, suppressing later
   manifest processing for that Run. Prove identical replay versus conflicting
   input, and retain an actionable failure without reporting complete conversion.
2. **Taskless attribution can disappear silently.** `FlowFile::declared_work`
   invokes launch resolution and turns errors into None. `I::work` takes a Task's
   current Wave without checking a separately recorded Wave; Wave-only lookup uses
   slug. Test landed/done Task, renamed Wave and conflicting ancestry. Historical
   identity resolution must not require launch eligibility or fresh active PRs.
3. **Current importer cannot justify dropping Flow files.** It handles only a
   waiting review. Its no-Run path creates Flow then reserves review separately;
   interrupt between writes and repeat, retaining original boundary identity and
   feedback. Preserve failed/completed/autonomous capture before any deletion.
4. **The Exec draft fabricates start precision for incomplete journals.** It
   selects each process's earliest event as `started_at`, even if that event is
   completion. Preserve observed timestamp with its actual meaning; no Run-based
   process backfill. Keep unknown via-agent, exit code and signal unknown.
5. **The current fixture's deletion proof is narrow.** C removes selected source
   files but keeps manifests and position files; it does not prove final readers
   are independent of all retired owners. Its fixture evidence list also omits
   the native-reference file. Extend preservation/read-after-removal assertions
   while retaining provider-native history and immutable payloads as evidence.

## Smallest safe order and migration boundaries

1. Inventory the released SQL boundary, filesystem evidence and dev-only rows
   separately on disposable copies. Reconcile exact IDs and conflicts before
   mutation; label inferred timestamps, unknown process/membership and unavailable
   payload. Add the map's counterexamples to existing populated fixtures first.
2. Preserve complete Flow captures and conversation/member histories, then map
   historical successful completions and mechanical results under existing fences.
   Preserve old IDs as historical selectors without retaining Run as a product.
   Import process journal evidence independently; never manufacture an Exec join.
3. Atomically convert references, Task managed-Flow pointer, review bindings,
   claims, Started invariants and history attribution. Unresolved inputs remain
   explicit evidence; a partial successful count cannot authorize dropping them.
4. Switch writers/readers together; prove stable identity, exact one-time Flow
   settlement, unknown evidence, usage equality, interruption/retry and ordinary
   read behavior. Only then remove Run owners, import-only columns, old subject
   resolution and mutable sidecar consumers. Keep retained native/payload evidence.

**First release:** start from populated shipped schema plus all four filesystem
origins; SQL migration alone cannot import filesystem history. Exercise final
canonical materialization through `current_draft_sql`/migration test helpers,
not direct draft-file includes. Finish the upgrade and validate final foreign
keys and actual user reads, including evidence the old SQL migration could not map.
**Already-applied development drafts:** include existing Sessions/replacements,
headless rows, Execs and live-shaped claim bytes at each relevant applied frontier.
Use a forward draft; do not rewrite applied checksums or replay an old draft over
newer schema. Materialize only in a disposable exact-input source copy per
TESTING.md. Neither path authorizes installed-Home conversion or promotion.

## Inspection fingerprint

HEAD and these hashes were unchanged at the second read. Main is concurrently
editing; this is a bounded source snapshot, not certification of later bytes.
SHA-256 paths below are relative to `rust/loopflow/`:

```text
77c1eb12ae92dca823f09a9d84a0592e97e3cdc81d86748267da9ddd4bd35858 src/ops/session_import.rs
0676d7d68d49ba68d605689b85bf3a62c56444221041b3a35417f8475709ebc9 src/session.rs
c3435df540aa8d5af9c0c3b532d17ae5dc671f3520b6b7cb159ee57aaf4ebfde src/run_record.rs
e3fd676c633867e23154c47850c2e93c711697684c11b4440463ae2cd4162ce1 src/journal/mod.rs
5a7b1fa4db7a2a980a3084d576ba88776f336ff1a3dcad7c12b562410e3351e3 src/store/sqlite/execs.rs
7b6387f3bc051a127ce644231d0ccac415cf2a769b2a64646eee47c2561972d5 src/store/sqlite/sessions.rs
e854e1720d4f929dd1739b4dc48cdf20ecfb3f247344ca73594aee550849f2e2 src/store/sqlite/runs.rs
4d52ec1ab55b2535111ca9f12b9d579c689afaa50e8aa0f9f53b4781f24ec50d src/store/sqlite/flows.rs
430cb7f898920d39821db03e06d0e459c149a23fe05c9a3b67ed1a8879b3805c src/store/migrations.rs
dc3e13687297ca82b52adabcf540833ba5e55eab744eaa0a4e5a67e91abe71d2 src/engine/agent.rs
18e20b4f0e1d4de0d96cdc88667934769a0592991a5300aedebb43ae5f315ea5 tests/session_cutover_tests.rs
66a80c5725c317b5b13db711535e02591d7364bd3492cc3ebefa7c06131aa92c tests/exec_ownership_tests.rs
32bab6db4165772526c93fc023b71c2331663e1f1b7a236f2df93d3503fdf0aa draft-set (17 files)
```

Draft-set digest hashes sorted `src/store/migrations/drafts/*.sql` entries encoded
as `<sha256><two spaces><basename>\n`; it fingerprints all drafts, including the
retained invocation, Session/Run, first-assignment, shared-driver and Exec cuts.
