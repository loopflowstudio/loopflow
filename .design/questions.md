# Open Questions

Questions captured during UX research that need clarification or decisions.

---

## From UX Research (ux-agent branch)

### Scope Questions

1. **Designer/PM profile**: Is this user segment in scope for Maestro? The current experience is heavily developer-centric. If non-engineers are a target audience, significant changes would be needed (in-app results, reduced terminal exposure, non-code task presets).

2. **Inline results**: Would embedding a terminal emulator (as mentioned in ux-gaps.md wild ideas) be considered, or is "launch to external terminal" the intentional architecture? The mental model gap is the #1 issue across profiles.

3. **Task selector vs slash commands**: The ux-gaps.md suggests replacing the Task selector with Notion-style slash commands. Is this direction approved, or should we iterate on the dual-input pattern?

### Implementation Questions

4. **Context preview panel**: ~~The design in ux-agent.md shows file removal (✕ buttons). Should removed files be temporarily excluded or persisted?~~ **Implemented**: Files are excluded for the current session only (stored in `excludedFiles` Set, not persisted to config).

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

---

## From UX Gap Analysis Update

### Strategic Direction

8. **Launcher vs. integrated experience**: The fundamental architectural question—should Maestro:
   - Embrace being a launcher (make handoff to terminal elegant)
   - Compete with terminals (embed output inline)
   - Take middle ground (show results summary, not streaming log)

   Current recommendation is option 3. The OutputPanel would become a results view showing "what changed" after completion rather than streaming output.

9. **Output Panel fate**: Currently duplicates terminal output. Three options:
   - Transform into results panel (show files changed, test results)
   - Keep as-is but differentiate (summarize instead of stream)
   - Remove entirely (accept launcher role)

### Input Unification

10. **Dual input pattern**: Task selector dropdown + text field colon syntax creates confusion. Proposal: single text field with `/` prefix for tasks (Notion-style). Is this the right direction, or is there value in keeping both entry points?

### Keyboard Navigation

11. **Command palette scope**: Maestro has ~20 actions. Is a full Cmd+K command palette warranted, or would excellent keyboard shortcuts (shown in menus/tooltips) be sufficient?
