# Current state and history inventory

2026-09-30 · Jack Heart requested: “Whatever history or extra state we don't need, toss it now.”

Keep Waves, Projects, Tasks and links, accounts/routes, resumable conversations, and current execution authority. New conversation/Flow events remain necessary for retries and settlement. No installed database is opened for writing.

The checkout contains 38 draft files; the supplied count of 19 is stale. Three landing groups replace the chain.

| Item | Lines before | Classification |
| --- | ---: | --- |
| `rust/loopflow/src/store/migrations/drafts/record_invocation_attempts__3c5ad598265e88c1f90e9426c7e54aa7.sql` | 30 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_imported_session_evidence__4a7eb8b1d3e14f7c8e925db0134fd9b8.sql` | 48 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/index_session_process_owners__eea16f37a14f41e2b5d8767d7e206598.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/runtime_flow_children__cfd282b86492452a8e94988d08625962.sql` | 28 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/drop_flow_turn_caller__0ccaa0f715a7407b846b1236e293ef68.sql` | 11 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_session_output__98630201d91945deac760c629972912d.sql` | 57 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_native_input__e2670f219b194c2698e8a9c310b6bd31.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/name_conversation_and_flow_owners__66bf43043eb242ca93a9e4427afef334.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/name_tasks_on_flow_step_runs__24f1227c06793e7da8da1d1c286089b6.sql` | 30 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_session_kind__6ba466cd248b5f305c7b24acd7bc63e7.sql` | 13 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_flow_turn_caller__c62d3a026776492ea7218689087df653.sql` | 9 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_flow_repository__9a25318e934745ca8b87d6a57eb4e54c.sql` | 13 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_run_work_source__6efd345d9a6d381e3e6a3888651f406f.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/project_status_chapters__9bea0388aa854e46a9db2d1e5c86ddfb.sql` | 60 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/drop_wave_services__355d8c5fd2e34f6c8833f276c6ba0422.sql` | 11 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/own_flow_launch__cf154ede27536f017b48d0b212f23c73.sql` | 35 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/drop_task_flow_step_projection__3ddf67770302639aa5d3833354d71645.sql` | 11 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/point_task_at_invocation__ef3fa1278026a9a2f7bdf0f19fd32209.sql` | 60 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_task_start_evidence__899bb8f4ca3d4437b450e5a206e02bbb.sql` | 27 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_input_sql_evidence__08eb814a9a324ef38c09ed1c7958f65e.sql` | 81 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_conversation_engine__c52db0bb8d2b4d6b848d8974fca2a7be.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/one_flow_driver__aa92827cf174386a754dcebb910303a5.sql` | 21 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_conversation_connection__87b194c675ad48d5af917c2728b3349f.sql` | 8 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/capture_session_events__76852b62431d481188c741fc4dd8999a.sql` | 310 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/flow_operation_history__8a533c4383974c1599d1ccf076665c63.sql` | 59 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_session_events__b3d8bbde097141a596c4562954131457.sql` | 23 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_task_first_run__b00a8bb58c57bf79085aba8e56c3f43b.sql` | 58 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_execs__8e1f10371bfa03133ea590b179afc4cf.sql` | 44 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/agent_session_admission__a89e13f5c5d2439da777324d31101db8.sql` | 140 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/own_sessions_and_runs__60c1dd27bd0474a7cecf89527353aced.sql` | 119 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/fold_flow_passes__7ab1cd6819ce77acd75bda26b3b2de0c.sql` | 112 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/conversation_mode__d2b08e73a45147e3b99c078dd60496a2.sql` | 19 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/index_session_metadata__ad34a971951e43b3ab72c983cc2d2dd8.sql` | 16 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/select_flow_history__abb59c97154347cfac291e7529bed420.sql` | 17 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_ask_sessions__ec6d1b7d0212a1de5be053d42b6d7721.sql` | 12 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_flow_invocations__b85b73f238426ffa86687ee09c801a0d.sql` | 53 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/record_run_end__bfe6920a245554ca8d02021c414a9d38.sql` | 11 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/retain_named_operation_history__3704bb0dbb1b462fa165973072c5309d.sql` | 40 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/store/migrations/drafts/own_conversation_ancestry__7a5cbe8208b34af886d520986a378458.sql` | 87 | Mixed: replace with direct final-schema migration |
| `rust/loopflow/src/ops/session_import.rs` | 947 | History, except resumable conversation conversion |
| `rust/loopflow/src/store/sqlite/sessions.rs` | 1830 | Mixed: keep current Session owner; delete historical SQL import |
| `rust/loopflow/src/store/sqlite/session_events.rs` | 1127 | Current: provider outcomes and recovery |
| `rust/loopflow/src/session_record.rs` | 4228 | Mixed: current capture/recorder; delete old-layout and historical readers |
| `rust/loopflow/src/engine/flow/saved_skill.rs` | 79 | History compatibility; delete |
| `rust/loopflow/src/store/migrations.rs` | 8216 | Mixed: migration runner and current-state upgrades; delete branch intermediate tests |
| `rust/loopflow/src/journal/mod.rs` | 2080 | Mixed: keep actual Exec recording; delete old event-ledger consumers |

## Remaining affected surfaces

| Item | Lines before | Classification and disposition |
| --- | ---: | --- |
| `docs/architecture-reference.md` | 951 | Documentation: current contract; remove historical preservation obligations |
| `docs/architecture/data.md` | 166 | Documentation: current contract; remove historical preservation obligations |
| `docs/lf-reference.md` | 1286 | Documentation: current contract; remove historical preservation obligations |
| `rust/loopflow/src/engine/flow.rs` | 2226 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/engine/invocation.rs` | 222 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/exec.rs` | 129 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/doctor.rs` | 1084 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/replay.rs` | 272 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/runs.rs` | 355 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/session.rs` | 334 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/top.rs` | 1443 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/commands/util.rs` | 2106 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/lf/mod.rs` | 2951 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/ops/flow.rs` | 607 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/ops/human_session.rs` | 3885 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/ops/mod.rs` | 77 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/ops/task_destination.rs` | 436 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/branch_data.rs` | 627 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/flows.rs` | 257 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/mod.rs` | 3555 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/sessions.rs` | 199 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/sqlite.rs` | 2757 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/sqlite/durable.rs` | 3046 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/sqlite/execs.rs` | 994 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/src/store/sqlite/flows.rs` | 2871 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `rust/loopflow/tests/active_runs_watch.rs` | 189 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/doctor_tests.rs` | 258 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/exec_ownership_tests.rs` | 1070 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/flow_tests.rs` | 2874 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/land_tests.rs` | 2033 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/session_cli_tests.rs` | 742 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/session_cutover_tests.rs` | 4230 | Tests: retain current behavior; remove historical-import expectations |
| `rust/loopflow/tests/store_contention.rs` | 106 | Tests: retain current behavior; remove historical-import expectations |
| `swift/Loopflow/Models/Exec.swift` | 48 | Current: use final owners and encodings; remove retired ledger/import dependencies |
| `tests/fixtures/dto/exec_page.json` | 42 | Tests: retain current behavior; remove historical-import expectations |

