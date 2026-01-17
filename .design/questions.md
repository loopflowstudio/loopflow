# Open Questions

Questions captured during UX research that need clarification or decisions.

---

## From UX Research (ux-agent branch)

### Scope Questions

1. **Designer/PM profile**: Is this user segment in scope for Maestro? The current experience is heavily developer-centric. If non-engineers are a target audience, significant changes would be needed (in-app results, reduced terminal exposure, non-code task presets).

2. **Inline results**: Would embedding a terminal emulator (as mentioned in ux-gaps.md wild ideas) be considered, or is "launch to external terminal" the intentional architecture? The mental model gap is the #2 issue across profiles.

3. **Task selector vs slash commands**: The ux-gaps.md suggests replacing the Task selector with Notion-style slash commands. Is this direction approved, or should we iterate on the dual-input pattern?

### Implementation Questions

4. **Context preview panel**: The design in ux-agent.md shows file removal (✕ buttons). Should removed files be:
   - Temporarily excluded for this run only?
   - Persisted as exclusions in config?
   - Something else?

5. **Running state indicators**: The research identifies "running state invisible" as an issue. Should running worktrees show:
   - Animated spinner?
   - Pulsing dot?
   - Progress percentage (if knowable)?
   - Stage indicator (reading → planning → writing)?

6. **Worktree auto-creation notification**: When a worktree is auto-created from main branch, should Maestro:
   - Show a toast notification?
   - Show an inline banner?
   - Do nothing (current behavior)?

### Priority Questions

7. **Which profile to optimize for first?**: The three profiles have different needs:
   - New Developer: Onboarding, explanations
   - Power User: Context preview, keyboard navigation
   - Designer/PM: In-app results, reduced complexity

   Current evidence suggests Power User is the primary target (CLI parity expected). Confirm?
