# Mobile: Stage 01 done, Stage 02 next

Branch `jack-heart.mobile.20260224_2118`. Stage 01 multiplatform cleanup is complete. Stage 02 (action buttons) is designed but not yet implemented.

## Stage 01 summary

Built on PR #410. Key changes:

1. **macOS file migration** — ~40 files with whole-file `#if os(macOS)` guards moved to `Concerto/Platform/macOS/`. Guards removed. `project.yml` destination filters restrict to macOS builds.

2. **iOS view polish** — `MobileWaveDetailView` and `MobileWaveListView` cleaned up. Unnecessary `Group` wrappers removed (safe on iOS 18+). Unused `palette` env var removed from `ConnectionSetupView`.

3. **LoopflowCore state refactoring** — `ConnectionStore.loadInitialState` extracted `bundledState()` and `remoteState()` helpers. `OutputBuffer` extracted `cancelStream(for:)`, fixing a leak where `streamGeneration` entries weren't cleaned up on manual stop/clear.

4. **CI hardening** — `CODE_SIGNING_REQUIRED=NO` added to xcodebuild UI test invocation. Boundary check script simplified.

5. **Mixed-platform files left in place.** `LiveOutput.swift` and `WaveChatView.swift` have partial `#if os(macOS)` guards (not whole-file). Splitting adds complexity for no benefit now.

6. **ConnectionProfile dropped.** Prototyped and removed. `ConnectionStore` with `ConnectionMode` is sufficient for single-connection use. Stage 03 revisits if needed.

### Architecture

```
swift/
  LoopflowCore/          # shared state (ConnectionStore, OutputBuffer, RepoState)
  Concerto/
    Platform/iOS/         # purpose-built iOS views
    Platform/macOS/       # macOS-only views, services, keyboard handling
    Views/                # shared views (LiveOutput, WaveChatView)
```

### Verification

| Check | Result |
|-------|--------|
| `swift test --package-path swift` | 7/7 passed |
| `uv run python scripts/check_swift_multiplatform_boundaries.py` | passed |
| `scripts/concerto-dev.py run-ios` | builds and launches in Simulator |

### Gap

No interactive iOS tap-through against live `lfd`. Build path confirmed on simulator but headless environment can't exercise UI interaction. Manual testing or a UI automation target needed.

## Rebase blocked

Automated rebase aborted — conflicts are structural.

**Root cause:** PR #410 merged a large subset of this branch's work into main. Main then evolved those files. Commit-by-commit rebase of 25 commits hits cascading conflicts across three categories:

1. **macOS file locations** — This branch has ~40 files in `Concerto/Platform/macOS/`. Main has them in `Concerto/` root (only 2 in `Platform/macOS/`). Architectural decision needed.
2. **wave/mobile/ docs** — Main has post-ship versions. The branch has 15+ intermediate versions. Net diff is small but every intermediate commit conflicts.
3. **LoopflowCore state files** — `ConnectionStore.swift`, `OutputBuffer.swift`, `RepoState.swift` diverged. Net diff ~100 lines.

**Net divergence:** 73 files, 441 insertions, 597 deletions. Most deletions are `#if os(macOS)` guard removal (×40 files).

**Recommended approach:** Fresh branch from `origin/main`, manually apply net delta. The 25-commit history is iterative work that PR #410 already landed.

## Stage 02: action buttons

Make mobile follow-up turns tap-first via `suggest_actions`.

### Implementation plan

1. Add `SuggestedAction` in `LoopflowCore` and `ChatState.suggestedActions`.
2. Parse `tool_use` items where `name == "suggest_actions"`.
3. Enforce lifecycle rules:
   - clear on user send
   - clear on composer typing start
   - clear on agent turn start
   - clear on session/connection reset
4. Add shared `ActionButtonsView` in `LoopflowCore`.
5. Embed in both chat surfaces:
   - iOS `MobileWaveDetailView`
   - macOS `WaveChatView`
6. Tap behavior: send button `label` exactly as user message; clear optimistically on tap.

### Constraints

- No new wire format.
- Stage 02A first: prompt/tool convention; 02B tool registration in `lfd` after 02A works.
- Max 4 rendered actions; malformed entries ignored.

### Done when

- Buttons render on iPhone, iPad, and Mac from `suggest_actions` tool calls.
- Tapping sends the label as the next user message.
- Buttons clear on tap/typing/new agent turn.
- Regression checks pass (`swift test`, boundary check script).

## Canonical specs

- `wave/mobile/01-multiplatform.md`
- `wave/mobile/02-action-buttons.md`
- `wave/mobile/03-multi-client.md`
- `wave/mobile/04-lfd-discovery.md`
