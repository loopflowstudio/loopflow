# StartWaveView: Replace Quick Experiment Landing

Strip Quick Experiment entirely. Replace with "Start a wave" + text field.

## What to build

The detail panel placeholder (shown when no wave is selected) becomes a single text field: "Start a wave" heading, "What do you want to build?" placeholder. Enter creates a wave with the typed name.

## Delete

- `QuickExperimentView.swift` — entire file (enum, sidebar view, detail view, preview mock)
- `launchQuickExperiment()` in `ContentView.swift` (lines 117-125)
- `launchQuickExperiment()` in `WaveSidebar.swift` (lines 244-253)
- Quick Experiment sidebar empty state content in `WaveSidebar.swift` (lines 206-234)
- `SidebarPreviewView` (in QuickExperimentView.swift)

## New: StartWaveView

New file: `swift/Concerto/Views/StartWaveView.swift`

```swift
struct StartWaveView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @State private var waveName = ""
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            VStack(spacing: Spacing.lg) {
                Text("Start a wave")
                    .font(Typography.heroTitle())
                    .foregroundStyle(palette.accent)

                TextField("What do you want to build?", text: $waveName)
                    .textFieldStyle(.plain)
                    .font(Typography.body())
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .frame(maxWidth: 400)
                    .focused($isTextFieldFocused)
                    .onSubmit { createWave() }
                    .disabled(isCreating)
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { isTextFieldFocused = true }
    }

    private func createWave() {
        // Same logic as WaveSidebar.createWaveDirectly()
        // but passes waveName instead of empty string
        // On success: wave selected, trigger name edit notification
        // On error: set errorMessage
    }
}
```

Key difference from `createWaveDirectly()`: passes `waveName` to `repoState.createWave(name:)` instead of empty string. If name is empty, NameGenerator still provides a default.

## Changes to existing files

**ContentView.swift:**
- Replace `QuickExperimentDetailView { step in ... }` with `StartWaveView()`
- Delete `launchQuickExperiment()` function

**ScreenshotWindow.swift:**
- Replace `QuickExperimentDetailView { _ in }` with `StartWaveView()`

**WaveSidebar.swift:**
- Simplify `emptyState` — remove QuickExperimentSidebarView, divider, SidebarPreviewView
- Remove `launchQuickExperiment()` function
- Empty state becomes minimal: centered "No waves yet" or just empty with the header "+" button

## Constraints

- `createWave(name:)` on RepoState already accepts a name — no model changes needed
- Wave creation still goes through lfd (connect if needed, create, select, trigger name edit)
- If text field is empty on submit, fall through to auto-generated name (existing behavior)

## Done when

- `grep -r "QuickExperiment" swift/` returns nothing
- Landing screen shows "Start a wave" heading + text field
- Typing a name and pressing Enter creates a wave with that name
- Empty name still works (auto-generates)
- Sidebar empty state has no step buttons or preview mock
