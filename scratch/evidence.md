# Evidence retained for the finish line

LOO-298 · Consolidated 2026-09-28. Results describe their recorded bytes only.
Detailed reports, earlier failures and source hashes remain in the
[archive](parallel-work.md). No new behavioral suite ran for this consolidation.

## Published CI and retained red results

Published `7e2101b41`, base `a2b59ed50`, PR1296. CI **36494751569** failed:
Rust job **109171585133**, 39 passed / 2 failed / 13 skipped / 1,890 unrun.

- `task_decision_driver_failures_open_one_unblock_and_reassess_feedback`:
  SQLite foreign-key failure, `controller/task/mod.rs:1842` at that checkpoint.
- `task_decision_public_resume_preserves_feedback_after_adoption_refusal`:
  “Conversation has no connection and no confirmed engine exit,” line 2044.

Log: `.lf/tmp/execution-model/flow-checkpoint-rust-ci.log`.
Migration, Rust lint, architecture, Python, website, smoke, Task installation,
Swift and UI compile jobs passed. Scratch-clear and aggregate test result failed.
These passes do not close the unrun Rust tests or establish a full gate.

The current focused reducer test first failed on absent Run outcome:
`.lf/tmp/cut-i/flow-native-completion-red.log`, one failed, 1.283 seconds.
A future green assertion must distinguish selected from earlier success, advance
once, reject stale claims/versions and retain unknown command outcome.

**Public driver-loss counterexample:**
`.lf/tmp/execution-model/supervisor-flow-completed-after-driver-1/` contains
results, source hashes and logs; script `supervisor-flow-completed-after-driver.py`
in the parent directory. Copied CLI SHA
`709b1ca131aa7c39f7b26ddf3fea8f5139a42c1122a5dc6a5c02d7f22efabdae`.
Actual Codex 0.157.1, synthetic local Responses, private Home. Kill only fixture
lf process group, retain separately owned provider, release response. Native
`thread/read` observes turn `01a0ea3c-8e9b-7590-943e-1050518b0fde` completed
**before** plain public `flow resume`. Resume exits 1 waiting for old Run end;
SQL history still has Started only. Original Exec outcome/exit remain NULL.
No SQL seeding/mutation created this result; exact fixture engine was cleaned.
This proves the failure after native completion, not merely a still-busy turn.

## Working recovery proof reviewed after consolidation

The supervisor inspected main's test source, raw results and logs. Final copied
candidate `162dd96586ed88470fbf974bc467a91fd2cf8eba19ad92bae525ca9f12be0c92`
passes `native-selected-{running,completed}-final/results.json`: public resume
consumes the exact selected completion once, completes the Flow, retains Session/
thread/generation, and leaves killed Exec outcome/exit NULL. Both check public
history's original Exec/generation. Completed mode observes native completion
before public resume; repeated resume consumes nothing twice. The same candidate
passes `native-selected-{retry,engine-loss}-final` for retry after a recorded
failure. Earlier c718 candidate results remain history, not additional coverage.
These repair the retained driver-loss counterexample within taskless fixtures;
account continuity, managed Task recovery and full owner removal remain unproven.

Cut `flow-selected-history-2.log` passes two checks (selection and busy-turn origin).
Cut `flow-selection-and-decision-fixtures.log` passes five checks, including both
published CI failures, exact selected-turn consumption and final cursor fences.
The selection test rejects earlier/unrelated success, old version/claim and retry
cross-talk, then consumes one exact success while Exec outcome stays unknown.
Cut `canonical-flow-history.log` passes four checks after 23 drafts materialize
as 0.12.25.001_release in a disposable source copy: selected-turn consumption,
both CI regressions and empty-draft initialization/upgrade (3.893s,31.25s compile).
This is not all-kind populated import proof. Cut `flow-history-clippy.log` records
all-target Clippy passing, 17.06s. These local passes do not replace hosted CI.

### Remaining failure: driver and engine both lost

