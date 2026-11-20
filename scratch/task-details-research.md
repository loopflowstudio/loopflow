# Task details and workspace layouts: T3 Code, Paseo, Paradev, and Rex

Jack requested a sync with current Loopflow and a review of T3 Code, Paseo, and Paradev, with particular emphasis on T3 Code. This is research for the next design, not acceptance of the October prototypes.

**The strongest direction to explore is T3 Code’s conversation with a retained, tabbed companion.** Open a file, diff, PR, or brief beside the conversation; preserve its place; let a selection in that evidence become context for the next message. The central interaction is the round trip from conversation to evidence and back.

## Changes viewer: PR scope, focused reading, immediate GitHub handoff

Jack described difficulty in VS Code obtaining a branch/base comparison rather than only local changes; JetBrains previously served that need. Kaleidoscope is the macOS diff viewer he knows best. He clarified the desired default as what would appear after `lf pr open`, and requested a GitHub PR action that publishes the current diff promptly while its description updates in the background. These are design inputs; no publication or implementation was requested during this study.

### Comparison and reading

Default to **PR preview**: the changes this Task would publish against its resolved PR base, including pending local changes eligible for publication. Show the base branch explicitly; stacked Tasks may target a parent branch. A published-PR view is a separate snapshot and can lag the preview. Avoid presenting base-branch-tip comparison and merge-base comparison as interchangeable. [Git comparison semantics](https://git-scm.com/docs/git-diff)

The current `create_or_update_pr` path prepares a commit and pushes before resolving PR copy. It creates/updates the PR afterward, so opening a new PR still waits for generated copy. Its publication base handles stacks. The Changes reader already supports a recorded base, optional committed head, drafts and untracked paths, but exact agreement with publication preparation needs verification, including excluded artifacts. Source: `rust/loopflow/src/ops/pr.rs`, `rust/loopflow/src/ops/task/snapshot.rs`. A preview must disclose unsaved editor buffers separately from on-disk content publication would commit.

JetBrains exposes branch-to-working-tree comparison directly. Current VS Code also offers committed comparisons through its Source Control Graph, including Compare with Merge Base; Jack's experience identifies discoverability and desired scope as the problem, not proof that current VS Code lacks comparison. [JetBrains](https://www.jetbrains.com/help/idea/manage-branches.html), [VS Code](https://code.visualstudio.com/docs/sourcecontrol/history)

Kaleidoscope supplies useful references for both entry and reading: custom branch comparisons and recent comparisons; a changeset file sidebar with filtering; focused text comparison with change stepping and alternative layouts. Its modified-line treatment distinguishes edits from additions/deletions and emphasizes changed words. Proposed Loopflow first view: filterable file list, one focused diff, readable side-by-side layout when wide, unified option when narrow, next/previous change, expandable unchanged context and retained position. This revises the earlier preference for continuous multi-file scrolling: study focused-file reading first given Jack's familiarity. [Repository view](https://kaleidoscope.app/help/docs/repository-view), [changeset window](https://kaleidoscope.app/help/docs/changeset-window), [text views](https://kaleidoscope.app/help/docs/text-comparison-views), [change highlighting](https://kaleidoscope.app/help/docs/text-diffs-and-colors)

### Reusable filtering

Jack requested global filtering, with repository versus Wave scope left open. Proposal: repository defaults for common noise, named Wave presets for area-specific focus, temporary per-view overrides. Keep focus filters distinct from exclusions: hiding tests or lockfiles must not remove them from the PR. Always show the hidden-file count and a Show all action. Store paths/globs and file categories rather than tying the filter to one ephemeral diff. Whether defaults are personal or shared remains unresolved; repository scope alone does not decide that.

### Open on GitHub

Jack clarified that the action should publish as a PR immediately, with description work in the background. Proposed sequence:

1. Prepare and push the current Task change using the existing publication rules.
2. Create a draft PR with the Task title and a minimal factual body, or reuse the existing PR without replacing its body. Preserve an existing ready PR's state.
3. Open GitHub as soon as the PR exists and its diff reflects the pushed head.
4. Generate the full description in durable background work and update that same PR. A failure leaves the usable PR open and makes retry available.

The new background writer must retain the intended head and body revision. If another push or manual description edit overtakes it, it cannot overwrite the newer result. A web link click must not depend on prose generation, and background copy generation must survive the originating view closing. Existing PR diffs may update after a push already; the current bottleneck also delays returning/opening the link.

This CLI publication behavior is a separable outcome from the viewer UI. Record it for a related Task rather than silently broadening LOO-515 into publication infrastructure. The viewer can use that operation once available. Button wording should make its effect clear: **Create PR & open** initially, **Update PR & open** when local work needs publication, **Open on GitHub** when already current.

## Selected delight priorities — Jack's November 14 clarification

Jack selected items 1, 2, 5, 6 and 7 from the seven-item comparison list, clarifying item 6 as “live, not pause.” This is the current prioritization and supersedes the broader quality-bar ordering below:

1. Next/previous change traverses file boundaries so the entire review can be read without repeatedly selecting files.
2. Precise word-level highlighting makes a small edit visually small, distinguishing modified content from additions and deletions.
3. Select code and ask about it in the Task conversation, carrying file, side, range and revision context into the draft; sending remains explicit.
4. The comparison and file list update live as the agent writes, including new files. Preserve the current reading anchor and focus while updating. A pause control is not a requested priority and should not be a centerpiece of the prototype.
5. Recognize moved code and connect its old and new locations, exposing edits within the moved block. Within-file moves are the initial reference capability; cross-file/semantic move detection is not implied.

Moved-code detection is now a priority, superseding its earlier placement as a later candidate. Reviewed marks and changed-since-reviewed indicators are secondary. Basic place retention remains expected behavior, but it is not a separate featured delight at the expense of the five selected interactions. PR-equivalent comparison, reusable focus filters and prompt GitHub publication/opening remain the previously selected foundation.

The next prototype's main review sequence should exercise all five: follow a live edit, notice precise word changes, traverse files by keyboard, inspect a moved block, and send selected context to the conversation draft. Label any simulated data or effects. This priority selection does not accept the October layout or authorize publication.

## Changes viewer quality bar — November 14 direction

Jack requested substantially more emphasis on polish and delight. This supersedes the research's minimal-first prioritization; polish is part of the core review experience. The October layout and implementation plan below remain provisional.

The next prototype should demonstrate one complete, carefully finished review loop, using Kaleidoscope as the familiar reading reference. It must show PR-equivalent scope, reusable focus filtering, and a clear GitHub handoff together with the following proposed quality bar:

- **Reading:** precise word-level emphasis, restrained addition/deletion/modified colors with non-color cues, deliberate typography and spacing, aligned sides, expandable context, and width-aware split/unified presentation. Long lines and rename-only, deleted, empty and binary files need intentional treatments.
- **Movement:** next/previous change across file boundaries, visible position in the review, searchable file navigation, generous click targets, discoverable shortcuts and consistent focus. Expanding context, hiding the tree, changing layout and returning from conversation preserve a meaningful reading anchor.
- **Continuity:** retain selected file, folds, scroll and filter choices per Task/comparison. New evidence never silently changes the comparison or steals the selected file. Loading should preserve the previous readable content and disclose its freshness.
- **Review progress:** Viewed marks are tied to compared content; later changes are visibly eligible for review again. Live updates offer a clearly labeled pause/resume state and preserve the reader's position. Prototype these in the first review experience rather than deferring them as decoration.
- **Feedback:** select a range, write a short question, add it to the conversation draft and return to the same diff. Preserve side, file and revision context. Copying code should produce usable text without patch markers or line-number gutters.
- **Restraint:** transitions explain changes in position without delaying navigation; respect reduced motion. Filtering discloses hidden files and never alters PR contents. Routine operations stay close to the content; advanced controls need not occupy permanent space.

Demo sequence: open the PR preview; apply a saved focus filter; advance across two files by keyboard; expand context and inspect a word-level change; mark a file viewed; simulate a new agent edit and show Changed; pause updates; add a line question to the conversation; switch Task and return; invoke the GitHub handoff. Simulated effects must be labeled in the prototype. Include a narrow companion and a widened review state.

This is an increased design and demo bar, not acceptance of a specific pane layout or authorization to publish. Move detection remains an additional design candidate; compare its value against the readability of the core diff before implementation.

## Small interactions worth borrowing

Jack asked which other diff-viewer interactions offer delight. The strongest candidates reduce the work of reading and returning, rather than add more Git controls. These are proposals for Loopflow, informed by documented behavior and inspected source; no hands-on competitor trial was performed.

1. **One key through the review.** Next change crosses file boundaries, so reviewing a changeset does not require repeatedly finding the next file. Kaleidoscope 7 documents this; JetBrains offers continuation to another changed file at the boundary. [Kaleidoscope 7](https://blog.kaleidoscope.app/2026/07/28/kaleidoscope-7-live-working-copy-changes/), [JetBrains viewer](https://www.jetbrains.com/help/idea/differences-viewer.html)
2. **Live comparison with a pause control.** Kaleidoscope updates the file list and working-copy comparison as an editor or agent writes, including new untracked files, and lets the reader pause. For Loopflow, proposed refresh behavior must also retain the current reading anchor and make paused/stale evidence explicit. [Live changes](https://blog.kaleidoscope.app/2026/07/28/kaleidoscope-7-live-working-copy-changes/)
3. **Review marks that notice new work.** T3's PR view distinguishes Viewed from Changed when a later push alters a viewed file, where the host supports viewed-file state. This prevents a checkmark from concealing a revision that has never been read. [T3 PR code tab][t3-code-tab]
4. **Return to the same place.** T3 restores scroll per comparison scope and waits for paged content to load before positioning; folds are retained too. Proposed Loopflow behavior: switching Task, conversation or file never costs the reading position. T3's inspected scroll cache is session-only, not evidence of restoration after restart. [Scroll restoration][t3-scroll], [diff state][t3-diff-state]
5. **Turn a selection into a question.** T3 supports line annotations that become conversation draft context. Loopflow should carry file, old/new side, range and compared revision, with sending separate. [T3 annotations][t3-annotations]
6. **Make a small edit look small.** Kaleidoscope distinguishes modified lines from additions/deletions and emphasizes the changed words within them. This helps the eye locate the actual edit. [Text differences](https://kaleidoscope.app/help/docs/text-diffs-and-colors)
7. **Recognize moved code.** VS Code documents optional within-file move detection with links between locations. Useful for refactors; source-aware/semantic comparison is a separate, larger capability and is not implied by move detection. [VS Code move detection](https://code.visualstudio.com/updates/v1_82#_moved-code-detection)
8. **Let the reader control density.** Side-by-side or unified, synchronized scrolling, expandable equal regions, wrapping and visible whitespace controls solve different reading problems. A visible indicator should disclose ignored differences. JetBrains documents alignment and synchronized scrolling; Kaleidoscope offers several layouts and text settings. [JetBrains](https://www.jetbrains.com/help/idea/differences-viewer.html), [Kaleidoscope text views](https://kaleidoscope.app/help/docs/text-comparison-views)

Earlier minimal-first proposal, superseded by Jack’s increased polish emphasis above: stable return position, next-change navigation across files, word-level highlighting, and selection-to-conversation. Live updates with pause and revision-aware review marks matter especially when an agent keeps writing during review. Move detection is valuable later; it is not a prerequisite for the first viewer.

## Higher-level layouts and Rex

Jack asked how the whole workspace and pane setup compare, including Rex. The comparison below comes from the pinned source already inspected and Rex's public documentation on November 14. These are structural findings, not a hands-on usability ranking.

**The useful distinction is the default center of attention and what a tab switches.** T3 anchors the conversation; Paseo gives each split pane its own tabs; Paradev and Rex put split arrangements inside outer tabs.

```text
T3       project / thread → conversation + tabbed companion
                                        + terminal drawers

Paseo    workspace → split layout → pane → content tabs
                                 → pane → content tabs
                    dedicated Files / Changes Explorer

Paradev  workspace → tab → split layout → typed panes
                    tab → another arrangement
                    Files / Changes / Review sidebar

Rex      session → tab → split layout → terminal blocks
                 tab → another arrangement
```

| Product | What the layout privileges | Practical consequence |
| --- | --- | --- |
| T3 Code | An agent conversation, with references in a resizable right companion and terminal drawers. The companion can maximize or become a narrow-screen sheet; side chats are supported surfaces. | Predictable places for talking and inspecting. Broad tool variety, but their placement remains organized around the main conversation. |
| Paseo | A workspace with split panes, each holding agent, file, diff, terminal, browser, or PR tabs; a dedicated Explorer provides Files/Changes navigation. | Switching a reference tab can leave the agent and other panes unchanged. More freedom brings more decisions about where newly opened content belongs. |
| Paradev | A project/worktree workspace with tabs containing arrangements of typed panes, plus Files/Changes/Review navigation. Native agent terminals, chat, files, and browsers participate. | A tab can represent an entire working setup, such as agent plus shell or a review arrangement. The app supplies Git/worktree/PR context around that flexibility. |
| Rex | A session containing tabs, each holding a split layout of blocks. Public docs describe current blocks as terminals. | Programs have equal standing. A coding agent, editor, diff tool, and logs can occupy the same arrangement; the programs supply their own domain-specific interaction. |

Sources: [T3 panel][t3-panel] and [terminal/companion composition][t3-workspace]; [Paseo layout][paseo-layout] and [tab kinds][paseo-tabs]; [Paradev pane engine][paradev-panes] and [workspace composition][paradev-workspace]; [Rex taxonomy][rex-taxonomy] and [pane actions][rex-actions]. Paradev also has a separately managed right pane area, so its current workspace is more elaborate than a single uniform split tree.

### What Rex changes in the comparison

Rex's session continues on its server when no client is viewing it. Its controls include directional splits, best-fit split, pane movement, tab movement and tab detachment into another window; CLI/Lua/API can arrange the workspace. This makes it a strong reference for durable, directly manipulable arrangements. It does not by itself establish the review workflow described above for T3, Paseo, and Paradev. Its docs list block plugins as a developing surface; the public taxonomy currently calls blocks terminals. Do not infer that a native file/PR companion is already a standard Rex block. [Taxonomy][rex-taxonomy], [actions][rex-actions], [API][rex-api]

Paseo and Paradev are closer in tool variety than the phrase “agent app” suggests. Their notable distinction here is nesting: **Paseo's pane contains tabs; Paradev's tab contains panes.** Rex shares the latter arrangement. This changes whether switching a tab replaces one reference or the complete visible setup. [Paseo layout][paseo-layout], [Paradev pane engine][paradev-panes], [Rex taxonomy][rex-taxonomy]

### Proposed Loopflow default

Use the existing Task workspace and movable tools, with T3's clear default emphasis:

```text
Task navigation | Conversation       | Changes / Files / Brief / PR
                |                    | selected reference
                | Shell when opened  |
```

Navigation selects the Task; tool tabs select content within it. Start with the conversation and one useful companion visible. Preserve resize, hide, zoom, and arrangement. Keep Shell available without consuming permanent space. Avoid making Brief, PR, Files, and Changes four mandatory columns.

This is a proposal for defaults and the existing companion, not approval to replace Loopflow's pane system with arbitrary IDE editor groups. Current accepted direction retains one Files tool and one Shell tool per Task, with tabs. The synced README already records retained per-Work layouts and Wave defaults. External Rex layouts are owned by Rex after opening; rearranging them does not rewrite Desktop's saved layout. [Current Desktop behavior](../swift/README.md), [host layout contract](../docs/workspaces.md)

The next Changes prototype should demonstrate this complete frame, including a wide review view and return to the conversation. Exact width ratios and additional splitting remain design choices.

## How the systems fit together

```text
T3 Code    thread → retained companion tab → scoped diff / file / PR
                         selected evidence → composer draft → thread

Paseo     workspace → Files / Changes explorer → document or side pane
                         review comments → choose agent → send feedback

Paradev   workspace → Files / Changes / Review → file, diff, or PR split
                         review line → agent feedback (+ optional GitHub post)
```

## T3 Code: the most useful reference

### 1. A conversation with contextual surfaces beside it

`ChatRightPanel` places the same tabbed content beside chat on wide layouts and in a sheet on narrow layouts. Its breakpoint is 980 px. The surface model explicitly represents files, the file browser, diffs, PRs, browsers, terminals, devices, and side chats. A PR retains which detail tab was selected; a file can carry a line to reveal. This makes opening a reference a navigation operation with a destination, rather than a generic switch to “Files.” [Panel composition][t3-panel], [surface model][t3-state]

Tabs and active selection are stored per environment/thread. The store distinguishes automatic reveals from deliberate user choices: a delayed proactive open checks whether the user has chosen something else in the meantime. A completed turn’s diff is offered once, and revisiting the thread does not repeatedly reopen it. Tests cover that behavior and isolation between environments. **Borrow the respect for an existing reading position, not just the tab styling.** [State and automatic opens][t3-state], [behavioral tests][t3-state-tests]

### 2. Changes have a named scope

The diff picker includes branch changes, uncommitted changes, the latest turn, a selected turn, and individual commits. A turn’s changed-files card links straight to that turn/file. The source-control guide says turn diffs omit changes merely brought in by a merge or rebase, while branch comparison includes the whole branch change. That prevents “what did the agent do?” from silently becoming “what changed in Git?” [Diff viewer][t3-diff], [turn card][t3-turn], [source-control behavior][t3-git]

For Loopflow, local checkout versus published PR is the essential distinction now. Turn-level scope would require trustworthy turn snapshots or attribution; the UI alone cannot supply that evidence.

### 3. Review becomes a message with context

Selecting diff lines creates a comment with the file, range, and diff context. Submitting that annotation adds it to the conversation’s composer draft. It does not immediately send the conversation message. The file tree also supports **Add to chat** and file mentions. This removes the manual loop of copying a path, finding line numbers, and pasting a snippet before asking a question. [Diff annotations][t3-annotations], [file browser][t3-files]

This is the most valuable interaction missing from the October prototypes. A useful companion should help formulate the next message as well as display information.

### 4. A tree chooses evidence; the document gets the space

The file browser loads directories through an environment-scoped reader and uses project path search. It retains expansion, reveals ancestors of a selected file, and supports context actions. The diff tree is optional and resizable; the code reserve prevents the navigator from consuming all the reading width. The diff itself is a continuous, collapsible multi-file document with sticky headers, unified/split modes, wrapping, and deferred patch loading. [File browser][t3-files], [explorer sizing][t3-explorer], [diff viewer][t3-diff]

This is a stronger starting point than making every changed file a separate navigation trip. It also argues for separating the checkout tree from the changed-file set: they answer different questions.

### 5. PRs are review objects, beyond a status card

The PR detail has **Summary**, **Timeline**, and **Code**, plus an optional beta **Guide**. The guide proposes an ordered reading tour, exposes excerpts and questions, and marks itself stale when its recorded head differs from the current head. Files can be marked viewed, and subsequent changes are surfaced again. These support understanding and review progress, not just publication. [PR detail][t3-pr], [guide and viewed-file behavior][t3-git], [guide freshness][t3-guide]

Loopflow’s brief and `scratch/` pages can provide outcome and reading guidance without implementing generated review tours in this Task. The enduring Task brief still needs its own identity: a proposed plan in a conversation is not the accepted Task scope. T3’s proposed-plan card is collapsible and can be copied, downloaded, or saved to the workspace. [Plan card][t3-plan]

### 6. Retention has several lifetimes

T3 persists panel selection and diff source per thread, but some review state is session-only. Its scroll restoration waits until enough diff content has loaded to reach the remembered position. A separate PR test suite checks that an unchanged refresh keeps the diff and that navigation can load later file pages before revealing a target. This is concrete evidence of attention to continuity, not a claim that every editor/PR round trip is flawless. [Diff state][t3-diff-state], [scroll restoration][t3-scroll], [PR tests][t3-pr-tests]

## Paseo: useful defaults inside flexible panes

Paseo supplies a particularly relevant design reversal. Its August 25 changelog records restoring a dedicated **Files / Changes Explorer**, replacing an empty, user-directed side panel. Content opened to the side became an ordinary closable pane. It also added per-action placement preferences and standalone diff tabs. This is evidence of a product change, not proof of the motives or user research behind it. [Changelog][paseo-changelog]

Current code keeps a distinction between the Changes tree and the working-diff viewer, supports opening to the side and adding files to chat, and stores collapsed files/folders separately from display preferences. A browser test explicitly checks that a new agent opens in the main area without replacing the selected Explorer content. **The lesson for Loopflow: provide a good default arrangement before offering more arrangement choices.** [Diff panel][paseo-diff], [pane placement test][paseo-placement]

Paseo’s **Send feedback** action collects review comments, selects a recipient when necessary, and clears only successfully sent comments. Failed sends retain feedback; disconnected or missing-context states explain why sending is unavailable. This is a second implementation of the evidence-to-conversation loop, with more direct sending than T3’s composer-draft path. [Feedback state machine][paseo-feedback]

## Paradev: separate navigation, delivery, and review

Paradev’s current workspace sidebar explicitly has **Files**, **Changes**, and **Review**. Review contains the PR header, checks, and comments; selecting a comment can reveal its location in the diff. Changes shares an explicit comparison control with the diff pane, including its base branch. Sidebar selection is retained per workspace. [Workspace sidebar][paradev-sidebar], [Review content][paradev-review], [comparison control][paradev-scope]

Its PR detail has **Summary / Code**. The documented workspace gesture opens a PR in a split on click, a new tab on Shift-click, or the host website on Command/Ctrl-click. Line comments can go to an agent with path, range, and diff side; posting to GitHub is optional. **Borrow the direct path from review feedback to the relevant code, and make opening locally versus externally predictable.** [PR documentation][paradev-pr]

Paradev also has a broader, configurable workspace and delivery surface. Copying all its controls into Loopflow’s companion would enlarge this Task considerably; the useful patterns are the explicit destinations and connected review loop.

## Comparison

| Question | T3 Code | Paseo | Paradev |
| --- | --- | --- | --- |
| Where does evidence open? | Thread’s tabbed companion | Retained panels with placement preferences | Tabs/splits, with an explicit workspace sidebar |
| What changed? | Branch, uncommitted, turn, commit; published PR separately | Working/base diff, commits, separate PR panel | Changes scope/base selector; published PR separately |
| How does feedback return? | Line comments in conversation draft | Collected comments sent to a chosen agent | Line comment sent to agent; optional GitHub post |
| What transfers best? | Stable context and contextual handoff | Useful defaults and focus preservation | Clear review navigation and comparison scope |

## What this changes for LOO-515

Proposed revisions to the October design, **not accepted choices**:

1. **Keep one retained companion beside the conversation.** Give Brief, Changes, PR, and open documents recognizable destinations. Avoid competing global and file navigation in one undifferentiated strip. The evidence favors a revision of A/C over B’s single scrolling dossier as the whole workspace.
2. **Preserve published PR review.** Reuse rendering where practical, but keep local checkout changes and published PR changes distinct. Delete redundant rendering code only when the replacement still serves both sources. The old blanket deletion of the GitHub patch viewer is too aggressive.
3. **Make selection useful in the conversation.** Explore an “Add to conversation” action carrying file, lines, comparison revision, and optional comment. Keep sending explicit. Loopflow uses native provider terminals, so this needs the existing input/draft owner rather than assuming T3’s web composer exists here.
4. **Let reading dominate the space.** Optional tree, explicit comparison label, continuous diff, file reveal, and stable scroll/folds. Widen or zoom the companion when needed while preserving easy return to the conversation.
5. **Keep the brief authoritative.** Outcome, acceptance, dependencies, and status come from the Task. Plans, walkthroughs, and source files can open alongside it. None of the reviewed components establishes a direct equivalent of Loopflow’s Task/Graph authority.

## Research record

### Evidence boundary

Current upstream source, documentation, test cases, and repository-supplied screenshots. No authenticated, running-app walkthrough was performed; tests were read, not executed. Screenshots illustrate hierarchy, not proof that every current interaction works. Findings below distinguish implemented behavior from proposals for Loopflow.

| Repository | Inspected revision |
| --- | --- |
| T3 Code | `3e1761821226c472318168238c7599eea26dc49c` |
| Paseo | `7286d3ef81077654e30fe50393dc449b52c84c72` |
| Paradev, formerly Superset | `c524b94c53e566487cb7551c3ed082771de9e292` |

Source links pin the inspected revisions.

[t3-panel]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/chat/ChatRightPanel.tsx
[t3-state]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/rightPanelStore.ts
[t3-state-tests]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/rightPanelStore.test.ts
[t3-diff]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/DiffPanel.tsx
[t3-turn]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/chat/ChangedFilesTree.tsx
[t3-git]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/docs/user/source-control.md
[t3-annotations]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/diffs/AnnotatableCodeView.tsx
[t3-files]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/files/FileBrowserPanel.tsx
[t3-explorer]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/files/FileExplorerPane.tsx
[t3-pr]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/pullRequest/PullRequestDetailPanel.tsx
[t3-guide]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/pullRequest/PullRequestGuideTab.tsx
[t3-plan]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/chat/ProposedPlanCard.tsx
[t3-diff-state]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/diffPanelStore.ts
[t3-scroll]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/diffs/useCodeViewScrollRestore.ts
[t3-pr-tests]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/pullRequest/PullRequestCodeTab.browser.test.tsx
[paseo-changelog]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/CHANGELOG.md#060---2026-08-25
[paseo-diff]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/packages/app/src/panels/diff-panel.tsx
[paseo-placement]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/packages/app/e2e/browser/explorer-pane-placement.spec.ts
[paseo-feedback]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/packages/app/src/review/feedback/index.ts
[paradev-sidebar]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspace/%24workspaceId/components/WorkspaceSidebar/WorkspaceSidebar.tsx
[paradev-review]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspace/%24workspaceId/components/WorkspaceSidebar/hooks/useReviewTab/components/ReviewTabContent/ReviewTabContent.tsx
[paradev-scope]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspace/%24workspaceId/components/ChangesScopeControl/ChangesScopeControl.tsx
[paradev-pr]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/apps/docs/content/docs/pull-requests.mdx
[t3-workspace]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/ChatView.tsx
[paseo-layout]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/packages/app/src/stores/workspace-layout-actions.ts
[paseo-tabs]: https://github.com/getpaseo/paseo/blob/7286d3ef81077654e30fe50393dc449b52c84c72/packages/app/src/workspace-tabs/model.ts
[paradev-panes]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/packages/panes/README.md
[paradev-workspace]: https://github.com/ParadevAI/Paradev/blob/c524b94c53e566487cb7551c3ed082771de9e292/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspace/%24workspaceId/page.tsx
[rex-taxonomy]: https://www.superlogical.com/rex/docs/automate/cli#taxonomy
[rex-actions]: https://www.superlogical.com/rex/docs/reference/mac-actions#pane
[rex-api]: https://www.superlogical.com/rex/docs/automate/server-api

[t3-code-tab]: https://github.com/pingdotgg/t3code/blob/3e1761821226c472318168238c7599eea26dc49c/apps/web/src/components/pullRequest/PullRequestCodeTab.tsx
