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
