# Assumptions

- “Record the commit as the Task and PR base” means keep
  `TaskPr.base_commit` as the single durable authority and expose it through the
  active Task projection and placement events. Adding a second `Task.base`
  field would create competing serial-PR state and is intentionally excluded.
- A Task built on unpublished canonical-main code may require that dependency
  to land separately before the Task can integrate cleanly. This change makes
  launch independent of publication; it does not make another Work's commits
  part of the Task PR.
