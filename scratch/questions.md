# Implementation assumptions

- Linear Initiatives, Projects, and Issues are authoritative for wave projects,
  project definitions/KRs, and tasks. `wave/<wave>/projects/*.md` remains a
  generated offline prompt cache and a one-time migration seed, not an editable
  second source of truth.
- A Loopflow project slug is derived deterministically from the Linear Project
  name. Duplicate derived slugs are a hard drift error because silently choosing
  one would weaken the exactly-one-wave invariant.
- Existing `pm.linear_project` rows are retained only as an explicit migration
  input. `lf pm init` migrates their labeled issues into native Linear Projects,
  writes `pm.linear_initiative`, and removes `pm.linear_project`.
- Project KR state is a human/project-loop judgment stored directly as Markdown
  task checkboxes in Linear Project content. Evidence automation is outside this
  implementation.
