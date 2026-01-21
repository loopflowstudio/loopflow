# Concerto — macOS App

Visual interface for loopflow. SwiftUI, requires macOS 15+.

## Architecture

- AppState.swift — Central observable state
- Services/ — Data loading, no UI
- Views/ — SwiftUI views
- Models/ — Swift structs mirroring Python dataclasses

## Communication with lfd

Two patterns, intentionally different:

1. **Direct DB reads** (SessionService.swift)
   - Reads ~/.lf/lfd.db directly via SQLite
   - Used for history queries
   - Works even if daemon isn't running
   - Simpler than socket for read-only data

2. **Socket subscription** (LFDEventService.swift)
   - Connects to ~/.lf/lfd.sock
   - Subscribes to events (session.*, agent.*, worktree.*)
   - Used for live UI updates
   - Reconnects on failure

## Why Both?

Direct DB reads mean Concerto can show history even if lfd crashed.
Socket events provide real-time updates without polling.

## Build

Open LoopflowSwift.xcodeproj in Xcode, build and run.
Distribution build: Archive → export as App.

## UI Tests

Generate the Xcode project if needed:
```bash
xcodegen generate
```

Run UI tests from the command line:
```bash
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'
```
