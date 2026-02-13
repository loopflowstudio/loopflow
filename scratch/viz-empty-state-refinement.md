# Empty State Refinement

## Problem

Empty states are the first thing users see. Concerto has four distinct empty states, each built independently with inconsistent treatment:

1. **Sidebar disconnected** (`WaveSidebar.swift:156`) — shown when lfd isn't running. Icon + text + "Connect lfd" button. Reasonable but plain.

2. **Sidebar empty waves** (`WaveSidebar.swift:201`) — shown when lfd is connected but no waves exist. Contains QuickExperimentSidebarView (2x2 step grid) + divider + Create Wave button + SidebarPreviewView (mock section list). Busy — three distinct sections competing in a narrow sidebar column.

3. **Detail panel no selection** (`QuickExperimentView.swift:100`) — full placeholder when no wave is selected. Hero icon + "Quick Experiment" title + 4 step buttons + "or" divider + "Select a wave" hint. Well-structured but the step buttons are large cards that dominate.

4. **Runs tab empty** (`WaveRunsTab.swift:45`) — "No runs yet" in caption text. Minimal but gives no guidance.

5. **Symphonia placeholder** (`PlaceholderView.swift:5`) — uses system fonts and hardcoded spacing. Not design-system-aware.

### Specific issues

- **Sidebar empty state is overloaded.** Quick experiment grid + divider + create button + preview mock. For a 280px sidebar, that's four visual ideas stacked vertically. Linear's empty states: one sentence, one action.

- **SidebarPreviewView is confusing.** Shows mock section headers (Needs Attention, Open PRs, Active, Idle) with colored icons — but these are preview decorations, not real sections. Users might think they're interactive or broken.

- **Detail panel step cards are oversized.** Each card is 120px wide with `sectionTitle()` font for the step name. Four cards at 120px + spacing = 528px minimum. Doesn't leave room for the rest of the layout to breathe.

- **No runs yet** gives no context. Could say what a run is or how to start one.

- **Symphonia** uses `.font(.system(size: 64))`, `.font(.largeTitle)`, `.font(.title3)`, `.font(.headline)` — all system fonts. Spacing is literal `24` and `16`.

## Approach

Two small PRs, each under 100 lines changed.

### PR 1: Simplify sidebar empty states

**Disconnected state** — keep as-is. It's clear and focused: one message, one action.

**Empty waves state** — simplify to match the disconnected pattern:

| Before | After |
|--------|-------|
| QuickExperimentSidebarView (2x2 grid) | Remove |
| Divider | Remove |
| Create Wave button | Keep — single primary action |
| SidebarPreviewView (mock sections) | Remove |
| — | Add subtitle: "Waves track ongoing branches, PRs, and runs" |

The detail panel already has Quick Experiment prominently. Duplicating it in the sidebar clutters the first impression. One CTA (Create Wave) with a one-line description is enough.

```swift
// After: sidebar empty state
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

This mirrors the disconnected state's structure exactly: icon + title + subtitle + button. Consistent visual language for sidebar empty states.

**What gets removed:**
- `QuickExperimentSidebarView` (still used? No — only called from sidebar empty state)
- `SidebarPreviewView` (only called from sidebar empty state)
- Both structs can be deleted from `QuickExperimentView.swift` if they have no other call sites

### PR 2: Polish detail panel + minor empty states

**Detail panel** (`QuickExperimentDetailView`) — reduce step card visual weight:

| Property | Before | After |
|----------|--------|-------|
| Step name font | `Typography.sectionTitle()` + `.semibold` | `Typography.body()` + `.medium` |
| Card width | `120` fixed | `100` fixed |
| Card padding | `Spacing.lg` vertical | `Spacing.md` vertical |
| Card background | `palette.surface` | `palette.surface.opacity(0.7)` |

This makes the step cards feel like options rather than destinations. The "Quick Experiment" title and bolt icon already establish the section — the cards don't need to shout.

**Runs tab empty** — add context:

```swift
// Before
Text("No runs yet")
    .font(Typography.caption())
    .foregroundStyle(.secondary)

// After
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
```

## Key decisions

**Remove Quick Experiment from sidebar.** The detail panel already showcases it prominently. Duplicating in the sidebar creates visual noise at first impression. Users who want Quick Experiment will see it in the main panel.

**Keep disconnected state unchanged.** It's already well-structured — icon + message + action. The empty waves state should match its pattern, not the other way around.

**Two PRs.** Sidebar simplification is one conceptual change (first impressions). Detail + minor states are independent cleanup.

## Scope

- In scope: Sidebar empty state simplification, detail panel card density, runs tab empty message, Symphonia design tokens
- Out of scope: Welcome window, setup view, command palette, new empty state illustrations

## Done when

- Sidebar empty state shows icon + "No waves yet" + subtitle + Create Wave button
- SidebarPreviewView and QuickExperimentSidebarView removed (if no other call sites)
- Detail panel step cards are visually lighter
- Runs tab empty state has explanatory subtitle
- Symphonia placeholder uses Typography and Spacing tokens
- `swift build --package-path swift` passes
