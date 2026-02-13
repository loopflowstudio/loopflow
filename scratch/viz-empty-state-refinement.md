# Empty State Refinement

## Problem

Empty states are the first thing users see. Concerto has five distinct empty states, each built independently with inconsistent treatment. Three of them are actively confusing.

**Sidebar empty waves** (`WaveSidebar.swift:201`) is overloaded. For a 280px sidebar, it stacks: QuickExperimentSidebarView (2x2 grid) + divider + Create Wave button + SidebarPreviewView (mock section headers). Four visual ideas fighting for a narrow column. The mock section headers (Needs Attention, Open PRs, Active, Idle) with colored status icons look interactive but aren't — users might think they're broken UI. Meanwhile the disconnected state right above it is clean: icon + title + subtitle + button. The empty waves state should match that pattern.

**Detail panel no selection** (`QuickExperimentView.swift:100`) has an incoherent bottom section. "Select a wave from the sidebar" appears even when the sidebar is empty — there are no waves to select. The step cards use `sectionTitle()` + `.semibold` at 120px wide, making them feel like primary destinations rather than lightweight options.

**Runs tab empty** (`WaveRunsTab.swift:45`) — just "No runs yet" with no context. Doesn't explain what a run is or how one starts.

**Symphonia placeholder** (`PlaceholderView.swift`) — hardcoded system fonts (`.system(size: 64)`, `.largeTitle`, `.title3`, `.headline`) and literal spacing (`24`, `16`). Completely outside the design system.

## Approach

Two PRs. Each changes one conceptual layer.

### PR 1: Sidebar empty state simplification

Replace the overloaded sidebar empty state with the disconnected state's pattern: icon + title + subtitle + button.

```swift
private var emptyState: some View {
    VStack(spacing: 0) {
        Spacer()
            .frame(maxHeight: .infinity)

        VStack(spacing: Spacing.md) {
            Image(systemName: "wave.3.right")
                .font(Typography.heroTitle(28))
                .foregroundStyle(.white.opacity(0.3))

            VStack(spacing: Spacing.xs) {
                Text("No waves yet")
                    .fontWeight(.medium)
                    .foregroundStyle(.white.opacity(0.7))
                Text("Waves track ongoing branches, PRs, and runs.")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.5))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, Spacing.lg)
            }

            Button {
                createWaveDirectly()
            } label: {
                Label("Create Wave", systemImage: "plus")
                    .font(Typography.caption())
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .disabled(isCreatingWave)
        }

        Spacer()
            .frame(maxHeight: .infinity)
    }
    .frame(maxWidth: .infinity)
    .padding()
}
```

**Delete these structs** from `QuickExperimentView.swift`:
- `QuickExperimentSidebarView` (lines 22-59) — only called from sidebar empty state + its own preview
- `SidebarPreviewView` (lines 64-95) — only called from sidebar empty state + its own preview
- Their `#Preview` blocks (lines 195-211)

**Remove the `ScreenshotWindow` reference** if it uses `QuickExperimentSidebarView`. Check `ScreenshotWindow.swift` before deleting — it may reference the sidebar view for screenshot generation. If so, use the simplified empty state pattern instead.

### PR 2: Detail panel + minor empty states

**Detail panel step cards** — reduce visual weight so they feel like options, not destinations:

| Property | Before | After |
|----------|--------|-------|
| Step name font | `Typography.sectionTitle()` + `.semibold` | `Typography.body()` + `.medium` |
| Card width | `120` fixed | `100` fixed |
| Card padding | `Spacing.lg` vertical | `Spacing.md` vertical |
| Card background | `palette.surface` | `palette.surface.opacity(0.7)` |

**Detail panel bottom section** — fix the "select a wave" hint that appears even when no waves exist. Make it conditional:

```swift
// Replace the static "Select a wave from the sidebar" section with:
VStack(spacing: Spacing.sm) {
    if repoState.waves.isEmpty {
        Text("Or create a wave for ongoing work")
            .font(Typography.body())
            .foregroundStyle(.secondary)
    } else {
        Text("Select a wave from the sidebar")
            .font(Typography.body())
            .foregroundStyle(.secondary)
    }

    Text("Waves track ongoing work with branches, PRs, and history")
        .font(Typography.caption())
        .foregroundStyle(.tertiary)
}
```

**Runs tab empty** — add a one-line explanation:

```swift
VStack(spacing: Spacing.xs) {
    Text("No runs yet")
        .font(Typography.caption())
        .foregroundStyle(.secondary)
    Text("Runs appear here when the wave executes a flow.")
        .font(Typography.caption(10))
        .foregroundStyle(.tertiary)
}
```

**Symphonia placeholder** — migrate to design tokens:

```swift
VStack(spacing: Spacing.xxl) {
    Image(systemName: "person.3.fill")
        .font(Typography.heroTitle(48))
        .foregroundStyle(.secondary)

    Text("Loopflow Symphonia")
        .font(Typography.heroTitle())
        .fontWeight(.bold)

    Text("Teams coordination for LLM coding waves")
        .font(Typography.body())
        .foregroundStyle(.secondary)

    Text("Coming soon")
        .font(Typography.caption())
        .foregroundStyle(.tertiary)
        .padding(.top, Spacing.lg)
}
.frame(maxWidth: .infinity, maxHeight: .infinity)
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep Quick Experiment in sidebar, just shrink it | Less change, preserves discoverability | The detail panel already has Quick Experiment prominently. Duplication in a 280px column creates noise at first impression. One CTA is enough. |
| Remove Quick Experiment from detail panel too | Simpler empty state everywhere | Quick Experiment is genuinely useful as a detail-panel no-selection state. It gives users something to do immediately. Keep it where there's room for it. |
| Add illustrations/art to empty states | More polished feel | Out of scope — custom illustrations are a separate effort. The icon + text pattern is clean and consistent. Ship this, add art later if needed. |
| Make "Select a wave" always show (ignore empty sidebar) | Simpler code | Actively misleading when sidebar is empty. Users shouldn't be told to do something impossible. |

## Key decisions

**One pattern for sidebar empty states.** The disconnected state already nails it: icon + title + subtitle + action. The empty waves state should mirror that exactly. From the wave direction: "Constraints are friends. The simplest thing that could work."

**Delete, don't hide.** `QuickExperimentSidebarView` and `SidebarPreviewView` have no other call sites. Delete the structs and their previews entirely. Dead code is a liability.

**Conditional detail panel hint.** "Select a wave from the sidebar" is wrong when no waves exist. "Create a wave for ongoing work" guides correctly. This is a 3-line conditional, not over-engineering.

**Two PRs, not one.** Sidebar simplification is about first impressions (what new users see). Detail + minor states are about polish (what returning users experience). Separate concerns, separate reviews.

## Scope

- In scope: Sidebar empty state simplification, struct deletion, detail panel card density, conditional hint text, runs tab empty message, Symphonia design tokens
- Out of scope: Welcome window, setup view, command palette, custom illustrations, new empty state animations

## Done when

- Sidebar empty state shows icon + "No waves yet" + subtitle + Create Wave button
- `QuickExperimentSidebarView` and `SidebarPreviewView` deleted from `QuickExperimentView.swift`
- Detail panel step cards use `Typography.body()` + `.medium` at 100px width
- Detail panel hint is conditional on whether waves exist
- Runs tab empty state has explanatory subtitle
- Symphonia placeholder uses `Typography` and `Spacing` tokens
- `swift build --package-path swift` passes
