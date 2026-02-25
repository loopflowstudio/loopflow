# Stage 01: Multiplatform Concerto

## Problem

Concerto is macOS-only today, so waves become invisible the moment you leave your desk. Stage 01 makes Concerto truly multiplatform without splitting the product into separate codebases.

This design directly advances wave/mobile goals:

- "One Concerto target, three form factors (Mac unchanged, iPad close to Mac, iPhone minimal)"
- "LoopflowCore as shared library: models, services, and reusable views — consumed by Concerto and Symphonia"

Who benefits now:

- **Mac users** keep current behavior while future mobile work lands on shared code.
- **iPhone users** can check wave status/output and take lightweight actions anywhere.
- **iPad users** get near-desktop information density with touch-safe interaction.

Why now: Stage 02 (action buttons) and Stage 03 (multi-client continuity) both depend on shared chat/state/view infrastructure. Without this extraction first, later stages duplicate work.

## Approach

Build this as a **core-first extraction with platform shells** (not a parallel iOS app).

1. **Make the package truly multiplatform**
   - Add `.iOS(.v18)` to `swift/Package.swift`.
   - Gate Ghostty dependency, `GHOSTTY_ENABLED`, and Carbon/Metal/IOKit linker settings to macOS only.
   - Keep one `Concerto` target and one scheme.

2. **Move shared state into LoopflowCore with explicit platform capabilities**
   - Move to `swift/LoopflowCore/State/`: `RepoState`, `WaveStore`, `RunStore`, `WorktreeStore`, `OutputBuffer`, `ChatState`, `ConnectionStore`, `ConnectionMode`.
   - Make shared types/public initializers explicit so both Concerto and Symphonia can construct them.
   - Remove hard dependency on `BundledDaemonManager` from `RepoState` by injecting a runtime capability:
     - macOS shell injects bundled-daemon support.
     - iOS shell injects remote-only behavior.
   - Result: `RepoState` stays the orchestrator, but LoopflowCore has no macOS framework dependency.

3. **Move reusable UI primitives + shared wave UI to LoopflowCore**
   - Move design tokens/resources (`BrandColors`, `DesignSystem`, `Typography`, palette environment).
   - Move shared wave/chat/output views that do not require AppKit-only APIs.
   - Replace direct platform calls (for example PR opening) with injected actions or SwiftUI environment actions so the same view compiles on both platforms.

4. **Create platform-specific shells in Concerto**
   - **macOS shell**: preserve current multi-window architecture (`PortfolioWindow`, `RepoWindow`, command palette, keyboard router, Ghostty).
   - **iOS shell**: add `MobileRootView`:
     - iPhone: `TabView` (Waves, Settings) + `NavigationStack` list→detail.
     - iPad: `NavigationSplitView` with no terminal/command palette/keyboard router.
   - Add `ConnectionSetupView` for iOS remote connection + saved profiles (no local repo picker).

5. **Resource and behavior parity pass**
   - Ensure bundled fonts are available to LoopflowCore-rendered views on both platforms.
   - Keep mac behavior unchanged while introducing iOS-only code paths behind platform checks.

Research patterns applied:
- Single shared state + platform-specific scenes (Apple multiplatform baseline).
- iPhone list→detail with tab root (ChatGPT/Claude mobile navigation pattern).
- Capability injection over `#if` sprawl for platform-only behavior.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate `ConcertoMobile` app target | Fastest short-term iOS launch | Duplicates state/views and fights the goal of one target + shared core |
| Keep RepoState in Concerto, only add iOS views | Lowest refactor risk | Blocks Symphonia reuse and makes Stage 02/03 branch into platform forks |
| Big-bang full move in one PR | Fewer intermediate states | High regression risk on macOS; impossible to isolate failures cleanly |

## Key decisions

- **Choose one target with shell branching, not separate apps.** Product stays coherent and maintainable.
- **RepoState moves to LoopflowCore now.** It is the seam that unlocks reusable chat/wave behavior.
- **Use injected capabilities for bundled daemon and external actions.** Avoids leaking AppKit/Process assumptions into core.
- **iOS is remote-client only in Stage 01.** No local file picker, no local git/worktree management.
- **Phone is intentionally minimal.** Status/output/chat entry points only; no step runner/typeahead/embedded terminal.
- **Preserve wave/mobile “Not here” constraints.** Explicitly exclude embedded terminal, phone step runner/typeahead, offline mode, App Store packaging.

Wild success details we are designing for:
- User starts a wave on Mac, opens iPhone, and sees live wave/output within one connection flow.
- iPad feels like “Mac without keyboard tricks,” not “blown-up phone.”
- Stage 02 action buttons drop into shared chat UI without another architecture rewrite.

Wild failure to avoid (from wave risks):
- Platform-gating turns into scattered `#if` churn across every file.
- View extraction drags app-level dependencies into LoopflowCore.
- Navigation mismatches between iPhone/iPad/macOS create three divergent products.

New risk introduced here:
- Font/resource packaging can silently regress when views move modules. Mitigate with explicit resource wiring and simulator checks on both iOS form factors.

## Scope

- In scope:
  - Package/platform gating for iOS build support.
  - State extraction to LoopflowCore with public APIs.
  - Shared view/design-system extraction.
  - iOS root navigation + connection setup + wave list/detail.
  - macOS behavior parity validation.
- Out of scope:
  - `suggest_actions` rendering/behavior (Stage 02).
  - Multi-client protocol or lfd concurrency changes (Stage 03).
  - Embedded terminal on iOS.
  - Phone step runner/typeaheads.
  - Offline mode or App Store distribution.

## Done when

- Builds pass:
  - `swift test --package-path swift`
  - `cd swift && xcodegen generate && xcodebuild -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=iOS Simulator,name=iPhone 16' build`
  - `cd swift && xcodebuild -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=iOS Simulator,name=iPad Pro (11-inch) (M4)' build`
- iPhone simulator flow works end-to-end: connection setup → connect to lfd → wave list → wave detail/output.
- iPad simulator shows split layout with touch-safe interactions and no terminal/command palette.
- macOS workflow remains unchanged (existing windows, keyboard router, terminal integration).
