---
requires: a reported failure or broken workflow
produces: working behavior with verification and retained incident evidence
action_style: procedural
---
Get the broken thing working again.

Use the report already available: conversation, arguments, stdin, clipboard,
logs, or an existing incident note. Ask only for information that is missing
and needed to proceed; no particular input format or Task is required.

## Repair

1. Establish what failed and what working looks like. Inspect the reported
   environment, current state, relevant code and recent changes. Reproduce the
   failure when safe; preserve the original observation when reproduction would
   risk data or service.
2. Separate observations from explanations. Follow useful stack frames and
   logs, state what the leading causes predict, and run the smallest safe check
   that distinguishes them. When evidence contradicts an explanation, revise it
   before making dependent changes.
3. Make the smallest useful repair. Preserve data, existing work and evidence;
   preserve state when repairing a migration failure. A temporary workaround
   can unblock someone, but keep the underlying failure explicit. Avoid unrelated
   refactoring or a broad prevention project during recovery.
4. Replay the original workflow through the reported surface and run the relevant
   regression checks. Follow newly exposed failures until that workflow succeeds.
   Verify against earlier counterexamples as well as the latest result. A green
   unit test alone does not prove the reported machine or release path recovered.

## Leave usable evidence

Update the existing incident note, or write a topic-named note under `scratch/`
when evidence needs to survive a handoff. Retain the symptom, reproduction or
observed failure, supported cause, repair, verification and unresolved questions.
Do not overwrite the working design with an incident template.

Report what works now and the proof. If recovery remains blocked, name the exact
failed check and what is needed next; do not call a workaround or an untested
change recovery. The evidence should support further causal investigation without
requiring this skill to have a following step.
