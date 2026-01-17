# Open Questions

## Resolved (2026-01-16)

1. **Target audience priority**: Power users for now. But progressive disclosure still matters—don't overwhelm with everything at once. Reveal complexity gradually while keeping the UI approachable.

2. **Results in terminal vs in-app**: Explore embedding an existing terminal (Ghostty, Warp) rather than building a competing terminal. Don't reimplement—integrate.

3. **Beta flag discoverability**: No discovery path needed. Beta features are in development; users won't see them until they're ready.

4. **Screenshot capture permissions**: Developer-only feature. Permissions will persist after initial grant—acceptable friction for developers.

5. **Worktree terminology**: Use "Workspaces" in the UI for approachability. The underlying git concept is worktrees, but "workspace" is more intuitive for users who aren't git experts. Code and CLI can still use "worktree" terminology.

6. **Configuration vs opinionation**: Progressive disclosure, not dumbing down. Keep advanced options but reveal them gradually. Don't hide context toggles—but maybe collapse them initially. Use real terminology (worktrees, diff, tokens) but don't require understanding everything upfront.

9. **Sidebar identity**: Changed from "BRANCHES" to "Workspaces" for friendlier tone. The all-caps semibold was visually aggressive; medium weight title case is more approachable.

## Open

8. **In-app terminal embedding**: SwiftTerm is implemented and works for auto mode. Should it be the default? Current: toggle is buried in "More options". Users who don't find it experience jarring context-switch to external terminal. **Recommendation from gap analysis**: Make embedded terminal the only option for auto mode. No toggle.

10. **Onboarding flow scope**: What should a first-time user walkthrough cover? Current candidates: (1) What Maestro does, (2) How workspaces isolate work, (3) Running your first task, (4) Where to see results. Should it be skippable?

11. **Mode picker explanation**: "Auto" vs "Interactive" is meaningless to new users. Options: (a) Add tooltips (already implemented but requires hover), (b) Add inline description, (c) Rename to "Run to completion" vs "Chat mode", (d) Hide mode picker entirely—default to auto, let task frontmatter control. **Recommendation from gap analysis**: Hide entirely. Default auto. Task frontmatter controls exceptions.

12. **Context chips progressive disclosure**: Context bar collapses by default (implemented via @AppStorage). But when expanded, five chips appear with no explanation. Add descriptions? Help links? **Recommendation from gap analysis**: Make context invisible by default. Smart per-task defaults. Show context section only when expanded deliberately.

13. **Whimsical worktree names**: NameGenerator produces names like "floral-tiger" which confuse new users. Keep for power users? Or use task-based names like "implement-auth"? **Recommendation from gap analysis**: Use the initial prompt as workspace identity. "add authentication" > "floral-tiger".

14. **Demo mode**: Should Maestro include a bundled demo project so users can experience the full workflow before opening their own repo? This would demonstrate value before commitment. **Recommendation from gap analysis**: Yes. Welcome screen should show product working, not just describe it.

15. **Slash commands**: Should the prompt input support `/design`, `/review` etc. as an alternative to the task dropdown? This aligns with Notion-style discoverable commands. **Recommendation from gap analysis**: Yes. Slash commands as explicit override. Task inference as default.

16. **@ mentions for context**: Should users be able to type `@src/auth.ts` in the prompt to add specific files to context? This is the Cursor pattern for surgical context override. **Recommendation from gap analysis**: Yes. @ mentions are faster than toggles and more discoverable than drag-drop.

17. **Work-state grouping**: Should the sidebar organize workspaces by work state (In Progress / Ready / Blocked) rather than flat list? This surfaces what needs attention. **Recommendation from gap analysis**: Yes. Work state matters more than creation order.

18. **Permission dialog identity**: The screen recording permission shows "MaestroU0.2026.01.14.08.16.sta-9kc_042" which looks like malware. Can we control the bundle identifier shown in system dialogs? **This is critical** — first system interaction erodes trust.

19. **Command preview prominence**: Power users want to see the exact command before running. Currently requires expanding "More options" then "Command Preview". Should remember expansion state? Add keyboard shortcut?

20. **Non-developer tasks**: Should Maestro support "explain" or "summarize" tasks for non-code users (Designer/PM profile)? Or is this out of scope for a developer tool?

21. **Help integration**: No "?" buttons or links to documentation in the interface. Add contextual help? In-app help panel? "Learn more" links in tooltips?

22. **Unified input**: Should the prompt input also function as command palette? Type to see tasks, recent prompts, slash commands, @ mentions. One input for everything (Raycast pattern).

23. **Clipboard mode**: Should Maestro work without a repo? Paste code → describe change → see result. Zero commitment entry.

24. **Sidebar presence**: Should the sidebar be permanent or on-demand? Most sessions use one workspace. Cmd+K could switch workspaces; sidebar appears only when invoked.

25. **Structured progress vs terminal**: Should running state show structured phases ("Reading codebase... Planning... Writing code...") with raw terminal as escape hatch? Or raw terminal only?

## Not Pursuing

7. **Task inference**: Decided against. Not worth the complexity—users can select tasks explicitly.

*Note: Gap analysis recommends reconsidering task inference. Ghost text showing inferred task, Tab to confirm. Simpler than dropdown for 80% case.*
