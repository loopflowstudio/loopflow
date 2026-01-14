# Maestro: Prompt Editor

**What to build:** A new tab in Maestro for editing tasks, pipelines, and voices.

## Current State

Maestro has:
- WorktreeSidebar (left) - list worktrees
- PromptLauncher (right) - select prompt, configure context, launch

The PromptLauncher lets you *run* prompts. This feature adds a way to *edit* them.

## New View: PromptEditor

Two-column layout for editing the prompt atoms:

```
┌─────────────────────────────────────────────────────────────┐
│ [Launcher]  [Editor]                                        │  ← tab bar
├────────────────────────────┬────────────────────────────────┤
│ TASKS                      │ VOICES                         │
│ ┌────────────────────────┐ │ ┌────────────────────────────┐ │
│ │ implement         ✎   │ │ │ architect            ✎   │ │
│ │ review            ✎   │ │ │ concise              ✎   │ │
│ │ polish            ✎   │ │ │ + new voice              │ │
│ │ + new task            │ │ └────────────────────────────┘ │
│ └────────────────────────┘ │                                │
│                            │                                │
│ PIPELINES                  │                                │
│ ┌────────────────────────┐ │                                │
│ │ ship: implement →      │ │                                │
│ │       review → polish  │ │                                │
│ │ + new pipeline         │ │                                │
│ └────────────────────────┘ │                                │
├────────────────────────────┴────────────────────────────────┤
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Bring an architect's perspective. Focus on what only    │ │  ← editor
│ │ an architect would catch:                               │ │
│ │                                                         │ │
│ │ - How does this fit the larger system?                  │ │
│ │ - What interfaces will other code need?                 │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

Click an item to edit it in the bottom pane. Changes save on blur/cmd+s.

## Data Structures

```swift
// Models/Voice.swift
struct Voice: Identifiable {
    var id: String { name }
    let name: String
    let path: URL
    var content: String
}

// Models/Pipeline.swift
struct Pipeline: Identifiable {
    var id: String { name }
    let name: String
    var tasks: [String]
}
```

Existing `PromptCard` already models tasks.

## Key Functions

```swift
// Services/VoiceService.swift
func loadVoices(from repo: URL) -> [Voice]
func saveVoice(_ voice: Voice) throws

// Services/PipelineService.swift
func loadPipelines(from config: LoopflowConfig) -> [Pipeline]
func savePipelines(_ pipelines: [Pipeline], to repo: URL) throws
```

## Files to Add

1. `Models/Voice.swift` - Voice model
2. `Models/Pipeline.swift` - Pipeline model
3. `Services/VoiceService.swift` - Load/save voices
4. `Services/PipelineService.swift` - Load/save pipelines
5. `Views/PromptEditor.swift` - Main editor view
6. `Views/AtomList.swift` - Reusable list component for tasks/voices/pipelines

## Files to Modify

1. `ContentView.swift` - Add tab bar to switch between Launcher and Editor
2. `AppState.swift` - Add voices, pipelines, selected item state

## Constraints

- **No separate database.** Read/write `.lf/` files directly.
- **Auto-save on blur.** No explicit save button.
- **Markdown editor.** Plain text, not rich text.

## Done When

1. Tab bar switches between Launcher and Editor views
2. Editor shows tasks, pipelines, voices from current repo
3. Clicking an item opens it in the editor pane
4. Edits save back to disk
