# Open Questions

- Should agent prompts be passed via stdin instead of CLI args for Claude/Codex/Gemini launches in Rust?
- Do we want per-model token budgets in Rust config (parity with Python), or keep a single budget?

# Decided

- **Flow resume after interactive steps**: Need a clear API for interactive sessions to signal completion so flows can continue. When running `lf flow ship` and it hits an interactive step like `design`, the step runs interactively, and when the user completes it (e.g., exits the agent), the flow should resume automatically. Implementation: interactive step exit → flow continues to next step.

- **`--pr` flag**: Low priority. Fine to just document "run `lf ops pr` after" rather than implement `--pr` on flow command.

- **`--chrome/--no-chrome` flags**: Implemented as separate boolean flags matching Claude CLI pattern. Uses `clap`'s `overrides_with` to handle mutual exclusivity.

- **Ops commands full parity**: Created roadmap doc [01-lf-ops-parity.md](../roadmap/rust/01-lf-ops-parity.md) covering worktree management, stacked branches, wave integration, PR auto-merge, commit message generation, and shell integration.
