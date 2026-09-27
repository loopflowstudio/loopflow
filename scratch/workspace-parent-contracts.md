# LOO-298 integration contract assessment

2026-09-27, 09:04–09:06 UTC · LOO-303 · implement

**Parent conversion remains incomplete.** Do not start the dependent attempt or
room/bind implementation yet. The published parent already contains ordered
Task-attempt storage, and a newer local commit adds table-owned interactive
Sessions. Neither supplies the complete shared consumer contract. This corrects
the earlier local-only observation that Run has no position fields; it does not
release the integration boundary in the [approved design](workspace-ux-on-data-model.md).

## Available sources and integration route

- LOO-303 started at `84d42b5e71ce82dd870855876690aaab62490c9c`. Preserved the
  supplied concept review with `lf commit` at
  `33d582f97000d91a06805339e08c652d18f60e86` before editing these notes.
- `lf status infrastructure --json` successfully resolves LOO-298 as
  `task_9756ded2ad49449090003636171df943`, with active PR
  `pr_64160469a52645eaafa913a5e024d2db`, GitHub #1296, phase `open`, and no
  recorded merge. Its publication names head
  `ca1be1116d704a4d908d473c2434ef59fd23261f`. The PR is now recorded on the
  Task; the original missing-PR-row incident is not the present dependency.
  The read reports uncommitted work and failed required checks. These are shared
  observations, not a claim that the parent is idle or permission to operate it.
- A fresh read-only `git ls-remote origin
  refs/heads/jack-heart/data-model-one-table-per` returns that same
  `ca1be1116d704a4d908d473c2434ef59fd23261f`. The local tracking ref matches.
- The registered parent checkout is
  `/Users/jack/src/loopflow.data-model-one-table-per`; its committed branch tip
  is `d814eb61775ed322fb7e459e1f2aba0daf9665e5`. Source citations below refer
  to this immutable revision unless stated otherwise. Its latest cut is
  interactive Session ownership, not the complete conversion.
- The parent also has uncommitted Ask-conversion edits. A bounded read-only
  snapshot at 09:05:45 UTC shows `open_conversations()` replacing the Ask list,
  but still calls `flow_session::list()`. Its Session command still lacks Bind;
  public Session/Task Flow shapes still lack attempt history. Six relevant files
  were copied and hashed, each unchanged during its read. This is unfinished
  contribution evidence, not an atomic tested revision or an integration source.
- `lf rebase --plan` selects `origin/jack-heart/data-model-one-table-per`, fork
  base `c832aaede633a8996bf694dfb7ae2579dbabce3a`, class `clean_authored`,
  strategy `direct_rebase`, 15 unique commits and 38 changed files. No rebase
  was applied: the required contracts are absent even in the newer local source.
  Once the conversion is available, reassess that revision and integrate through
  `lf rebase`; preserve the parent's work and land it before this PR.

Raw state, 27 pinned source/fixture files and their hashes, and the six-file
working snapshot receipt are in `.lf/tmp/parent-contract-assessment/`. The
committed revisions and findings here remain sufficient to reproduce the audit
if that disposable evidence is cleared. No parent checkout was modified.

## Contract checklist

