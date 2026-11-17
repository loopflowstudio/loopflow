# LOO-925: the tracker mirrors the store

Status: built. Concept review round 2 (lost vs open, `--keep-theirs` /
`--use-mine`, `tracker_actions.tracker`) is built; see `scratch/questions.md`.
Open choices there keep today's behavior and do not block publishing.
Concept review 3's gap (answering Project and Git lines) is built.
The design as built is `docs/dev/planning.md` ("Words", "One outbound queue").

## Decision: any tracker — Linear and Asana, Notion deleted

The store is the plan. A tracker carries the store's changes out and its
coworkers' changes in. Linear and Asana are the two trackers on the queue;
Notion is deleted. Git is a tracker in the glossary but not on the queue: it
exchanges the store's own WorkEdit journal between peers.

Basis, recoverable only from here:

- LOO-691's unmerged branch `loo-691-combined` (e8ffd80ca0, 2026-11-04)
  records in `scratch/design.md`: "Jack Heart, 2026-11-02: drop Notion in this
  diff; regressions on Linear, Asana and Git planning; feature and
  architectural parity between the three (one interface at one layer, no
  Linear escape hatch or Linear-named shared types)." That is the design
  doc's wording, not a transcript. Its Notion delete (cd7afe0127) never
  reached main.
- Asana was rebuilt 2026-10-22 (#1607) after an earlier delete (c113ef04b4,
  2026-07-02). The README promises "whatever … tracker you already use".
- Run observation, 2026-11-16, `~/.lf/loopflow.db`: every `pm_items`
  (1,246) and `pm_projects` (57) row is `linear`; all 118 `.lf/config.yaml`
  planning blocks under `~/src` are Linear. Making `provider` truthful needed
  no data repair.
- Live store backlog before this branch, 2026-11-16: `task_changes` 503
  acknowledged, 167 attempted, 86 pending; 1,064 `task_state_deliveries`;
  2,656 comment deliveries. The 167 attempted-and-never-confirmed edits were
  the lost-reply rule failing quietly. Migration dry run on that store: 77
  actions, 0 sendable old rows.

## Demo

```bash
export LF_HOME=$(mktemp -d)        # branch lf, Linear stand-in (test fixture) or a sandbox Team
lf task create --title "Mirror demo" --json
lf task edit <id> --title "Mirror demo, renamed"
lf steer <id> "first comment"
lf task complete <id>
lf repo plan --json                # open actions in seq order: kind, change, phase, tracker
lf task show <id> --json | jq .sync.actions   # the same rows, this Task's slice
lf task sync <id>                  # looks now; on Git exchanges the ref
```

Switch `.lf/config.yaml` to `provider: asana` against the Asana stand-in and
run the same commands: the same rows with `tracker: asana`. Pull the
network: every command saves, `lf repo plan` lists the actions as queued with
the tracker's error, and the next connected command drains one bounded pass.
QA run: `scratch/qa.html`.

## Answering a lost line wherever it shows (concept review 3, built)

The answer belongs to the line, not the Task. `SqliteStore::answer_lost(ids,
answer)` answers action ids of any owner (a `tracker_actions` row, else a Git
`planning_peer_changes` head). `lf repo plan answer <id> --keep-theirs |
--use-mine` (served `plan.answer`, `ops::planning_sync::answer_lost`) resolves
the 8-character prefix `lf repo plan` prints among the repository's lost
lines, then runs `sync_after_save` for the owner. `lf task sync <task>
--keep-theirs | --use-mine` stays the bulk answer (Jack Heart, round 2: "2a")
over its slice's lost ids; `--action` and `task.sync`'s `action` are gone.
Desktop: `ServedWrite.planAnswer`; `TrackerSyncView(sync:, repo:)` reads an
`answerLost` environment action that `answeringLostLines(model)` installs in
`SessionsView` and `WavesView`, so Task and Project lines all get the buttons.

## Choices built that the code does not explain

- **One enqueue point, by origin.** `planning_write::WriteOrigin::{Local,
  Tracker, Import}`; only `Local` queues, so provider facts (removal stamp,
  adopted state, binding attach) never echo back out.
- **One conflict rule for drain and acceptance.** `planning::rules::decide`
  lives in storage; acceptance (`tracker_actions::reconcile_in`) calls it.
  The old acceptance copy (`observe_attempts`) was deleted, not ported.