Supervisor probe `supervisor-flow-both-loss-3/results.json` (same parent directory)
fails public `flow resume FLOW --retry`: exit1, Connection refused (os error61).
Candidate SHA `162dd96586ed88470fbf974bc467a91fd2cf8eba19ad92bae525ca9f12be0c92`.
Selected native turn is held; fixture lf is killed, surviving engine identity is
checked, then exact fixture engine is crashed. `ps` confirms it absent before
retry. Flow remains current without a failure; old Exec outcome/exit remain NULL.
No SQL manipulation. Source routes pending selection through old-socket readback
before replacement. Existing stopped-engine proof starts with a completed failure
receipt and does not cover this case. Fix within existing recovery owners, retaining
unknown prior turn/Exec evidence and same native conversation.

Earlier -1/-2 probes stopped at fixture graceful shutdown waiting on held work;
they are not product failures. -3 deliberately injects SIGKILL after exact identity
checks and reaches the advertised retry command. Later assertions inherited from
the surviving-engine fixture must be adapted to generation replacement before a
future green run can establish full recovery. The failing retry assertion itself
is direct CLI evidence. No configured account or installed Home was used.

### Automatic retry: retained failure and repaired public path

`supervisor-flow-automatic-retry-1/{probe.py,results.json,source.json,probe.log}`,
same162dd965 candidate, actual Codex/private Home/synthetic Responses. One public
headless Flow command receives one transient-unavailable response, then success.
The lf retry loop logs attempt1→2, resumed=true, no account failover. Session
history retains failed and successful distinct turns under one native thread and
generation, with successful cumulative usage40/10. The command exits1 waiting
for its selected native completion; Flow history contains only selected seq1,
which names the failed turn, and no consumption. No explicit Flow retry or SQL
mutation. This reproduces the source concern in remaining-work.md; existing
explicit-retry passes do not close it. Preserve both outcomes and select only
the authorized successor, without replacing Session identity or inventing Execs.

Supervisor inspection of `native-selected-automatic-1/results.json` and the
tracked fixture on 2026-09-28 confirms the repaired candidate
`84fb1f7d744de2409c8cedc1153c0fed6eb2c1d043e7875039e84c10ed68f0b3`
exits0 and completes the Flow. One command/Exec, AgentSession, native thread and
engine generation contain failed then successful turns; selection records seq1
then seq3 and consumes only success seq6. Successful cumulative usage40/10 is
retained; the failed response reports no usage. The fixture asserts one engine.
This closes that public counterexample with real Codex and synthetic Responses;
it is not configured-account or full managed-Task acceptance.

### Both-dead recovery: repaired standalone path

Supervisor inspection of `native-selected-both-loss-1/results.json` and the
tracked fixture confirms candidate
`6823ef083d531ae4719bd03d2403bc23c0794246b1c248d8edd8dd4f7384069c`
recovers through public `flow resume --retry` after exact fixture driver and
engine SIGKILL. Session/thread remain, generation1→2, old Exec outcome/exit stay
null. Native readback reports the earlier turn interrupted; the retry's success
is consumed once and repeated resume consumes nothing further. Public history
attributes the earlier interruption to its original Exec/gen1 and success to
the new Exec/gen2. These are real Codex/private-Home/synthetic-Responses results.

The preceding Connection-refused result remains the pre-repair observation.
Current source shares `prepare_native_retry` between explicit Flow retry and
Task resume preparation, before replacement claim acquisition. Managed-Task
policy now has focused evidence in Cut `flow-final-recovery-tests-2.log`: four
tests pass (4.291s), including both prior CI failures, exact native selection and
`managed_flow_retry_releases_only_dead_native_selection_and_consumes_its_successor`.
Supervisor inspected the test: public managed resume prepares the dead selection
then preserves the existing branch-adoption refusal; a manually claimed successor
with synthetic native completion is settled through `drive_task`, preserving
history, Session identity and unknown old Run result. Its owned dead process is
real, provider/claim/completion are simulated. This proves dispatch preparation
and reducer policy, not a full configured managed Codex launch. Final combined-byte
native verification remains due. Earlier compile failures are retained in the
unsuffixed test log; `-2` is the completed passing command.

### Failed-turn decision: reproduced and repaired

Supervisor probe `supervisor-failed-decision-retry-1/{probe.py,source.json,
results.json,probe.log}` runs candidate84fb1f7d (the automatic-retry pass above)
through real Codex, synthetic Responses and a private Home. Resource preflight
passed with73.1GiB free. No SQL mutation, installed-Home access or source build.