| Required contract | Present evidence | Missing before dependent implementation |
| --- | --- | --- |
| All-kind stable Session/current Run | `session.rs:9` owns Session identity/current Run; `runs.session_id` supplies history. Task reviews and, at local tip, interactive Sessions use rows. `human_session.rs:1387,1817` publish those stable IDs. | `human_session.rs:575` still unions Task reviews, Ask files, taskless Flow files and interactive rows. Ask writes remain at `:2262`; taskless persistence remains separate. The dirty Ask cut does not finish taskless ownership. No all-kind replacement/import proof is established here. |
| Typed Task/Wave/repository ancestry | `session.rs:37` has typed optional Task/Wave; `store/sqlite/runs.rs:76` infers parents and rejects mismatches transactionally. Interactive launch now uses that writer. | Public `SessionRecord` at `human_session.rs:213` still has Work/path and no canonical repository field or member-Run history. `Run` has cwd but no repository field; `runs --task` still selects manifest evidence (`lf/commands/runs.rs:77`). Final all-consumer ancestry/read contract is absent. |
| Historical Task destination | LOO-303's existing exact roadmap reader and tests at `status_tests.rs:888,957` cover retained/completed Tasks, missing active PR, repository collisions and no-start inspection. Prior executed receipts are in the navigation review. | Preserve this child-owned extension through integration. The parent's ordinary current roadmap and historical Run-to-Session lookup do not substitute for it. No repeat or new installed-link acceptance claimed. |
| Confirmed write-once bind fenced to selected Run | `record_task_first_run` SQL draft rejects Task movement/clearing and incompatible Wave changes. Existing rename/completion operations have expected-Run fences. | `SessionCommand` (`lf/mod.rs:733`) and `SessionActionKind` (`human_session.rs:121`) have no Bind. No exact-target confirmation transport or bind transaction is delivered. Storage UPDATE constraints are not that operation. |
| Shared Started | `record_task_first_run` draft sets `tasks.started_at` once on insertion/first assignment, including reservations. | `store/sqlite/chapters.rs:96` still combines the column with events/worker generations. `lf/commands/waves.rs:1090` additionally counts PR evidence; `lf/commands/run.rs:912` still writes Started events. General launch/import conversion and column-only consumer behavior remain incomplete. |
| Ordered/current position attempts | Published parent already has `Run.node/iterations/attempt`; `runs.rs:144` allocates ordinals inside the transaction; `:196` selects the current attempt under the invocation version fence; `:234` reads ordered history. `record_invocation_attempts` stores `flow_invocations.current_run_id`. | `PinnedTaskFlow` (`ops/task_flow.rs:35`) and `SessionFlowMembership` (`human_session.rs:242`) expose no attempt list/ordinal/current-position selection. `TaskExecutionSnapshot.run_id` exists, but derives from worker claim or pending review, not a historical position projection. `Run` lacks outcome/end/usage fields needed by the complete history view. `Store::position_runs` currently feeds tests, not a public consumer. |
| Retained invocations and runtime children | `retain_flow_invocations` retains Task capture/cursor/claims on completion/replacement; a partial index chooses one current Task root. The headless attempt test retains both Runs after completion. | `task_execution.rs:42` reads only the current position; after completion it returns `Finished { flow }` from events, without the retained graph. No public retained-invocation/child-history projection is delivered. Runtime loop-child ownership is still absent; template composition must not be used to invent it. |
| Taskless legality and common owner | `ops/flow_run.rs:33` permits optional Task/Wave. SQL Run construction enforces nullable invocation/Task equality. | Taskless execution still reads/writes `flows/<id>/position.json` (`flow_run.rs:108,136`) and separate review state. Nullable SQL columns do not establish common driver/Session ownership or taskless selected-attempt recovery. Preserve these conversations and future non-bindable orphan explanations. |

## Fixtures and proof limits

Compared the exact blobs for `sessions.json`, `session.json`,
`session_memberships.json`, `task_flow.json` and `task_execution.json` between
published `ca1be1116` and local `d814eb617`: all five are identical. Parsed them
successfully. Session fixtures retain `work`, `wave_id`, `work_path` and `run_id`;
Flow fixtures retain graph/cursor/returns. None contains attempt ordinals,
ordered attempt history or the new current-position projection. The existing
Task execution fixture does contain `run_id`; it is not ignored by this audit.
Rust and Swift declarations agree that the richer public contract is absent.

Inspected the parent's
`headless_position_retains_attempts_and_rejects_the_replaced_run`
(`store/sqlite/durable.rs:2931`): it reserves A and B, rejects stale A decisions
and settlement, checks ordinals 1/2, and retains both after completion. The
parent slice review reports 49 distinct passing Task-controller/durable tests
and a canonical historical-cursor repeat. Its interactive cut receipt reports
two actual CLI tests with shell providers (`session_cutover_tests.rs:209,314`),
plus neighboring checks. These are prior parent receipts, not tests executed
here, and do not prove all-kind ownership or installed providers. The interactive
receipt explicitly says old file-only Sessions no longer list until import;
that cut must not be activated as a completed Home conversion.

This assessment changes Markdown only. No product tests, builds, resource
recovery, native rendering or installation were needed. Reuse the child's
unchanged nine-test keyboard/return and Xcode compile receipts only within their
original scope. The inherited `wave_chapters` architecture-map failure remains
open; no fresh architecture pass is claimed.

Documentation verification passes: working-diff whitespace, the new note's
local links, all eight checklist rows, and SHA-256 verification of the 27
retained source/fixture files. These checks validate this assessment only.

## Next implementation boundary

The next useful trigger is a completed parent owner/consumer conversion with
its source and fixtures, not another return-label repair. Reassess the checklist
on that source, integrate through `lf rebase`, then extend the shared attempt
projection where still needed. The first UI proof remains failed A → current B
at one position with independent Task Run C concurrently active: node detail,
running line and Session chip all name B as attempt 2; selecting A retains A.
No Swift ordinal inference, copied parent storage or decorative room is selected.

Keep all eight complete-design obligations: loop/restart/child and unavailable
capture history; every null-Task orphan, including Wave-only/shared-shell cases,
with one mount; universal permanent-target confirmation and every bind race;
installed cold/warm links and keyboard behavior; configured providers/drafts;
live captures at both widths and Jack's verdict; final deletion/docs. Preserve
light-only/no-teardown, the sidebar Session shortcut and non-Task monitoring.

Source review distinguished storage, public read projection and configured
acceptance rather than treating one as another. No remaining live reader was
deleted. This pass selects no Flow edge and performs no publication, Task
completion, external message or live migration. No user decision is required
to resolve the dependency already owned by LOO-298.
