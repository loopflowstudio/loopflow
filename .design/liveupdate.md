# Live Update

## What to build

Enhance Maestro SwiftUI app with live-refreshing worktree list and main branch protection.

**Location**: `loopflow.uiexplorations/Maestro/` (SwiftUI macOS app)

## User quotes

> "as soon as the wt remove happens we should somehow notify the maestro app, probably via lfd"

> "it's a little more robust to react to wt than lf since users could invoke wt manually"

> "the app is hierarchical, and the prompter is basically 'inside' the worktree, so which files specifically we are parsing is well defined"

> "the frontmatter is for `lf x` but the user's toggle should just override the default. However we can use the frontmatter to set the initial selection"

## Existing Architecture

```
Maestro/Maestro/
├── Views/
│   ├── WorktreeSidebar.swift    ← left panel, worktree list
│   ├── PromptLauncher.swift     ← right panel, task selection + run
│   └── ...
├── Models/
│   ├── Worktree.swift           ← worktree data model
│   └── PromptCard.swift         ← task with defaultMode
├── Services/
│   ├── WorktreeService.swift    ← calls `wt list`
│   └── TerminalLauncher.swift   ← AppleScript to terminals
└── AppState.swift               ← central state, runMode, selectedWorktree
```

**PromptLauncher** already:
- Has task selector with `defaultMode` from frontmatter
- Has Auto/Interactive segmented control bound to `appState.runMode`
- Builds command with explicit `-a`/`-i` via `appState.buildCommand()`
- Launches via `TerminalLauncher`

## Main Branch Protection

Never run prompts directly in main. When main is selected:

1. User clicks Run
2. Generate random worktree name from magical/musical words
3. Create worktree: `wt switch --create <random-name>`
4. Launch task in new worktree
5. UI refreshes worktree list, selects new worktree

```swift
// Maestro/Maestro/Services/NameGenerator.swift
let magicalWords = ["aurora", "cascade", "drift", "echo", "flume", "grove", ...]
let musicalWords = ["allegro", "cadence", "forte", "harmony", "lyric", "tempo", ...]

func generateWorktreeName() -> String {
    let magical = magicalWords.randomElement()!
    let musical = musicalWords.randomElement()!
    return "\(magical)-\(musical)"
}
```

**In PromptLauncher.launchInTerminal():**
```swift
// Check if launching from main
if appState.selectedWorktree == nil || appState.selectedWorktree?.branch == "main" {
    let name = generateWorktreeName()
    try await appState.createWorktree(name: name)
    // Select the new worktree
    appState.selectedWorktree = appState.worktrees.first { $0.branch == name }
}
// Then launch...
```

## Changes Required

### 1. Live worktree refresh (WorktreeSidebar.swift)

Currently `appState.refreshWorktrees()` is called manually. Need to poll or subscribe to changes.

**Option A: Poll** (simplest)
```swift
// In WorktreeSidebar or AppState
.task {
    while !Task.isCancelled {
        await appState.refreshWorktrees()
        try? await Task.sleep(for: .seconds(2))
    }
}
```

**Option B: File system events** (FSEvents)
Watch for changes to worktree directories.

**Option C: lfd subscription** (requires Python changes)
Connect to lfd Unix socket, subscribe to `worktree.*` events.

### 2. Main branch protection (PromptLauncher.swift)

Add to `launchInTerminal()`:
```swift
private func launchInTerminal() async {
    guard let repo = appState.currentRepo else { return }

    // Main branch protection
    let isMain = appState.selectedWorktree?.branch == "main"
              || appState.selectedWorktree == nil

    var workPath: URL
    if isMain {
        let name = NameGenerator.generate()
        do {
            try await appState.createWorktree(name: name)
            await appState.refreshWorktrees()
            appState.selectedWorktree = appState.worktrees.first { $0.branch == name }
            workPath = URL(fileURLWithPath: appState.selectedWorktree!.path)
        } catch {
            launchError = error.localizedDescription
            showingLaunchError = true
            return
        }
    } else {
        workPath = URL(fileURLWithPath: appState.selectedWorktree!.path)
    }

    // Rest of launch logic...
}
```

### 3. Name generator (new file)

```swift
// Maestro/Maestro/Services/NameGenerator.swift
enum NameGenerator {
    static let magical = [
        "aurora", "cascade", "crystal", "drift", "echo", "ember",
        "fern", "flume", "frost", "glade", "grove", "haze",
        "ivy", "jade", "luna", "mist", "nova", "opal",
        "petal", "prism", "rain", "ripple", "sage", "shade",
        "spark", "star", "stone", "storm", "tide", "vale",
        "wave", "wisp", "wren", "zephyr"
    ]

    static let musical = [
        "allegro", "aria", "ballad", "cadence", "canon", "chord",
        "coda", "duet", "forte", "fugue", "harmony", "hymn",
        "lilt", "lyric", "melody", "motif", "opus", "prelude",
        "refrain", "rondo", "sonata", "tempo", "trill", "tune",
        "verse", "waltz"
    ]

    static func generate() -> String {
        let m = magical.randomElement()!
        let n = musical.randomElement()!
        return "\(m)-\(n)"
    }
}
```

## Launch flow (already works)

```
1. User selects worktree in sidebar
2. User selects task → appState.runMode = prompt.defaultMode
3. User may flip Auto/Interactive toggle
4. User clicks Run:
   - If main: create new worktree with random name, refresh, select it
   - Launch: `lf <task> -a` or `lf <task> -i` (explicit flag)
```

The `-a`/`-i` flag is already explicit in `buildCommand()`.

## Live updates (optional enhancement)

**Option A: Simple polling** in WorktreeSidebar
- Poll every 2 seconds
- Minimal code change
- Good enough for now

**Option B: worktrunk hooks + lfd** (future)
User hook in `~/.config/worktrunk/config.toml`:
```toml
[pre-remove]
notify = "lf notify worktree-changed removed '{{ worktree_path }}' '{{ branch }}' || true"
```
Then Maestro subscribes to lfd socket for instant updates.

## Constraints

- **Never run in main**: Launching from main creates a new worktree first
- **Explicit flags**: `buildCommand()` already passes `-a` or `-i`
- **Worktree-scoped**: Tasks come from selected worktree's `.lf/`

## Open questions

1. Polling interval for live refresh? (2s feels responsive, 5s is lighter)
2. Should we add FSEvents watching instead of polling?

## Done when

```
# 1. Worktree list updates after wt remove
wt remove some-feature
# → Within 2-5 seconds, Maestro UI removes it from sidebar

# 2. Auto/Interactive toggle already works
# (verify: select task, check toggle matches defaultMode, flip it, run, see correct flag)

# 3. Main branch protection
# In Maestro: no worktree selected (or main selected) → select task → click Run
# → New worktree created (e.g., "aurora-cadence")
# → Sidebar shows new worktree, selected
# → Task runs in new worktree
```
