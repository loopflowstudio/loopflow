# LOO-903 native context preview

- History root: `/Users/jack/.lf/demos/loo-903-backlog/history`
- Runtime root: `/Users/jack/.lf/demos/loo-903-backlog/runtime`
- App: `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Preview.app`
- Session: `54042b3fb1a74c77accdec367946048e`; no demo provider Session is created.
- Data: fresh shared read, 46 unstarted Product Tasks, November 14, 2026. Full snapshot retained privately as `history/backlog.json`.
- Runtime has no CLI, provider or planning connection. Its LF_HOME is not consumed. No private Machine needs provision; the app only reads its bundled snapshot. No existing Loopflow app is replaced or quit.
- Source: `BacklogPreview.swift`; compiled with the checkout's `AppBootstrap.swift`, `DesignSystem.swift`, `BrandColors.swift`, `ThemeDefinition.swift`, `ThemeFile.swift` and `GhosttyThemeMapping.swift`. Bundled Fonts and Themes come from those sources' existing resources.
- Teardown: close the window or quit **Loopflow Backlog Preview**. It discards unsaved preview-only drafts; the real plan is unchanged. Verify its recorded PID exited before removing runtime. Keep history.
- Reopen: launch the App path above. No provider process or execution is reconstructed.
- Launch and observed UI results will be added after the app is running.

## Native review opened

- Build: Swift 6 compilation against the production design sources passed; the separate preview bundle was locally signed.
- `cua.getApp` opened **Loopflow · Backlog preview**. Its accessibility reading showed Product, Backlog 46, search, selectable Task rows, a full LOO-452 brief, Edit brief, and a separate Repository conversation button.
- Native screenshot inspected: burgundy titlebar, retained working-Task sidebar and Wave navigation, compact middle list, full-height brief reader. No brief Back/Forward or tabs.
- This proves the native layout is presented, not real saves, provider attachment, scroll retention across actual Loopflow views or design approval. The window remains open for review.
- Live preview PID: `46350`; executable `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Preview.app/Contents/MacOS/BacklogPreview`. Teardown if normal Quit fails: recheck this exact PID and executable, then `kill -TERM 46350`. Runtime stays while it is live.

## Reorder / compact-header revision

Revised bundle: `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Review.app`, separate identity `studio.loopflow.backlog-review.loo903`. The first bundle remains intact so its unsaved preview state is not discarded. The new preview adds drag handles/insertion markers, Move up/down equivalents and a compact scope/chapter/conversation line with objective disclosure. Selection stays independent of reordering. Launch identity and UI observations follow after opening.

## Revised native result

- Swift 6 build passed. Compact-header screenshot inspected; Product/chapter/conversation fit on one line, and objective disclosure expands/collapses.
- Native Move up action moved LOO-452 above LOO-456 while its full brief stayed selected. The footer reported local-only order change.
- Automated drag attempts did not change order. Their cause was not established. The custom gesture was replaced with SwiftUI native draggable/dropDestination, with insertion markers and an end-of-list target. Pointer drag still needs hands-on verification; do not report it as a passed native check.
- Before replacing the revised binary, the revised app was quit through its own UI and process exit was checked. The first preview was not quit. Both complete bundle paths remain available.
- Current preview process `24268`: `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Review.app/Contents/MacOS/BacklogPreview`. If normal Quit fails, verify this exact PID/path before `kill -TERM 24268`. Keep its runtime while it lives.
- Current preview process `46350`: `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Preview.app/Contents/MacOS/BacklogPreview`. If normal Quit fails, verify this exact PID/path before `kill -TERM 46350`. Keep its runtime while it lives.

## Expand-in-place revision, November 19

- The side reader is gone; `Loopflow Backlog Review.app` now shows one list whose rows open in place, with row selection and Start/Claim previews. The earlier binary in that bundle was replaced while neither preview was running.
- Build: `xcrun swiftc -swift-version 6 -O -parse-as-library -target arm64-apple-macos26.0 scratch/native-backlog-preview/BacklogPreview.swift` plus the six design sources under `swift/Loopflow/`, then `codesign --force --sign -` on the bundle.
- Current preview process `33353`: `/Users/jack/.lf/demos/loo-903-backlog/runtime/Loopflow Backlog Review.app/Contents/MacOS/BacklogPreview`. PIDs 24268 and 46350 above have exited.
- Seen in a window capture: a pointer drag in progress with its insertion marker, and a reordered list. Not exercised by an agent: opening briefs, editing, multi-select.

- Selection and actions revision runs as PID 89103 in `Loopflow Backlog Preview.app` (first bundle, binary replaced while it was not running); the expand-in-place build stays in `Loopflow Backlog Review.app`, PID 33353. Both windows carry the same title.

- November 19, later: the first bundle now runs the one-selection build (one selected Task shows its brief, several show none, floating action bar, All/Mine/Available). Its PID changes with each rebuild; find it with `pgrep -f "Backlog Preview.app"`.

- Nov 19 build 12: keys handled by one NSEvent local monitor in PreviewLibrary.handle; `screencapture -l <window id>` returns "could not create image from window" from this shell.