- **Per-request deadline, pass bound by requests** (20 requests / 5 s,
  concurrency 4). A shared deadline once let a slow first creation starve
  every later Task; `sync_after_save` drains its own owner first.
- **#1829 stall fix kept:** no freshness check inside a write; one
  `ListTeams` per process; a lost reply is looked for once per
  `max(600 s, delta_fresh_secs())` (`looked_at`).
- **`pm_issue_changes` stays.** Dropping the launch read's `changes.removed`
  failed `task_admission_reads_released_evidence_without_reconstructing_provider_content`:
  released stores hold removals without a stamped Task.
- **Migration window risk, accepted.** An older lf still running after
  install writes its own new edits to the old tables and delivers them
  itself; if it exits first they stay unsent. Its acquisition can adopt a
  coworker's value over an edit that exists only in `tracker_actions`; the
  new drain then sees the base moved and conflicts, keeping the losing value.

## Remaining work, filed

- LOO-941: contract migration ≥ 30 days after merge — drop `task_changes`,
  `project_changes`, `task_state_deliveries`, `task_comment_deliveries`, the
  creation export columns, `pm_issue_changes`; rename
  `task_linear_observations`, `linear_team_checks`, `~/.lf/chapter-locks/`.
- LOO-942: wire `linear_id` → a neutral name (opens with the question in
  `questions.md`).
- LOO-944: GOAL.md name/summary → Initiative as Wave `field` actions (never
  sent by a released lf; no regression).
- LOO-955: codebase-wide vocabulary (older "settled", "pending effects",
  observation/acquisition prose).

## Forbidden outcomes

- New code reading or writing an old receipt table.
- A `Linear*` type, GraphQL key or `match` on provider outside `linear/`,
  `asana/` and `ops/tracker.rs`.
- A drain without a request bound; Notion code kept "for later".

## Done when

- `grep -rn 'teamId\|stateId\|assigneeId' rust/loopflow-storage/src | grep -v migrations` is empty.
- `grep -rln -i notion rust/*/src --include=*.rs | grep -v migrations` is empty.
- A pass with 100 queued actions sends ≤ 20 requests; with none, 0
  (`ops::tracker::drain::tests`).

## Checks

