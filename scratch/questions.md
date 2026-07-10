# Open questions

## Linear project retirement — not part of the trace-capture branch

The Evals task set is empty and its local project file is removed, but the
Linear Project still exists. `lf pm` exposes task move/close and initiative
rename, not project update/archive. Because Linear is authoritative for project
definitions, `lf pm sync --plan` would currently restore `evals.md` and replace
the revised Context/Trace cache files with their older Linear definitions.

Do not bypass `lf pm` with a raw Linear mutation. Finish the retirement by
either adding the missing project update/archive operation to Loopflow or
archiving Evals and updating Context/Trace through an approved Linear surface.
`lf code` must not implement PM project mutation while executing
`scratch/intelligence.md`.