The two-step Flow's first decision turn calls public `lf flow decide advance`
successfully, then receives a transient error. The same-command automatic retry
completes without calling decide. The command incorrectly exits0 and the Flow
completes. Six HTTP requests retain exact tool output `Decision recorded`; native
history records failed turn seq7, successor start8 and successor success11.
Flow events select5, select8, consume11. Assertion fails: `Flow consumed a decision
written by a failed native turn`. Fixture cleanup completed without an error.

Source explanation: `select_flow_turn` replaces selected_start while
`record_verdict_in` and `record_route_in` persist cursor candidates under the same
Run. Automatic retry retains that Run, so the failed turn's candidate survives.
Correlate or clear navigation on authorized native-turn replacement; keep history,
same-turn contradictory-choice rejection and stale claim/version checks. Router
leakage has the same source shape but is not yet separately reproduced. The
earlier automatic-retry pass remains valid for an ordinary nondecision skill.

Source now clears the failed candidate during native reselection. Supervisor
inspected the tracked fixture and `native-recovery-decision-{missing,replace}-combined`
results on candidate
`894aded0d8c465a971d4006ed6016e2c3ae38bbd1d6eb175100c7cadd473d701`.
Missing-decision retry exits1 with `requires a decision`, retains the cursor with
no verdict, and consumes only the preceding work step. Replacement-decision retry
changes failed Iterate to successful Advance, exits0 and consumes only the first
work completion and successful retry completion. Both retain completed/failed/
completed native history. These public CLI checks close the original decision
counterexample; they do not separately prove XOR router replacement.

`native-recovery-{automatic,both}-combined` also pass on that identical candidate:
ordinary same-command automatic retry and explicit retry after both processes
die. The both-dead proof retains null old Exec outcome/exit and exactly one
consumption. Its fixture server logs a BrokenPipe after deliberate client/engine
loss; the command completed0 and the final recovery assertions all passed.
Real Codex plus synthetic Responses/private Homes remains the proof boundary.

Final source checks: Cut `flow-navigation-tests.log` passes the four focused
recovery/decision tests (4.347s). Cut `flow-recovery-clippy.log` completes all-target
Clippy (16.78s). Cut `canonical-recovery.log` materializes23 drafts in a disposable
source copy and passes five tests (3.998s,32.38s compile): those four plus empty-
draft initialization/upgrade. `canonical-recovery-source.json` retains exact source
hashes. This is focused materialized-schema proof, not populated all-kind import.

Two additional probes
`supervisor-late-decision-retry-{1,2}` tried a delayed child from the failed turn.
Neither reached the intended late decision write: the first hit zsh background
nice refusal; the second left an empty child log. Both correctly stopped for
missing decision. These are inconclusive fixture results, not another product
failure or proof that late commands are fenced. Their original logs remain.

## Retained local proof

Paths without a directory are under `.lf/tmp/execution-model/`; Cut logs are
under `.lf/tmp/cut-i/`. Native tests use actual Codex with synthetic upstream and
private stores, not configured model/accounts or rendered Desktop.

