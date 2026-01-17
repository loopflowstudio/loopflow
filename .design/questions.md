# Open Questions

Questions captured during auto-mode runs that need human input.

---

## From ux-gaps analysis (2026-01-16)

### Architecture

1. **Could Maestro work without the lf CLI for basic operations?**
   - Currently requires lf/wt CLI tools installed before showing any value
   - A bundled mini-runtime could provide degraded-but-useful functionality
   - Trade-off: distribution complexity vs. first-run friction

2. **What's the core value of a native macOS app vs. extension/web app?**
   - Current Maestro mostly launches external apps (Warp, Cursor)
   - VS Code extension would integrate natively with file trees, terminals
   - Clarifying this shapes every design decision

### Onboarding

3. **What would a demo/sandbox repo contain to demonstrate value?**
   - Interactive exploration before opening real repo
   - Fake worktrees, example prompts, simulated output
   - Could be bundled or downloaded on first launch

4. **Is the worktree abstraction necessary for first-time users?**
   - Power users value worktree management
   - New users don't know what worktrees are
   - Could worktrees be auto-created and hidden from beginners?

### Interaction Model

5. **Should model selection happen before, during, or after prompt composition?**
   - Currently: prominent mode toggle before running
   - Alternative: infer from prompt, or select after seeing preview
   - Impacts whether users think in "loopflow vocabulary" or natural language
