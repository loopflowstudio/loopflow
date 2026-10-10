# Material assumptions

- 2026-10-09: “No command writes Wave goal or memory text” removes document editing
  commands. Existing PM frontmatter metadata mutations preserve the objective body;
  removing PM binding/Team migration commands is outside LOO-449. Creation may seed
  new files as explicitly required. No stored-document compatibility path remains.
- Relocation remains a registry-address operation; authored directories are not
  silently moved or overwritten. Direct file authoring owns their placement too.
- 2026-10-09: Jack Heart requested a test proving sync sends the edited summary,
  but `pm_sync_async` has no summary writer; only Initiative creation sends it.
  Adding that behavior needs reconciliation with the narrow test-only scope.
  Branch-versus-main selection remains explicitly open, not chosen here.