## Result and cutover boundary

The 38-draft chain is replaced by `record_execs`, `project_status_chapters` and
`session_ownership`: final tables directly, with current Task graph/cursor/review
conversion. Released migrations are unchanged. Historical import, `run_events`,
`import_evidence`, old manifest/saved-graph codecs, directory discovery and their
tests are deleted. Exec retains the command error formerly held in the ledger;
its unused Wave copy and journal sequence counter are removed. Current native
turn recovery and Session/Flow history remain because retries consume them.

The installed database was backed up through SQLite `mode=ro`. Rehearsals under
`.lf/tmp/deep-compress/` migrated a fresh Home and that database copy. Original
Wave (52), Project (39), Task (273), PR (364), account (6), route (12), limit (12),
provider-session account (2,886), token (5) and browser-binding (7) rows retain
all original columns exactly. Foreign keys pass. The one-machine offline
`convert_current.py` retains 59 resumable Sessions and 20 saved Flows in the final rehearsal; public
Session inventory, Flow inventory and every Flow detail decode. No old turns,
process receipts or imported driver authority were copied. No provider was
started, so this is not native-resume acceptance. The earlier rehearsal had
58 Sessions and 19 Flows: installed filesystem state evolved between reads.
The database backup is consistent, but these live filesystem observations do
not establish one atomic snapshot; quiescence remains necessary for cutover.

The conversion script and copied payloads are private rehearsal artifacts, not
runtime compatibility. Before actual promotion, quiesce the old writers, take a
fresh backup, repeat the current-state conversion and verify its selected
captures/native identities. This checkpoint authorizes no installed-Home write.
The unrelated disposable development Home with an obsolete draft is expendable;
its ledger is not rewritten or adopted. Historical import obligations are removed
per Jack Heart's quoted decision; configured execution acceptance remains.

Final checks: `deep-affected-green.log` passes all 363 affected Rust tests;
`deep-swift-final.log` passes 18 DTO tests. Cargo build, fmt, all-target Clippy
with warnings denied, migration history, architecture and generated-doc freshness
pass. Net **7,884** lines removed against `c7fcd31ec`, including tests and docs.
Earlier failing runs remain evidence; the final affected suite supersedes their
repaired expectations. No full gate, provider acceptance or installation ran.
