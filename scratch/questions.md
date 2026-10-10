# Material assumptions

- 2026-10-09: “No command writes Wave goal or memory text” removes document editing
  commands. Existing PM frontmatter metadata mutations preserve the objective body;
  removing PM binding/Team migration commands is outside LOO-449. Creation may seed
  new files as explicitly required. No stored-document compatibility path remains.
- Relocation remains a registry-address operation; authored directories are not
  silently moved or overwritten. Direct file authoring owns their placement too.
- 2026-10-10: Jack Heart resolved branch/main selection: Linear sync publishes
  only the committed default-branch goal. The reader uses the local default-branch
  ref selected by existing origin/HEAD discovery (main when unset), without fetching.
  Missing refs/files fail rather than falling back to the checkout or stored text.
