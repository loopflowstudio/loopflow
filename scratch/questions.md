# Open questions

Resolved during implementation:

1. **Layout scripts location**: `scripts/layouts/` (simpler, matches design doc).
2. **Container `run` action default**: Infer wave from current repo. If ambiguous, prompt with picker.
3. **Help overlay**: Use `display-popup` on tmux 3.2+ (check version), fallback to `display-message`.
4. **Status in lf mode**: Show git branch. Show active step if an `lf` process is detectable via `pgrep`.
