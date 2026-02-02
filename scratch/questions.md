# Open Questions

## Connect Button Implementation

The design doc says Connect should "open the embedded Ghostty terminal attached to the running process" and "attach to existing PTY". The current implementation opens an external terminal in the worktree directory instead of attaching to the running agent's PTY.

Full PTY attachment would require:
1. Daemon to track/expose the PTY file descriptor for running agents
2. Ghostty or Concerto to support connecting to an existing PTY (not just spawning new processes)

The current implementation provides a useful workaround - users can inspect the worktree, run git commands, or observe files while the agent runs. For true PTY attachment, consider implementing:
- `/v1/waves/{id}/attach` endpoint that returns PTY connection info
- GhosttyKit extension for attaching to existing PTYs
