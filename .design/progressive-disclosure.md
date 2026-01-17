# Progressive Disclosure Pass

## What to build

Audit and restructure Maestro's UI to reveal complexity gradually. Use real terminology, but don't show everything at once. First-run experience should feel approachable; power features accessible but not in your face.

## Principles

1. **Collapse, don't remove** — Advanced options exist, just tucked away initially
2. **Real terminology** — "Workspaces" in UI, "worktree" in code. "Diff" not "changes"
3. **Expand on interaction** — Clicking into an area reveals its depth
4. **Remember preferences** — Once expanded, stay expanded for that user

## Changes by area

### Sidebar Header
**Current**: "BRANCHES" (all-caps, semibold, aggressive)
**Change**: "Workspaces" (title case, medium weight)

```swift
// WorktreeSidebar.swift
Text("Workspaces")
    .font(.headline)
    .fontWeight(.medium)
```

### Context Bar
**Current**: Five chips always visible (Docs, Files, Diff, Clipboard, Summaries)
**Change**: Collapsed by default, shows token count only. Click to expand.

```
Collapsed:  [Context: 14.2k tokens ▾]
Expanded:   [Context: 14.2k tokens ▴]
            ☑ Docs  ☑ Files  ☐ Diff  ☐ Clipboard  ☐ Summaries
            [+ Attach files...]
```

```swift
@AppStorage("contextBarExpanded") var contextBarExpanded = false
```

### Options Section
**Current**: Model selector, voice selector visible. Command preview collapsed.
**Change**: All collapsed under "Options ▾". Command preview promoted when expanded.

```
Collapsed:  [Options ▾]
Expanded:   [Options ▴]
            Model: claude:opus
            Voice: architect
            ─────────────────
            Command Preview:
            lf implement -m claude:opus --voice architect ...
```

### Task Selector
**Current**: Dropdown shows task name + "auto"/"interactive" badge
**Change**: Add one-line description visible in dropdown. Tooltip on hover for full description.

```
┌─────────────────────────────────────┐
│ implement                      auto │
│ Turn design doc into working code   │
├─────────────────────────────────────┤
│ review                         auto │
│ Review diff and produce assessment  │
├─────────────────────────────────────┤
│ design                  interactive │
│ Produce implementation spec         │
└─────────────────────────────────────┘
```

### Results Panel Header
**Current**: Five controls in one row (status, text, duration, toggle, clear, expand)
**Change**: Primary: status + duration. Secondary actions in overflow menu.

```
Running implement...  00:45  [⏹] [•••]
                              ↑     ↑
                            stop  menu (clear, expand, copy output)
```

### Empty States
**Current**: Worktree empty state explains concept. Other areas have no guidance.
**Change**: Add contextual hints that disappear after first use.

```swift
// ResultsPanel empty state
if results.isEmpty && !hasRunTaskBefore {
    Text("Run a task to see output here")
        .foregroundColor(.secondary)
}
```

### First-Run State
**Current**: Full UI visible immediately
**Change**: Sensible defaults pre-selected, collapsed sections, hint text

First run defaults:
- Task: `implement` pre-selected
- Context: Docs + Files enabled, others off
- Options: collapsed
- Context bar: collapsed (just shows token count)

## Data structures

```swift
// Track user's disclosure preferences
class DisclosureState: ObservableObject {
    @AppStorage("contextBarExpanded") var contextBarExpanded = false
    @AppStorage("optionsExpanded") var optionsExpanded = false
    @AppStorage("hasRunTaskBefore") var hasRunTaskBefore = false
}
```

## Key functions

```swift
// Collapsible section wrapper
struct DisclosureSection<Content: View>: View {
    let title: String
    @Binding var isExpanded: Bool
    let content: () -> Content
}
```

## Files to modify

| File | Change |
|------|--------|
| `WorktreeSidebar.swift:147-149` | Header "BRANCHES" → "Workspaces" |
| `PromptLauncher.swift:944-1007` | Context bar collapse/expand |
| `PromptLauncher.swift:60-66` | Options section collapse |
| `PromptLauncher.swift:200-226` | Task descriptions in dropdown |
| `ResultsPanel.swift:64-120` | Header density, overflow menu |
| `AppState.swift` | Add DisclosureState |

## Constraints

- **Persist with @AppStorage** — User preferences survive app restart
- **No mode switching** — Don't build "simple mode" vs "advanced mode". Just collapse/expand.
- **Respect existing users** — If they've used Maestro before, don't reset their preferences

## Done when

1. Sidebar header says "Workspaces" (not "BRANCHES")
2. Context bar collapsed by default, expands on click
3. Options section collapsed by default
4. Task dropdown shows one-line descriptions
5. Results panel has overflow menu for secondary actions
6. Preferences persist across app restart
7. `./dev swift` builds and runs
