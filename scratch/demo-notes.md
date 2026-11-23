# Demo notes · LOO-1064

## Notes from Jack Heart (2026-11-22)

- Jack Heart: "should the loading indicator go in the middle of hte pane and be bigger/"
- Jack Heart: "what would the best designed apps do"
- Jack Heart: "yes" (in response to: delay spinner ~150–200ms and center regular-size on empty panes; keep retained screen clear of spinner overlays and slightly dimmed while showing progress in the pane strip)
- Jack Heart: "the double spinner w the refersh button is a bit weird"
- Jack Heart: "there are also some jumpy times when i click between sessions where it switches to showing just a single word or something and then switches back"
- Jack Heart: "inside the terminal"
- Jack Heart: "its when i click between tasks so i think it has to do with some sort of jumping between the terminal holders or some terminal reload"
- Jack Heart: "1" (commit the single-spinner fix on LOO-1064, close Loopflow Dev, and run `deliver`)

## Context

`SessionOpeningView` in `swift/LoopflowMac/Views/WorkspacePanesView.swift` kept `.topLeading` alignment, `.controlSize(.small)`, and `.background(palette.background)` from when it displayed `ProgressView("Opening… Input paused.")`. Every other full-pane loader in Desktop (`SessionsView`, `TaskFilesView`, `WorkSurfaceView`) uses a centered default-size `ProgressView()`.
