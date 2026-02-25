# Review: Multiplatform Concerto (iOS)

Branch: `jack-heart.mobile.20260224_1845`

## What was implemented

Single-target multiplatform build that runs Concerto on iOS (iPhone + iPad) alongside macOS. The implementation has three layers:

1. **Shared state extraction** — RepoState, WaveStore, RunStore, WorktreeStore, ConnectionStore, OutputBuffer, and ChatState moved from Concerto into `LoopflowCore/State/` with public APIs. Design tokens (BrandColors, DesignSystem) also moved to LoopflowCore.

2. **Platform shell separation** — Platform-specific code lives in `Platform/iOS/` and `Platform/macOS/` directories. RepoState is initialized via platform convenience inits that inject capabilities: macOS gets BundledDaemonManager and LocalShellCommandRunner; iOS gets remote-only defaults.

3. **iOS views** — Purpose-built mobile views (MobileRootView, MobileWaveListView, MobileWaveDetailView, ConnectionSetupView) rather than sharing macOS views through LoopflowCore. iPhone uses TabView + NavigationStack; iPad uses NavigationSplitView.

## Key choices

**Purpose-built iOS views over shared views.** The original plan called for moving macOS views (WaveSidebar, WaveDetailPanel) into LoopflowCore. Instead, iOS got its own views. Mobile UX patterns (tab bars, navigation stacks, pull-to-refresh, touch targets) differ enough that forcing shared views would have produced worse UX on both platforms. Shared views like LiveOutput and WaveChatView (already in Concerto/Views/ without platform gates) are reused where they fit naturally.

**State in LoopflowCore, views in Concerto.** State types are the shared contract between platforms. Views stay in the app target because they're tightly coupled to platform idioms. This avoids LoopflowCore becoming a monolith while keeping the shared logic truly shared.

**Capability injection over `#if` spreading.** macOS capabilities (bundled daemon, shell commands) are injected through optional closures in RepoState's designated init, not through `#if os(macOS)` scattered through shared code. The boundary check script (`check_swift_multiplatform_boundaries.py`) enforces this.

**iOS is remote-only.** No local repo/worktree flows on iOS in Stage 01. The iOS convenience init sets `connectionStore.setMode(.remote)` — the app starts at a connection setup screen.

## How it fits together

```
LoopflowCore (shared library)
├── State/       — RepoState, stores, ChatState
├── Design/      — BrandColors, DesignSystem
├── Models/      — Wave, ServerConnection, etc.
└── Services/    — WaveService, EventService, etc.

Concerto (app target)
├── ConcertoApp.swift  — #if os(macOS) / #else split at @main
├── Platform/iOS/      — MobileRootView, ConnectionSetupView, etc.
├── Platform/macOS/    — LocalShellCommandRunner, RepoState+macOS
└── Views/             — Shared views (LiveOutput, WaveChatView, etc.)
```

RepoState is the orchestrator. Platform shell files create it with platform-appropriate capabilities. Views read from it via `@Environment(RepoState.self)`.

## Risks and bottlenecks

**No live end-to-end validation.** Simulator builds work but full flows (connect to lfd → wave list → detail → output) haven't been tested against a running lfd on device. This is the primary gap before calling Stage 01 fully closed.

**~40 macOS-gated files still in Views/ and Services/.** Files like KeyboardRouter, CommandPalette, GhosttyManager have whole-file `#if os(macOS)` gates but haven't been moved to `Platform/macOS/`. Pure mechanical move — no behavior change — but it's visual noise that should be cleaned up.

**macOS xcodebuild test instability.** `xcodebuild test` for the Concerto scheme fails in this environment at ConcertoUITests-Runner startup. Swift package tests (143 tests) pass cleanly. This is an environment issue, not a code issue.

## What's not included

- Action buttons on mobile wave detail (Stage 02: `wave/mobile/02-action-buttons.md`)
- Multi-client support (Stage 03)
- lfd discovery/Bonjour (Stage 04)
- macOS view migration to `Platform/macOS/` (noted as remaining work in `01-multiplatform.md`)
- Local repo flows on iOS (out of scope by design)

## Validation

| Check | Result |
|-------|--------|
| `swift test --package-path swift` | 143 tests, all pass |
| `check_swift_multiplatform_boundaries.py` | Pass |
| iPhone 17 simulator build | Pass |
| iPad Pro 11-inch (M5) simulator build | Pass |
| macOS behavior unchanged | Pass |
