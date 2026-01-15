# LLM-Powered Compare in Maestro

Add LLM analysis to Maestro's worktree comparison.

## Context

This branch updated Maestro to use `lfops` commands. Maestro already has:
- `DiffSheet`: Shows git diff for a single worktree
- `CompareSheet`: Shows git diff between two worktrees
- `WorktreeService.getDiff()`: Fetches raw diff

Meanwhile, the CLI has `lfwt compare` which launches an LLM session to analyze two worktrees and write a recommendation to `.design/comparison.md`.

Gap: Maestro shows raw diffs but doesn't offer LLM analysis.

## Expansion

Add an "Analyze with LLM" button to CompareSheet that:

1. Runs `lfwt compare <a> <b> --print` in the background
2. Shows a loading state while analysis runs
3. Displays the resulting comparison inline

This extends the comparison feature from "see what's different" to "understand which is better."

## Implementation

### CompareSheet changes

Add state for analysis:

```swift
@State private var analysisContent: String?
@State private var isAnalyzing = false
```

Add button in the sheet:

```swift
Button {
    analyzeWithLLM()
} label: {
    Label("Analyze", systemImage: "sparkles")
}
.disabled(isAnalyzing)
```

Add analysis display (below or instead of raw diff):

```swift
if let analysis = analysisContent {
    MarkdownView(content: analysis)
}
```

### WorktreeService extension

Add `compareWithLLM()`:

```swift
func compareWithLLM(_ a: String, _ b: String, in repoURL: URL) async throws -> String {
    // Run lfwt compare in headless mode
    let process = Process()
    process.executableURL = findCommand("lfwt")
    process.arguments = ["compare", a, b, "--print", "--output", "/dev/stdout"]
    // ...capture output...
}
```

Wait—this won't work directly. `lfwt compare --print` runs Claude Code which expects a TTY. The output goes to the daemon's collector.

Better approach: read the output file after completion.

```swift
func compareWithLLM(_ a: String, _ b: String, in repoURL: URL) async throws -> String {
    let outputPath = repoURL.appendingPathComponent(".design/comparison.md")

    let process = Process()
    process.executableURL = findCommand("lfwt")
    process.arguments = ["compare", a, b, "--print"]
    process.currentDirectoryURL = repoURL

    try process.run()
    process.waitUntilExit()

    guard process.terminationStatus == 0 else {
        throw WorktreeError.commandFailed("lfwt compare failed")
    }

    return try String(contentsOf: outputPath)
}
```

### UX flow

1. User opens CompareSheet (two worktrees selected)
2. Raw diff displays immediately
3. User clicks "Analyze" button
4. Spinner shows, button becomes "Analyzing..."
5. On completion, analysis appears below the diff
6. Analysis includes recommendation from `.design/comparison.md`

## Scope

Single feature: add LLM analysis to CompareSheet.

- 1 new service method
- 1 new state variable + button in CompareSheet
- ~50 lines of Swift

## What this enables

Instead of manually reading diffs to decide which worktree to keep, users get an LLM recommendation. This is especially useful when:
- Two agents solved the same problem differently
- A branch has been iterated and you want to compare with the original
- Reviewing before merging

## Open questions

1. **Output location**: `lfwt compare` writes to `.design/comparison.md` in the repo. Should we clean this up after display? Or leave it for reference?

2. **Model selection**: `lfwt compare` uses the configured model. Should Maestro expose model selection for ad-hoc comparison? (Probably not for v1—use config.)

3. **Progress feedback**: The analysis may take 30-60 seconds. Should we show daemon output during analysis? (Nice to have, not blocking.)

---

## Implementation complete

Added:
- `WorktreeService.compareWithLLM()` - runs `lfwt compare --print` and reads result
- `CompareSheet.onAnalyze` - optional closure for triggering analysis
- Analysis UI: loading state, error display, Diff/Analysis view toggle
- Analyze button in CompareSheet action bar (only shows when `onAnalyze` is provided)

Build verified: Maestro compiles successfully. Python tests pass.
