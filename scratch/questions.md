# Remaining naming decisions

- October 8: new lf names use bounded request excerpts, then concrete skill,
  Task title or workspace. This is a reversible implementation choice; semantic
  refinement remains possible through generated suggestions, with human names kept.
- Plain native starts/resumes still need a provider integration design. Claude's
  documented title-setting hooks offer a path; Codex documents app-server naming
  but no hook title output. Hook distribution, native human-name precedence and
  live Codex TUI adoption are unresolved. No provider settings were installed.
- Jack's no-account/no-existing-window boundary remains. cmux denied workspace
  creation outside its process origin; no bypass or configuration change occurred.
- Preserve native passthrough: cmux live rename uses its control channel; other
  terminals update on reconnect. A workspace with several Sessions follows the
  latest launch or rename, while each surface keeps its own name.
