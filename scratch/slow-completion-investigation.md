# Slow Desktop completion — October 15 diagnosis

Jack Heart requested diagnosis only after Complete on LOO-312 appeared hung and
then succeeded. The command was short; the duration and cause of any additional
Desktop delay remain unmeasured. A separate, reproducible feedback defect lets
Desktop stay busy indefinitely after command success while awaiting confirmation.
Do not treat that reproduction as proof of the historical bottleneck.

LOO-312 remains unfinished: final read-only SQLite inspection returned
`planning_state=unstarted`, `planning_completed=0`, no completion request/error.
No live completion, reopening, abandonment, publication, installation, demo
restart, or worktree creation was performed by this investigation. Lifecycle
controls at HEAD `52ee931af5` are unchanged. Tests used in-memory fixture Tasks
and a disposable LF_HOME; their command closure never spawned lf.

## Historical measurements

Times below are UTC (09:37–09:38 PDT). Source: read-only queries of
`~/.lf/loopflow.db`, tables `processes`, `task_events`, and `tasks`.

| Evidence | Measured result |
| --- | --- |
| Completion Process `1e9adaa2-8f45-4768-88fb-693b23c7378d` | Bundled demo `lf task complete LOO-312`; started 16:37:23, finished 16:37:24, exit 0, no recorded error |
| Events 69071 / 69072 | Completion requested and committed at 16:37:23 |
| Reopening Process `eff506c9-0991-4e93-add6-6b75238f1be3` | Started 16:38:28, finished 16:38:30, exit 0 |
| Event 69073 | Reopened at 16:38:28; Workflow unchanged |
| Desktop `desktop-cache/timings/reads.ndjson` | One retained successful `task complete` read: **1,199.5 ms** |

Process timestamps have whole-second precision, not exact elapsed durations.
The Desktop journal measures the complete RegistryQuery runner, including launch,
pipe drainage and exit. Its sole retained completion entry agrees with the
Process evidence, but entries omit time, launch identity and Task identity, so
this correlation is not a unique event join.

There is no recorded `wave workflow` Process from 16:36:40 through 16:39:20.
`/tmp/loo462-repo-entry-demo.log` contains 36 untimed AttributeGraph cycle lines;
they cannot establish whether rendering stalled at the click. A unified-log
signpost query for 09:37:18–09:38:05 PDT returned zero records
(`/tmp/loo462-completion-signposts.jsonl`). No historical click, checkpoint send,
checkpoint arrival, or busy-cleared timestamp was recovered.

## Where time can be spent

1. **Command.** `RegistryQuery.swift:325` awaits `task complete`.
   `RegistryQueryLocal.swift` runs it off the main actor and times the entire
   operation. `ops/task.rs:3112,3284` resolves the saved Task, requests completion,
   checks delivery, commits locally, awaits `planning_sync::sync_after_save`, then
   performs eligible cleanup. For Linear, `planning_sync.rs:53` synchronizes
   repository exports, then fields and deliveries. That repository-wide work can
   make other completions slow, but it is included in this command's short total.
   There are no retained per-phase timings to split this 1.2 seconds further.
2. **Confirmation.** `WorkModel.swift:1771` sets `acting`, awaits the command,
   then awaits `confirmWorkUpdate` before clearing the draft. With a live reader,
   confirmation sends a numbered checkpoint, not a broad refresh. Changed Task
   and planning frames alone do not settle it. Rust's `work_watch.rs:959–1095`
   emits the checkpoint after every due selected projection in its pinned
   snapshot. Other projection work can therefore delay confirmation even after
   Task status has arrived. Checkpoints bypass usage throttling; the ordinary
   10-second usage rest is not a mandatory completion delay.
3. **Application and feedback.** Swift consumes frames sequentially. Applying
   revisions may await a Workflow catalog read (`WorkModel.swift:1257`), including
   before finishing checkpoint waiters; this is a possible extra dependency but
   lacks historical support here. `acting` disables lifecycle actions throughout
   both phases. The new `Updating Task…` text improves the old unlabeled spinner,
   but still does not distinguish command execution from observation.