- Compress 2: `cargo nextest run -p loopflow-storage -p loopflow-execution --lib -E 'test(tracker_actions) | test(planning::rules) | test(ops::tracker::) | test(asana_planning_tests) | test(planning_action_tests::) | test(planning_order_tests::) | test(task_comments_tests::) | test(linear::actions::) | test(planning_lookup_tests::) | test(task_planning_tests::) | test(local_planning)'` → 81/82; the failure, `native_runner_is_quiet_until_its_turn_ends_codex_foreground`, is sccache failing `rustc -vV` in a fixture build (environment, not this diff). Clippy storage+execution `--all-targets -D warnings` clean. Gate reruns that test.
- Vocabulary pass: storage+execution nextest over the tracker suites → 209 pass; `-p loopflow` schema, local_planning, pr, lookup, status, task_abandonment, documented_commands, subscribe → 105 pass; server schema regenerated.
- `cargo nextest run -p loopflow --test task_pr_authority_tests` → 12 pass.
- `EXPLAIN QUERY PLAN`, store seeded to 10,001 Tasks / 9,508 actions (84 unconfirmed): `uncreated()` searches `tasks` by `external_issue_id` and actions by index; the drain select reads the partial index. Per-row `EXISTS` (1 ms) replaced a repository scan (5 ms).
- Concept-review round (one tracker-action view, Git conflicts, `lf task sync` on Git, no `--retry-updates`): execution `store::planning_sync_tests:: | ops::tracker:: | task_sync | tracker_actions` → 16 pass; `-p loopflow` local_planning, status, planning_setup, dto_fixtures, subscribe, `lf::tests::`, documented_commands, schema → pass, except two `subscribe::` watches that timed out in the 120-test run and pass alone (`a_store_created_after_the_watch_started_is_shown`, `a_paused_watch_and_a_truncated_log_both_catch_up`; load-timed, gate watches them); storage `query_plans | tracker_actions | migration` → 157 pass; `scripts/test_desktop.sh --filter "DTOFixtureTests|DesktopHeadlessTests|WorkModelTests|RegistryQueryTests|WaveLensTests|TaskRowMetadataTests|WaveStandupTests"` → 170 pass; clippy clean. `lf::tests::native_login_controls…` listed `notion` after Notion's delete; fixed.
- Concept review follow-up (2026-11-17, 37f38c3537, block moved before the send in 87c9476afe): a tracker block (Linear Project with no Initiative) costs the pass nothing and stays queued (`a_blocked_creation_costs_the_pass_nothing`: 21 blocked Projects + one edit → 1 request; fails with the request charged); `ActionOwnerKind`/`TrackerKind` enums in Rust, schema and Swift; `TrackerReceipts` → `TrackerActions`. Storage+execution tracker suites → 45 pass; `-p loopflow` `dto_fixtures:: | server::schema::tests:: | local_planning | planning_status | repo_plan | task_sync` → 93 pass; `scripts/test_desktop.sh --filter 'DTOFixtureTests|DesktopHeadlessTests'` → 73 pass; clippy clean.
- Not run: `binary(two_people)`, `test(transport::)`, `binary(golden_prompt)`. Gate: `scripts/check_migrations.py`; CI runs the suites.
- Tracker::blocked, then compress 3 (one Project→Wave lookup in `linear/actions.rs::project_wave`, fake hook named `blocked`): `cargo nextest run -p loopflow-execution --lib -E 'test(ops::tracker::) | test(linear::actions::)'` → 12 pass; clippy execution clean. `documented_commands::` (planning.md prose) left to gate.
- QA 3 (247d2917fd, `scratch/qa.html`): conflict line says `lost:` (a Git loser may be a peer's head); `lf task sync` of an unfetched Task exchanges the repository's planning first. `two_people::a_peer_syncs_a_task_it_has_never_fetched` passes and fails with the lookup reverted; `store::planning_sync_tests:: | task_sync` → 5, `dto_fixtures::` → 52, Swift `DTOFixtureTests` → 57 pass; clippy clean.
- Concept review round 2 (lost vs open, answers, `tracker` column): execution `test(task_sync) | test(lost) | test(planning_sync_tests::)` → 18 pass (event close fails with `retire_changed_again` removed); `-p loopflow -p loopflow-storage` `lf::tests:: | server::schema | write::tests | dto_fixtures | repo_plan | planning_status | task_sync | planning_lookup_tests | tracker_actions | query_plans | documented_commands | planning::rules` → 135 pass, `task_status_reads_projectless_planning_without_allocating_execution` timed out at 300 s in the batch and passes alone in 234 s (Machine under load; gate watches it); schema regenerated; Swift `DTOFixtureTests|DesktopHeadlessTests|RegistryQueryTests|ServerProtocolTests|WorkModelTests` → 161 pass; clippy clean. `served_reads` (new `task.sync` arms) left to gate.
- Merge of #1960 (f5a6e4b686): `WorkSurfaceView` called the now-synchronous `answerLost` with `await`; fixed in ae0b2f7dcc. Desktop build not run; gate builds it.
- Concept review 3 build: execution `store::planning_sync_tests:: | task_planning_tests:: | planning_action_tests::` → 28/29; new `a_project_name_linear_won_is_answered_from_its_line` and `a_git_project_head_that_lost_is_answered_with_a_merge` (fail on the Task-only `answer_lost_values`); the failure is the known sccache `codex_foreground` fixture. `-p loopflow` `lf::tests:: | server::schema::tests:: | write::tests:: | documented_commands::` → 54 pass; schema regenerated; `scripts/test_desktop.sh --filter 'DesktopHeadlessTests|RegistryQueryTests'` → 37 pass; clippy storage+execution+loopflow clean. `served_reads` (new `plan.answer` arm) and the Xcode Desktop build left to gate; `lf repo plan`'s short-id text has no test of its own.
- Compress 4: `answer_lost` takes the lines its callers already read and dispatches by `tracker` (no per-id table probes; a line answered meanwhile is left alone); Swift `ServedWrite.taskSync` deleted (no Desktop sender). Execution `store::planning_sync_tests:: | task_planning_tests:: | planning_action_tests::` → 28/29 (known sccache `codex_foreground`); clippy storage+execution+loopflow clean; `scripts/test_desktop.sh --filter 'RegistryQueryTests|DesktopHeadlessTests'` → 37 pass.
- QA 4 (b8c7c7b1af, `scratch/qa.html`): two Git peers; Task and Project lost lines answered with `lf repo plan answer` (prefix and full id), `lf task sync --keep-theirs`, and served `plan.answer`; both peers converge. `tracker_actions.sql` on a copy of this Machine's store → 80 actions, all `linear`. Nothing fixed; findings listed in the report.
