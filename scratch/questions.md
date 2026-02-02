# Open Questions

- List formatting for `lf --list` is simplified; do we need exact parity with the Python CLI output (sections, badges, external skills)?
- `lf config` currently prints raw file contents for `--global/--repo` and debug output for merged config; should we add structured YAML output for merged config?
- `lf flow --pr` is warned but not implemented. Should Rust `lf` create PRs after flow completion (and if so, via gh or engine APIs)?
- Flow execution pauses on interactive steps and exits; do we need to resume flows automatically after interactive steps complete?
- Ops commands are minimal; `next` and `abandon` only implement basic git behavior. Should we port the full Python behavior (stacked branches, wave integration, PR auto-merge, worktree preservation)?
- `--chrome/--no-chrome` is modeled as `--chrome <bool>` in clap; should we add explicit `--no-chrome` flag for parity?
