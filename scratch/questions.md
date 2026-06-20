# Open questions / assumptions

- Assumed `session.launch` applies to existing interactive step runs (`interactive: true` or `-i`) and `--web` forces an interactive handoff for otherwise-headless steps.
- Assumed Codex CLI launch should mirror the repo's current interactive Codex flags (`-C`, `-c model=...`, `--sandbox workspace-write`) rather than switching to undocumented short flags in the design sketch.
