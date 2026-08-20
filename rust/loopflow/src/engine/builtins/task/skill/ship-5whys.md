---
description: Implement the next coherent prevention from an incident's 5 Whys.
action_style: procedural
capabilities: [task_implementation]
---
Ship one prevention from the incident analysis.

Read `scratch/<branch>.md`. Find the highest-leverage unchecked change whose
dependencies are satisfied. Verify that its causal claim is supported by the
recorded incident evidence; revise the analysis if new evidence contradicts it.

Implement the smallest coherent slice across the layer where the cause lives:
code, migration, tests, process, prompt, or documentation. Add the focused proof
that would have caught or prevented the incident. Mark only demonstrated fixes
complete in the analysis.

Publish or refresh the Task PR when the slice is reviewable. Never land it here.
Return the shipped evidence and the next unchecked prevention; when none remain,
say so explicitly so the final flow can gate the whole incident response.
