# Open Questions

## UX Research (2026-01-16)

1. **Target audience priority**: Is Maestro primarily for power users who want a dashboard, or should it be accessible to non-technical users (designers, PMs)? The current design serves power users well but alienates newcomers.

2. **Results in terminal vs in-app**: Is there a technical reason results must appear in an external terminal? Would embedded PTY or websocket output streaming be feasible?

3. **Beta flag discoverability**: How are users expected to discover the Pipelines and Agents features hidden behind `Flags.beta`? Is there a preference panel or menu item planned?

4. **Screenshot capture permissions**: The Cmd+Shift+S debug capture triggers a system permission dialog. Is this the intended mechanism for users, or should there be a fallback that doesn't require screen recording permission?

5. **Worktree terminology**: The app uses "worktree" throughout, but this is git jargon. Would "workspace", "branch folder", or "isolated copy" be clearer for non-git-experts?

## UX Gap Analysis (2026-01-16)

6. **Configuration vs opinionation**: The gap analysis suggests hiding context toggles and defaulting to "implement" task. But loopflow's power comes from explicit context control. Is the right answer: (a) hide complexity for new users and reveal after first success, (b) accept that Maestro is a power-user tool and optimize for that, or (c) build two modes (simple/advanced)?

7. **Task inference viability**: Could Maestro infer task from prompt content (e.g., "review the auth" -> review task)? This would require NLP or pattern matching. Is this worth the complexity, or should we just default to "implement"?

8. **In-app streaming technical feasibility**: The gap analysis heavily emphasizes streaming results in-app. What's the technical path? Options: embedded PTY (complex), websocket from lfd (requires daemon changes), poll log files (simple but not real-time). Which is worth pursuing?

9. **Sidebar identity**: Should the sidebar show "branches" (git-centric) or "features in progress" (work-centric)? The latter is more accessible but loses precision for power users who think in git terms.
