# Stage 01: Multiplatform Concerto

Design doc for the first stage of the mobile wave. See `wave/mobile/01-multiplatform.md` for full spec.

## What to build

Make Concerto build and run on iOS. One target, three form factors. LoopflowCore absorbs everything shared — state (including RepoState), views, models, services. Concerto becomes a thin macOS app shell.

## Key decisions

- **1 target, not 3**: Concerto is a single multiplatform app. `#if os(macOS)` / `#if os(iOS)` where UX diverges.
- **LoopflowCore gets everything shared**: Models, services, state (RepoState, WaveStore, ChatState, etc.), and reusable views. Both Concerto and Symphonia (separate repo) depend on it.
- **RepoState moves to LoopflowCore**: It's the orchestrator both apps need. No macOS imports — macOS-specific stuff is in the views, not in RepoState.
- **iOS is always a remote client**: No local git repos on phone. "Open repo" becomes "connect to lfd."
- **iPhone layout**: Tab bar (Waves / Settings). Wave list → push to detail. ChatGPT-style navigation.
- **iPad layout**: NavigationSplitView like Mac, minus terminal and keyboard router.
- **No embedded terminal on iOS**: Ghostty is macOS-only. Mobile gets output view.

## Implementation order

1. Package.swift: add `.iOS(.v18)`, platform-gate Ghostty and macOS linker settings
2. Move state types to LoopflowCore (RepoState, WaveStore, OutputBuffer, ChatState, ConnectionStore) — make public
3. Move shared views to LoopflowCore (with font resources, DesignSystem, BrandColors)
4. Platform-gate macOS code: `#if os(macOS)` around Carbon imports, KeyboardRouter, NSOpenPanel, Ghostty, TerminalLauncher, ScreenshotWindow
5. ConcertoApp: `#if os(macOS)` / `#else` for window setup
6. New iOS views: MobileRootView (tab bar), MobileWaveListView, MobileWaveDetailView, ConnectionSetupView
7. iPad adaptations: size class checks for sidebar vs stack navigation
8. Verify Mac still works — all tests pass

## Constraints

- Mac behavior must not change
- LoopflowCore must not import macOS-specific frameworks

## Done when

- `swift build` succeeds for iOS target
- iPhone simulator: connection screen → connect to lfd → wave list → wave detail
- iPad simulator: sidebar + detail layout
- Mac: all existing tests pass, no behavior change