A checkpoint has no request-age deadline. The transport's ten-second silence
check (`LocalLineObservation.swift:206`) is reset by incoming frames; Rust sends
heartbeats every two seconds independently of projection completion. A reader
can therefore remain live while confirmation stays unresolved. This is a
confirmed structural failure mode, not evidence that the historical reader
actually stopped making progress.

## Disposable reproduction

A temporary test extension exercised the actual `WorkModel.completeTask` →
`performTaskAction` → checkpoint path using the existing WorkFeed and DTO fixtures.
The fake command took 150 ms, then a planning frame marked fixture `issue-now`
completed. The test supplied heartbeats while withholding its checkpoint.

| Injected checkpoint delay | Start → checkpoint request | Confirmation wait* | Checkpoint supplied → action idle | Total |
| --- | ---: | ---: | ---: | ---: |
| 0 ms | 153.0 ms | 0.16 ms | 0.16 ms | 161.6 ms |
| 1,200 ms | 157.5 ms | 1,236.3 ms | 0.18 ms | 1,404.1 ms |
| 11,000 ms | 157.6 ms | 11,449.3 ms | 0.09 ms | 11,614.4 ms |

*Measured after the completed planning frame was applied; includes test scheduling.
All three cases observed successful Task status while `acting` remained true.
Only the matching checkpoint cleared it. The no-delay case rules out an inherent
multi-second WorkModel delay in this fixture. These are model timings, not native
rendering or production-scale Rust projection timings. Heartbeats were injected
at the model boundary; the transport timeout behavior was established by source.

Check: `swift test --package-path swift --filter completionPhaseProbe` — all 3 cases PASS; `/tmp/loo462-completion-probe.log`. An initial temporary-test compile error used the wrong suite extension; corrected before the passing run. Production code was never changed, and the temporary extension was removed after execution. Reproduction source/runner remain at `/tmp/loo462-completion-probe.swift` and `/tmp/loo462-run-completion-probe.py`.

## Ruled out and unresolved

- A many-second Linear/completion command, failure/retry, or pipe-drain hang is
  inconsistent with the successful Process and ~1.2-second runner measurement.
- Completion was committed immediately in the recorded second; Workflow arrival
  or continued agent work was not holding up the local status mutation.
- Workflow catalog refresh is a source-level possibility, but no such command
  was recorded around the incident. Do not label it the cause.
- Constant WorkModel latency is contradicted by the immediate-checkpoint fixture.
  A checkpoint delayed by query work, delivery, or frame application remains possible.
- Neither an actual long post-command wait nor a long pre-command UI scheduling
  delay can be quantified from the historical records. The original observation
  supplies no elapsed time. AttributeGraph cycles are not causal proof.

## Smallest proposed repair

Keep the existing lifecycle controls, authoritative completion and checkpoint
contract. In the existing Task action draft, distinguish command execution from
confirmation: after success show **“Task updated; refreshing…”**, not the same
busy label. Bound confirmation by its own request age, independent of heartbeat
liveness. If it expires, retain current readings and report **“Task update
accepted; refresh not confirmed”**; offer observation refresh, never automatically
repeat completion. A refresh error must not imply the mutation failed.

Before changing query/synchronization behavior, add one correlated timing trail
for action entry, CLI return, checkpoint request, checkpoint decode, checkpoint
application and busy-state clearance. Existing generic read timings omit the
identity/timestamps needed to join these phases. Exercise that trail on disposable
fixtures, including delayed projection and delayed application. Only a measured
production-scale bottleneck warrants narrowing projection work or changing
planning delivery; this incident does not justify either yet.

Review finding: the present API conflates successful mutation with successful
observation in one `acting` lifetime. Preserve the observation guarantee while
making those outcomes distinguishable. Exact remaining uncertainty is where,
and for how long, the historical UI waited outside the short CLI invocation.
