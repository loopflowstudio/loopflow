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

6. **Should output stay in external terminal or move into Maestro?**
   - External terminal: simpler architecture, users already know terminals
   - Inline output: more integrated "workspace" feeling, like Cursor
   - Hybrid: summary in Maestro, full output available externally
   - This is the difference between "launcher" and "workspace"

### Three Modes Problem

7. **Should Maestro serve one interface or three distinct modes?**
   - Quick Command: "Run review on this branch" (speed-optimized)
   - Exploration: "Help me figure out what to build" (conversational)
   - Orchestration: "Run ship pipeline across three worktrees" (visibility)
   - One interface serving all three = mediocre at each?

### Language & Naming

8. **Should we rename "worktrees" to something less technical?**
   - "Worktree" is git plumbing; users care about "work"
   - Alternatives: "parallel features", "branches", "work items"
   - Same question for "voices" → "tone" or "style"?

### Platform Strategy

9. **What's the iOS/iPad story?**
   - Native macOS patterns may not translate to touch
   - Patterns we pick now constrain future platforms
   - Is Maestro desktop-only forever, or cross-platform eventually?
