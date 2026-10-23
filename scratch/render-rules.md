# Render-cost rulebook (research, October 22)

Sources: WWDC23 10149/10160, WWDC21 10022, WWDC25 306, Airbnb SwiftUI performance,
Fatbobman lazy containers, Swift with Majid scroll geometry.

A. Observation
1. A body depends only on the @Observable properties it reads.
2. Push reads into leaf views; pass the object, let the child read the property.
3. Per-row observable objects, not a slot in a shared collection read by every row.
4. Equal writes to Equatable properties no longer notify on current toolchains;
   non-Equatable types and in-place mutation (append) still do. Guard when unsure.
5. Computed properties read in body depend on every stored property they touch;
   store hot derived values.
6. @ObservationIgnored for state body never reads (caches, timers, scroll offset).
7. Apply refreshes in place by id; new instances with the same id re-subscribe rows.

B. Diffing
8. View stored properties are props; closures never compare equal, so a view taking
   a closure re-runs on every parent update.
9. Heavy closure-taking views: Equatable + .equatable() comparing data only (disputed
   whether conformance alone suffices).
10. Body cheap and side-effect free: no sorting/filtering/formatter creation.
11. Split big bodies into sub-structs; computed `some View` vars/funcs are not
    diffing boundaries and re-run with the parent.

C. Identity
12. Structural identity; no AnyView to unify branches.
13. Stable ForEach ids, one view per element, filter in the model not in ForEach.
14. A changing .id() is a remount; only for deliberate resets.
15. Hover reveals: keep mounted, opacity + allowsHitTesting + accessibilityHidden;
    hover state local to the row (or an observable flag read only by the leaf).

D. Scrolling, time, animation
16. Long lists: LazyVStack in ScrollView or List; plain VStack builds every row.
17. No state writes per scroll frame; reduce onScrollGeometryChange to a coarse value.
18. TimelineView around the smallest leaf; slowest acceptable rate.
19. Explicit animations; refreshes apply with animations disabled.
20. onContinuousHover fires per mouse move; reduce before writing.

E. AppKit / Ghostty
21. updateNSView cheap, idempotent, change-gated; never rebuild the terminal there.
22. Expensive NSViews owned by a longer-lived store; terminal output/focus/cursor
    never reach observed properties.

F. Measure
- Debug › Flash Re-renders (Dev builds; RenderCost.swift).
- Instruments 26 SwiftUI template: Update Groups, Long View Body / Representable
  Updates, Cause & Effect Graph.
- `Self._logChanges()` behind #if DEBUG as a hint.

## Hotspot audit (October 22), ranked; status beside each

1. SessionsContentView.body (SessionsView ~1062) reads breadcrumb/sessions/sidebarWidth/
   focus and passes fresh closures to WorkNavigator + WorkspacePanesView + 15-closure
   ConsoleCommandActions → whole window per Session change.
2. SessionsStore.reconcile (SessionsView 733–769) element writes always notify → every
   pane + Ghostty updateNSView per Session change; closedState rewritten (MultiplexerStore 73).
3. Sidebar rows inline in one body; O(rows×members×tasks) task(id:) lookups.
4. WorkModel.apply unguarded: selectionPlans, comments.receive, taskDiagnostics,
   workActivityScope, waveDetail sequence, selectedTaskEvidence tuple.
5. Pane strip hover shared (hover.paneId) read by every pane body; Close inserted by `if`.
6. ⌘/flagsChanged writes navigation.showsSlotHints unguarded; every row reads it.
7. Breadcrumbs rebuild WorkProjection up to 6× per body.
8. Row TimelineView wraps preview + metadata, every 5 s per row.
9. previewCopy written every 2 s heartbeat (WorkModel 1259).
10. RepoView onChange(of: model.workScope) depends on planning; SessionsContentView.init
    side effects write repoPath.
11. Dashboard tiles inline funcs; fresh onFocus closures; member.token JSON per tile.
12. ConsoleCommandActions new every root render.
13. Sidebar/pane divider drags re-render window / every pane per frame.
14. RepoView.init decodes saved workspace per parent re-render.

## Rulebook check (October 22)

1. GhosttyTerminalView.body → shellCommand → LoopflowZshBootstrap.install()/installBash():
   two disk writes per shell-pane body run.
2. Ghostty updateNSView ungated: set_size, refreshCommandBlocks, set_feedback per update.
3. Shared pane hover (= audit 5).
4. Dashboard tiles: no diffing boundary (= audit 11).
5. onFocus closure chain through panes.
6. Dashboard focus spring resizes every terminal each animation frame.
7. WorkNavigator rows func + closures (= audit 3).
8. Row TimelineView scope (= audit 8).
9. Sidebar plain VStack (LazyVStack; test focus/scroll restore).
10. HoverRevealed hit testing/accessibility — fixed.
11. Implicit .animation(value: isFocused) over whole pane.
12. AnyView in MultiplexerNodeView split tree.
13. Small identity issues (pinned filter in ForEach, palette two views per element).
14. Formatters in body (TaskCommentsView, Telemetry).

## Status (October 22, after 7aaf276ef1)

Done: audit 2, 3 (slots/ids once; rows still inline), 4 (selectionPlans,
previewCopy only), 5, 6, 8, 9, 11; root reads of sidebarWidth and sessions moved
(part of 1); checker 1, 2, 3, 4, 5 (Equatable, not removed), 8, 10, 11, 12.
Left: 1 rest (breadcrumb/projection memo, ConsoleCommandActions 12), 4 rest
(comments, taskDiagnostics, workActivityScope, waveDetail, selectedTaskEvidence),
7 breadcrumbs ×6, 10 RepoView workScope + init side effects, 13 pane divider
drag, 14 RepoView.init decode; checker 6 focus spring resizing terminals,
9 LazyVStack (focus/scroll restore risk), 13 identity, 14 formatters.
Orphaned `lf monitor work` after quit: intermittent; not reproducible outside
the app (exits in 0.5 s on EOF); no other pipe holders seen.

## Status (October 23, after 5ddbdfd445)

Jack: sidebar scrolling fixed. Breadcrumb memoized (audit 7, the breadcrumb part
of 1); comments, taskDiagnostics, workActivityScope and selectedTaskEvidence
guarded (audit 4). Left: waveDetail (WaveDetailSnapshot not Equatable; Wave
pane only), ConsoleCommandActions per render, audit 10/13/14, checker 13/14.
Checker 6 (focus spring) stays until Jack judges flicker: interpolation re-runs
layout and Ghostty resize, not bodies. Checker 9 (LazyVStack) dropped: scrolling
is fixed and it risks focus/scroll restore.
