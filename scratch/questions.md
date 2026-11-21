# Open choices, made for review

- **No second-class viewer.** The brief says "Consider an optional passive /
  second-class viewer flag". The plan has one rule instead: an attach says
  whether a person is at it (`focused`), and focus or input takes the size
  from anyone. A Dev pane someone types into resizes the Session.
- **Release Desktop keeps warm conversations**; only Dev stops opening
  unvisited Work. Removing warmth everywhere would also drop saved-tab restore
  and worktree retention by open views.
- **Dev's restored selection waits for a click.** At launch Dev shows the
  placeholder for the selected Work and starts no terminal until a deliberate
  visit.
- **A relay whose stdin is not a terminal attaches unfocused.** A piped or
  headless `lf session resume` no longer takes the size.
- **Old holders stay exposed to release Desktop's warm attaches** until their
  Sessions restart; nothing migrates or restarts them.
