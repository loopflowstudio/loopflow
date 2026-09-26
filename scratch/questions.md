# LOO-298 review agenda

2026-09-26. Jack's approved model stands. These are decisions and consequences
to review with the rewritten docs, not a conversational request from this worker.

1. **Naming.** The spec keeps Flow = template and Flow invocation = execution.
   This is the approved default; no CLI-wide rename is proposed.
2. **Taskless Flows.** Current `lf flow <name>` persists invocations with no
   Task. The approved `invocation ⇒ Task ⇒ Wave` rule excludes them. The draft
   spec chooses Task-owned Flow execution and retains taskless Skills/prompts;
   it does not create synthetic Tasks or quietly weaken the invariant. Jack's
   review must settle this user-visible loss before code changes. Existing
   taskless invocations need an explicit Task assignment or a preserved terminal
   disposition before cutover; automatic abandonment is not proposed.
3. **Historical missingness.** Many old Runs have no capture proving whether
   they were independent. Proposed `membership_known` preserves Unknown as a
   stored fact; null invocation alone is not labeled Independent for those rows.
   No runtime fallback to manifests/subjects is retained.
4. **Identity on replacement.** One Session per Run means replacing a published
   Run creates a different Session; the old Session and its human name remain
   historical. Unpublished retries retain their reserved ID. Review this against
   the former Flow/Ask behavior that replaced the Run behind one Session ID.
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
