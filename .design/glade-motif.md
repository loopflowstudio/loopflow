# Pipeline Editor for Maestro

## What to build

A visual pipeline editor in Maestro that lets users create, edit, and run sequential pipelines with per-step configuration.

## User requirements (verbatim)

> "Things I want to make sure are easy:
> * defining 'inner pipelines', little modules i can save and reuse
> * mutating arbitrary conditions of it, things like context and voice per-task"

## MVP scope: Sequential pipelines

Start with the common case: linear pipelines like `design → implement → review → polish`. Each step runs after the previous one completes. This covers 90% of workflows.

```
design ──▶ implement ──▶ review ──▶ polish
```

Per-step configuration (voice, model, context) lets users customize behavior without creating new task files.

**Deferred:** Branching/merging pipelines. The user mentioned wanting to "try things with two voices and then integrate both"—this is real but secondary. Get the sequential UI right first.

## Existing code context

The codebase already has pipeline support in `src/loopflow/lfd/pipelines.py`:

```python
@dataclass
class StepConfig:
    model: str | None = None

@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None      # Nested pipeline reference
    parallel: list["PipelineStep"] | None = None
    config: StepConfig | None = None
```

## Data structures

Extend `StepConfig` to support voice and context:

```python
@dataclass
class StepConfig:
    model: str | None = None
    voice: str | None = None          # NEW
    context: list[str] | None = None  # NEW
```

The existing `PipelineStep` structure already supports:
- `task`: run a single task
- `pipeline`: reference another pipeline (for composition)
- `config`: per-step overrides

No new structures needed for sequential pipelines.

YAML example:

```yaml
# .lf/pipelines/ship.yaml
steps:
  - task: design
  - task: implement
    config:
      voice: architect
      model: claude:opus
  - task: review
  - task: polish
```

## UI changes

Maestro uses SwiftUI with existing patterns: typeahead selectors, chips for multi-select, popovers for detail editing, NavigationSplitView for sidebar/detail layouts.

### Pipeline list in sidebar (new section)

Add "Pipelines" section to `ContentView` sidebar alongside Worktrees:

```swift
// Maestro/Views/ContentView.swift
Section("Pipelines") {
    ForEach(appState.pipelines) { pipeline in
        PipelineRow(pipeline: pipeline)
    }
    Button("New Pipeline", systemImage: "plus") {
        showNewPipelineSheet = true
    }
}
```

Visual representation:
```
┌─ Sidebar ────────────────┐
│ Worktrees               ▼│
│   main                   │
│   feature-auth           │
│                          │
│ Pipelines               ▼│
│   ship ●─●─●─●           │  (4 steps)
│   quick ●─●              │  (2 steps)
│   [+] New                │
└──────────────────────────┘
```

### Pipeline editor view (new)

New view file: `Maestro/Views/PipelineEditor.swift`

Uses horizontal flow layout showing pipeline structure:

```swift
struct PipelineEditor: View {
    @Binding var pipeline: PipelineDef
    let availableTasks: [PromptCard]

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Pipeline name
            TextField("Pipeline name", text: $pipeline.name)

            // Steps flow (horizontal scroll)
            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    ForEach(pipeline.steps.indices, id: \.self) { i in
                        StepView(step: $pipeline.steps[i], availableTasks: availableTasks)
                        if i < pipeline.steps.count - 1 {
                            Image(systemName: "arrow.right")
                        }
                    }
                    AddStepButton { addStep() }
                }
            }

            // Actions
            HStack {
                Button("Save") { save() }
                Button("Run") { run() }
            }
        }
    }
}
```

Visual flow:
```
┌─ Pipeline: ship ──────────────────────────────────────┐
│                                                        │
│  [design] ──▶ [implement●] ──▶ [review] ──▶ [polish]  │
│                                                   [+]  │
│  ● = has config overrides (click to edit)             │
│                                                        │
│  [Save]  [Run]                                         │
└────────────────────────────────────────────────────────┘
```

### Step chip component

Reuse existing chip pattern from `PromptLauncher`:

