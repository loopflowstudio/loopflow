# Open demo choices

- October 8: Jack's no-provider-account constraint and cmux's process-origin
  access policy leave native automatic naming and rendered title precedence
  unobserved. No bypass or configuration change is authorized.
- Preserve native terminal passthrough: live rename uses cmux's control channel;
  other terminals pick up renames on reconnect. Extending live rename to every
  host would need a provider naming API or terminal transport work.
- A plain `claude`/`codex` process has no lf owner. Repository naming guidance
  and the landed system-context fix cannot guarantee its title, especially when
  resuming an old instruction-derived name. lf also preserves skill/word-pair
  defaults until renamed. Reliable work-specific naming remains implementation
  work; the requirement has not been waived.
- With multiple Sessions in one cmux workspace, each surface keeps its name and
  the workspace follows the most recent launch or rename. Review that choice.
