# LOO-298 review agenda

2026-09-26. Interactive review amends Jack's earlier model. The current
participant's name is unresolved. Approval covers the changes recorded below;
implementation assumptions are distinguished from explicit confirmation.

1. **Naming — confirmed.** Flow = template, Invocation = execution; use
   “Flow invocation” in full. No CLI-wide rename is proposed.
2. **Taskless Flows — resolved in interactive review.** The current participant
   (name unresolved) explicitly requires Flows to work without a Task.
   Invocation Task is nullable; taskless and Task-owned execution use the same
   SQLite records and driver. Existing taskless invocations import with their
   captures, progress and review Sessions intact, without synthetic Tasks.
   `Task ⇒ Wave` remains; `invocation ⇒ Task` is removed. The existing nullable
   parent equality still constrains binding an invocation Run; whether bind may
   attach a taskless invocation's individual Run to a Task remains for review.
3. **Historical missingness.** Many old Runs have no capture proving whether
   they were independent. Proposed `membership_known` preserves Unknown as a
   stored fact; null invocation alone is not labeled Independent for those rows.
   No runtime fallback to manifests/subjects is retained.
4. **Identity on replacement — confirmed.** Session owns Runs and a current
   Run. Stable Session ID, title and feedback survive Run replacement; prior
   Runs remain history. `runs.session_id` supplies the history and
   `sessions.current_run_id` selects a member. Validate that relationship and
   change the pointer with the exact pending attempt atomically.
   Cross-Run bind scope was not explicitly decided: the working assumption is
   current Run ancestry, inherited by replacement, with no bulk history rewrite.
5. **Operational consequences.** Every launch now requires a writable local Run
   store; bind of an invocation Run cannot change Task; moving the last Run away
   can change displayed Started but never authorize retirement by itself. These
   follow the chosen owners, and the doc spec states them explicitly.

Implementation assumptions: preserve manifest/context/terminal evidence and
provider-native history; “every sidecar goes” concerns mutable product and
attachment state. Real-Home conversion requires a separate maintenance invocation
after quiescing exact old writers, with backup/rehearsal first. The present slice
neither inventories private Home contents nor changes them.

Integration finding: this checkout predates several LOO-291 features and already
dropped the former `runs` table. Reconcile actual source at implementation time;
do not reapply the interrupted `d0-docs-partial.patch` over the completed doc edits.

## First implementation cut (2026-09-26)

The selected implement step follows the supplied completed-review feedback.
The four Task step projection columns (`flow`, `step`, `node_id`, `human`)
can be removed without introducing a second owner. Root `step_index` and
`iteration` cannot yet go: historical flat review records still need them.
Their conversion stays with the complete invocation migration.

Review discovery now decodes each current Task position and selects the captured
human policy, including nested XOR paths. This is a bounded intermediate reader
over current Task positions, not the final indexed Session inventory and not a
latency acceptance claim. A malformed autonomous capture is now surfaced by
review discovery rather than skipped by the former SQL `human=1` filter.

Verification observation: `scripts/resource_envelope.py` and its safe `--recover`
pass both report the active `main-view-task` checkout at 14.5 GiB against its
12 GiB budget, while the disk has 100 GiB free. Recovery correctly leaves the
active checkout alone. TESTING.md stops product tests on unresolved pressure;
this worker does not delete another contribution's active build output. Static
checks continue. Behavioral tests remain required once that resource condition
clears; no real Home import or installed binary promotion is authorized by this
local proof.
