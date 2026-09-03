# Release transition ownership

## Decision

Release promotion stops and restarts each live Project or Task controller. It
does not allow adjacent controller generations to coexist.

The existing controller authority contract already identifies one safe signal
target: a Work-linked, birth-validated OS Exec owner. Adding coexistence would
require a second generation lease or ownership namespace and would weaken the
one-owner invariant. A fresh local install also changes stores, so allowing the
old process to continue would leave it writing the prior store after the Home
keeper moves to the target store.

## Durable handoff

The machine install switch receipt owns the handoff. Before any controller is
signaled it atomically records every live Project and Task controller as a
`ControllerHandoff` containing its Work, tmux transport name, and exact startup
attempt. The receipt records monotonic outcomes for that handoff:

- `captured`: the prior owner was live when the switch began.
- `quiesced`: that exact owner is positively absent.
- `parked`: the controller reached an intentional human boundary and must not
  be relaunched automatically.
- `restarted`: the target release established a distinct running attempt.

The owner tuple remains in the immutable startup and Exec receipts; the switch
receipt persists only the Work and attempt ids needed to resume the transition.
Run and provider state remain advisory.

Promotion holds the machine-global exclusive promotion lock. Ordinary Work
launches hold the shared side of that lock through their startup outcome, so a
controller cannot appear between capture and quiescence. Recovery uses the
same switch receipt and exact authority query. Before store advancement, a
failed switch restores quiesced controllers through the prior selection. After
advancement, recovery converges every captured controller through the target
selection before settling the switch.

## Forbidden outcomes

- Promotion never kills a Project or Task controller by deterministic tmux
  session name or signals an owner that fails fresh birth validation.
- Missing or contradictory ownership never becomes process absence.
- A controller absent before promotion is not started as a side effect.
- A parked controller is not restarted automatically.
- A switch cannot settle while a captured controller lacks a durable quiesced,
  parked, or restarted outcome.
- Provider, Run, trace recency, or tmux presence cannot add or remove a handoff.

## This slice

Persist the release controller handoff in `SwitchReceipt`, serialize ordinary
Work launch against promotion, quiesce exact captured owners before target-store
advance, restore them on a pre-advance rollback, and restart them through the
target execution context before settlement. Route switch recovery through the
same convergence functions.

Done when a production-shaped local promotion with a live Task records one
captured prior attempt, proves it absent before store advance, creates one
distinct target attempt, and settles only after that owner is live. Forced
missing Exec identity or an unowned tmux transport must reject promotion before
advance and signal no process. A forced interruption after quiescence must
recover from the receipt and restart exactly the captured Work.

## Review

Reviewed on 2026-09-02 against the complete Task diff, the public Project and
Task controller path, and both machine-managed promotion paths. The behavioral
proof is an isolated production-shaped local promotion with real public `lf`
commands, SQLite stores, startup/Exec receipts, OS processes, and switch
receipts. It does not mutate the configured machine installation.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|-------|------------------|----------------------|-------|--------|
| Live controller handoff | Capture one exact prior attempt, prove it absent before advance, and settle only after a distinct target attempt is live | The switch receipt advances `captured -> quiesced -> restarted`; each transition is persisted around the same authority query | Public Project/Task controller integration test; settled switch and startup/Exec receipts | Pass |
| Missing ownership fails closed | Missing Exec identity or an unowned controller tmux transport rejects before advance and signals no process | Capture accepts only `Live`, ignores positive absence/parking, and returns every `Unverifiable` result without mutation | Public promotion probes remove the Exec receipt and substitute an unrelated pane PID; both owners remain live | Pass |
| Recovery is receipt-driven | An interruption after quiescence restores exactly the captured Work and remains idempotent if recovery exits after the fresh prior attempt starts | Normal launches are fenced while a switch receipt exists; receipt-scoped recovery alone bypasses the shared lock and accepts its already-restored exact owner | Two forced public-command interruptions followed by recovery; only one restored attempt remains live | Pass |
| Receipt evidence is monotonic | A terminal parked or restarted outcome must keep naming the attempt that established it | Receipt validation rejects an empty parked attempt and any rewrite of a terminal handoff attempt | `machine_install::tests::controller_handoff_*` | Pass |
| Lock bypass is receipt-scoped | Only the active switch recovery may launch a replacement while promotion holds the exclusive lock | The handoff marker selects the operation; the separate switch capability must name the active receipt and prove exclusive ownership | Source inspection plus the public target-restart path | Pass |
| Store ownership follows the handoff | A fresh target restarts from the store the prior owner was using | Fresh promotion clones `prior.selection.store`, the same store used for capture and quiescence | A post-snapshot Task steer survives in the target store and the target controller starts from it | Pass |
| Project and Task execution still advances | Existing controller startup, disagreement, stop, resume, and phase behavior remains intact | Promotion reuses the shared authority and startup contracts without adding a second owner model | `cargo test -p loopflow --test controller_startup_tests public_project_and_task_controllers_prove_startup_and_resume -- --test-threads=1` | Pass |

Negative source search found two production `launch_work` callers, one startup
receipt writer, one controller-authority query, and one switch handoff
collection. No Project or Task liveness reader, input fallback, deterministic
controller tmux kill, or promotion-side provider/Run authority remains. The
only remaining tmux-presence liveness check is the pre-outcome startup watchdog;
Home keeper replacement retains its separate service contract.
