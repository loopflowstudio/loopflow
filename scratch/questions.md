# Material assumptions

- 2026-10-09: “No command writes Wave goal or memory text” removes document editing
  commands. Existing PM frontmatter metadata mutations preserve the objective body;
  removing PM binding/Team migration commands is outside LOO-449. Creation may seed
  new files as explicitly required. No stored-document compatibility path remains.
- Relocation remains a registry-address operation; authored directories are not
  silently moved or overwritten. Direct file authoring owns their placement too.
- 2026-10-09: Jack Heart's mocked-sync acceptance requires a missing summary
  writer. The narrow writer is now implemented, without treating a
  creation/config-reader test as acceptance. Branch-versus-main policy stays
  open; the implementation preserves sync's supplied checkout path rather than choosing
  another checkout. Missing GOAL.md is diagnosed before outbound mutation, not
  treated as permission to clear the provider summary.