| Evidence | Observed result and limit |
| --- | --- |
| `native-flow-live-final/results.json`, `native-flow-loss-final/results.json` | Same Session/thread/history across retry. Live generation 1→1, dead engine 1→2; nested child attribution and one engine. Candidate `4ac6ec1310b509c3ba97884902a5a1f5dc7982da6c08ea540b216a68e98835f3`. No driver-loss or account continuity proof. |
| `native-flow-checkpoint/results.json` | Final checkpoint engine-loss retry: failure then success, same thread/Session, old history, one replacement. Candidate `709b1ca…abdae` above. |
| `supervisor-flow-recovered-origin-1/results.json` | Deliberately remove SQL completion after real failed turn, terminate engine, retry. Old failure restored under original Exec/gen1; new success under retry Exec/gen2; idempotent history. Simulated missed receipt, not actual write failure or recovered usage proof. |
| `supervisor-flow-driver-loss-1/results.json` | Earlier killed-driver/held-turn test also failed missing Run completion. Later completed-before-resume result above is stronger; neither is fixed by engine-loss retry. |
| `supervisor-busy-start-regression-2/results.json` | New client receives existing active turn, preserves original Exec/generation and one completion/usage; strict prior candidate failed. A diagnostic timeout with exit 0 is not a pass. Native turn/start can add input to an existing turn. |
| `supervisor-public-approval-1/` | Old client approval rejected; selected client executes one marker; sibling survives. Native A-close/B-connect replay had another sibling client alive; not zero-client approval continuity. |
| `supervisor-pagination-1/` | 104 exact completions across a 100-item page; 103 unknown origins retained; idempotent. Only latest missed turn has 20/5 usage; lifetime 2100/525 is not allocated to missing turns. |
| `supervisor-zero-clients-1/` | Both lf clients and inspector close; marker written before fresh inspector. Reconnect records one completion and 120/30 lifetime usage; old Exec remains interrupted. Marker proves tool execution before inspection, not whole-turn completion before inspection. |
| `public-live-history-2/` | Exact selected-child ancestry after handoff; stale start/steer/interrupt rejected; old headless SIGINT/130 leaves selected/sibling turns alive. |
| Cut `flow-conversation-and-ci.log` | Four CLI checks pass: failed/retried conversation identity/title, ad hoc/replay writable private stores, refusal before provider launch for inaccessible store. |
| Cut `stored-session-graph-{red,green}.log` | Nested stored node 3 was wrongly emitted as structural key 3; mapper now returns 1/fix/1. Green one test covers nested/post-XOR current and prior occurrences. Numeric Rust/Swift conversion remains due. |
| Cut `canonical-engine.log` | Four passes after 22 drafts materialized as 0.12.25.001_release in disposable source: graph, history/fences, copy authority exclusion, initialization/upgrade. Source fingerprint `canonical-engine-source.json`. |
| Cut `flow-native-final-clippy-2.log` | All-target Clippy passed 14.92s; prior test held async environment guard across await. Not a behavioral suite. |
| Cut `shared-stacking.log` | Three passes, 5.840s: existing-root parent selection retains fork/history/publication, held claim exclusion, local rebase. GitHub simulated. |
| Cut `publication-preservation-3.log`, `integrated-publication-env.log` | Acknowledged identity retained after follow-up read/readiness failure; merge intent ordering and independent parent/PR facts preserved. Original PR1296 incident cause remains unknown. |
| H7 integrated and Cut `native-socket-admission-2.log` | Twelve-mutation two-Home matrix and legacy response-loss matrix pass; archived predecessor comparison fixed timestamp precision and passed; authored-content and fresh-status-conflict regressions pass. No configured Linear rotation or remote-work absence proof. |
| Cut `wal-admission.log`; first-Home receipt `loo298-first-home-execs-1dmf_kwt/receipt.json` under OS temp directory | Three focused checks plus three fresh concurrent CLI pairs pass: each command has start/end, two succeeded Execs, zero agent Runs, no warnings. Earlier pair lost start on concurrent WAL negotiation; retained failure remains history. Six commands do not prove arbitrary fleet load. |
| Cut `canonical-materialized-repair.log` and related `canonical-{init-red,init-repair}.log` | Canonical nested-transaction RED repaired; seven materialized tests include CI failures and populated backup. Original receipts in archive. |
| Cut `import-started-and-rebase.log` | Three checks pass, including import/dry-run retaining Started for originally unbound Runs and original ancestry/bind timestamp. Synthetic history; not complete importer proof. |
| Rebase DTO/Swift batch | Corrected obsolete H7 Project-history rejection; ten DTO/history and 47 Swift checks passed, including 101-Session inventory. No rendered pane/draft retention proof. |

## Measurement limits

Old empty-inventory paired CLI measurement: first initialization 3.668→2.370s;
subsequent fresh-process median 2.354→0.704s (four repeats, interleaved order).
Other integrated changes differ, so no isolated causal claim. No dense discovery,
Desktop latency or final-tree performance acceptance follows. Script
`.lf/tmp/measure-schema-cli.py`; exact receipt path and binary hashes in archived
`parallel-performance.md`.

Parsed-command probe `command-observation-cpxkch7g/receipt.json` records succeeded
list Execs and failed rename Exec; help/version/argument errors emitted no Exec.
Early paths 9–11ms, empty listing about 752ms in one diagnostic sample. Admission
coverage remains unfinished. A separate four-test batch had a Nextest LEAK;
passing assertions are not clean process settlement.

Current production count is in [control](parallel-work.md); it excludes working
edits. Preserve historical failed attempts and measured missingness when updating
this ledger. Passing an isolated replacement check never upgrades unchanged gaps.
