# Open Questions

## UX Research (2026-01-16)

1. **Target audience priority**: Is Maestro primarily for power users who want a dashboard, or should it be accessible to non-technical users (designers, PMs)? The current design serves power users well but alienates newcomers.

2. **Results in terminal vs in-app**: Is there a technical reason results must appear in an external terminal? Would embedded PTY or websocket output streaming be feasible?

3. **Beta flag discoverability**: How are users expected to discover the Pipelines and Agents features hidden behind `Flags.beta`? Is there a preference panel or menu item planned?

4. **Screenshot capture permissions**: The Cmd+Shift+S debug capture triggers a system permission dialog. Is this the intended mechanism for users, or should there be a fallback that doesn't require screen recording permission?

5. **Worktree terminology**: The app uses "worktree" throughout, but this is git jargon. Would "workspace", "branch folder", or "isolated copy" be clearer for non-git-experts?