```swift
struct StepChip: View {
    let step: PipelineStep
    @State private var showEditor = false

    var body: some View {
        Button {
            showEditor = true
        } label: {
            HStack(spacing: 4) {
                if let task = step.task {
                    Text(task)
                } else if let pipeline = step.pipeline {
                    Image(systemName: "rectangle.stack")
                    Text(pipeline)
                }
                if step.config != nil {
                    Circle().fill(.blue).frame(width: 4, height: 4)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.quaternary)
            .cornerRadius(4)
        }
        .popover(isPresented: $showEditor) {
            StepEditorPopover(step: $step)
        }
    }
}
```

### Step editor popover

```swift
struct StepEditorPopover: View {
    @Binding var step: PipelineStep
    let availableTasks: [PromptCard]

    var body: some View {
        Form {
            // Task selector (typeahead)
            Picker("Task", selection: $step.task) {
                ForEach(availableTasks) { task in
                    Text(task.name).tag(task.name)
                }
            }

            // Or nested pipeline
            Picker("Pipeline", selection: $step.pipeline) {
                Text("None").tag(nil as String?)
                // ... available pipelines
            }

            Section("Overrides") {
                TextField("Model", text: binding(for: \.config?.model))
                TextField("Voice", text: binding(for: \.config?.voice))
                // Context paths (chips)
            }

            Button("Delete", role: .destructive) { delete() }
        }
        .frame(width: 300)
        .padding()
    }
}
```

### PromptLauncher integration

Add pipeline selection to existing launcher (minimal change):

```swift
// In PromptLauncher.swift, extend the task picker
Picker("", selection: $selectedItem) {
    Section("Tasks") {
        ForEach(prompts) { prompt in
            Text(prompt.name).tag(LaunchItem.task(prompt))
        }
    }
    Section("Pipelines") {
        ForEach(pipelines) { pipeline in
            Text(pipeline.name).tag(LaunchItem.pipeline(pipeline))
        }
    }
}
```

## Constraints

**Must get right:**
- Pipeline files stored as YAML in `.lf/pipelines/`
- Inner pipelines referenced by name via `pipeline:` field
- StepConfig must support model, voice, and context overrides
- Steps shown as clickable chips with config indicator (blue dot)

**Acceptable to defer:**
- Branching/merging pipelines
- Drag-and-drop reordering (use buttons/context menu first)
- Live pipeline execution visualization in editor
- Pipeline versioning or history

## Done when

**Backend:**
```bash
# StepConfig supports voice and context
uv run pytest tests/test_pipelines.py -k "voice or context" -v

# CLI can run pipelines from .lf/pipelines/
lf ship  # runs .lf/pipelines/ship.yaml
```

**UI:**
```bash
# Build succeeds
cd Maestro && xcodebuild -scheme Maestro -configuration Debug build

# Manual verification:
# 1. Open Maestro with a repo
# 2. Create pipeline via Pipelines sidebar
# 3. Add steps with per-step voice/model config
# 4. Save → check .lf/pipelines/test.yaml exists
# 5. Run pipeline from Maestro
```

## Files to modify

**Python (backend):**
- `src/loopflow/lfd/pipelines.py` - extend StepConfig with voice/context
- `src/loopflow/cli/run.py` - support running pipelines from .lf/pipelines/
- `tests/test_pipelines.py` - tests for StepConfig serialization

**Swift (Maestro):**
- `Maestro/Models/Pipeline.swift` - model matching Python structures
- `Maestro/Services/PipelineService.swift` - load/save pipeline YAML
- `Maestro/Views/PipelineEditor.swift` - editor view
- `Maestro/Views/ContentView.swift` - add Pipelines section to sidebar
- `Maestro/Views/PromptLauncher.swift` - add pipeline selection

## Open questions

1. **Migration**: Should we migrate existing `config.yaml` pipelines to `.lf/pipelines/*.yaml`? Decision: support both, prefer `.lf/pipelines/` for new pipelines. Simple task lists in config.yaml continue to work.

## Future: Branching pipelines

Deferred to a future iteration. The user wants to "try things with two voices and then integrate both"—this requires:
- `branch:` construct with isolated worktrees
- Merge strategies (first_success, synthesize, all)
- Synthesis task to combine branch outputs

Get sequential pipelines right first.
