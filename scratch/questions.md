# LOO-924 implementation notes

- 2026-11-17: Keep WorkSource as attribution provenance: context and primary
  selection still read its variants. Launch lifetime no longer reads it.
- `codex_service_tier` and `chrome` stay on AgentConfig: they are provider
  launch options, independent of the selected model.
- The planned demo says `lf ps` shows Bare/Worker. Current process surfaces
  expose no launch-state label, and Run is intentionally not a wire/storage
  type. Verify lifetime, hook ownership and steer delivery in the launch
  matrix; defer adding a process label rather than deriving Run again from argv.

- 2026-11-17: Moving the document fixture to managed resume exposed a lost
  `--docs` selection on the fresh conversation's exec. Forward invocation
  settings in the existing conversation argv so the initial prompt keeps it.
