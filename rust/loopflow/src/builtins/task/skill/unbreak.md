---
requires: a reported failure that is blocking someone now
produces: working behavior for the people affected, with the failure kept visible
action_style: procedural
---
Get the broken thing working again for the people it is blocking.

Restoring comes first; finding the cause comes after. A workaround, rollback or
config change is a good result when it restores the workflow safely.

1. Establish what failed, for whom, and what working looks like. Inspect the
   reported environment and current state. Save the original observation
   and any data at risk before you change anything.
2. Make the smallest safe change that restores the workflow. Avoid refactoring
   or prevention work here.
3. Use the restored workflow through the path the reporter uses, and confirm it
   works for them. A green unit test alone does not show it works.

Report what works now, what you changed, and what is still wrong underneath.
Keep the symptom, reproduction and your change in the existing incident note,
or a topic-named note under `scratch/`, so the cause can be found next. If
recovery is still blocked, name the exact failed check and what is needed.
