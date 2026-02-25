# Gate review: jack-heart.mobile.20260224_2118

## What was implemented

Stage 01 multiplatform cleanup, building on PR #410:

1. **macOS file migration** — ~40 files with whole-file `#if os(macOS)` guards moved from `Concerto/` root to `Concerto/Platform/macOS/`. Guards removed. `project.yml` destination filters restrict them to macOS builds.

2. **iOS view polish** — `MobileWaveDetailView` and `MobileWaveListView` cleaned up: unnecessary `Group` wrappers removed (safe on iOS 18+), unused `palette` env var removed from `ConnectionSetupView`.

3. **LoopflowCore state refactoring** — `ConnectionStore.loadInitialState` extracted `bundledState()` and `remoteState()` helpers, reducing duplication across 4 legacy migration paths. `OutputBuffer` extracted `cancelStream(for:)` which also fixes a minor leak where `streamGeneration` entries weren't cleaned up on manual stop/clear.

4. **CI hardening** — Added `CODE_SIGNING_REQUIRED=NO` to xcodebuild UI test invocation (CI + TESTING.md). Simplified boundary check script by hardcoding `MAIN_REF = "main"`.

5. **Wave doc updates** — `01-multiplatform.md`, `03-multi-client.md`, and `README.md` updated with Stage 01 outcomes, lessons learned, and resolved risk items.

## Key choices

- **Mixed-platform files left in place.** `LiveOutput.swift` and `WaveChatView.swift` have partial `#if os(macOS)` guards (not whole-file). Moving them would require splitting, which adds complexity for no current benefit.

- **ConnectionProfile dropped.** Prototyped in Stage 01 and removed. `ConnectionStore` with `ConnectionMode` (bundled/remote) is sufficient for single-connection use. Documented in wave docs for Stage 03 to revisit if needed.

- **`streamGeneration` cleanup in `cancelStream`.** Previously `clearOutput` and `stopStreaming` cancelled tasks and removed them from `streamTasks` but left stale `streamGeneration` entries. Now cleaned up properly. Safe because the generation counter is monotonically increasing — old task defers check `==` on their own generation, which no longer exists.

## How it fits together

```
swift/
  LoopflowCore/          # shared state (ConnectionStore, OutputBuffer, RepoState)
  Concerto/
    Platform/iOS/         # purpose-built iOS views
    Platform/macOS/       # macOS-only views, services, keyboard handling
    Views/                # shared views (LiveOutput, WaveChatView)
```

`project.yml` destination filters ensure `Platform/macOS/` sources only compile for macOS. `scripts/check_swift_multiplatform_boundaries.py` enforces at CI time that macOS-only imports don't leak into shared code.

## Risks and bottlenecks

- **No interactive iOS validation.** Build path confirmed on simulator but tap-through against live `lfd` is blocked on headless simulator interaction. Manual testing or a UI automation target needed before full confidence.

- **Rebase conflict.** This branch has a documented rebase blocker against main (see `scratch/questions.md`). Recommended path: fresh branch from `origin/main` with cherry-picked delta, rather than commit-by-commit rebase through 25 cascading conflicts.

## What's not included

- Stage 02 (action buttons) — designed in `scratch/` but not implemented on this branch.
- Interactive end-to-end iOS testing against live `lfd`.
- Reconnect handling or background lifecycle (Stage 03 scope).

## Verification

| Check | Result |
|-------|--------|
| `swift test --package-path swift` | 7/7 passed |
| `uv run python scripts/check_swift_multiplatform_boundaries.py` | passed |
| No TODOs, debug prints, or FIXMEs in diff | confirmed |
| Style guide compliance (CLAUDE.md) | no violations |
| TESTING.md matches CI workflow | in sync |
