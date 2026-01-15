# Pipeline Editor for Maestro

## What to build

A visual pipeline editor in Maestro that lets users create, edit, and compose pipelines with per-step configuration, reusable inner pipelines, and branch/merge patterns.

## User requirements (verbatim)

> "Things I want to make sure are easy:
> * defining 'inner pipelines', little modules i can save and reuse
> * mutating arbitrary conditions of it, things like context and voice per-task"

> "Do we need an atomic operation involving branch/compare/synthesize or something?"

> "I do want to be able to do things where i try things with two voices and then integrate both. but i need the pipeline to have one clear output."

## Core insight: Not pure DAGs

The user wants to branch, try alternatives, then merge. This is a **scatter-gather with synthesis** pattern:

```
         ┌── opus voice ──┐
design ──┤                ├── synthesize ── polish
         └── sonnet voice─┘
```

Key difference from pure DAGs: the merge step isn't just collecting outputs—it's synthesizing them into a single coherent result. The pipeline must have "one clear output."

## Existing code context

The codebase already has pipeline support in `src/loopflow/lfd/pipelines.py`:

```python
# Current implementation
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

Current `parallel` runs steps concurrently **in the same worktree** (fine for lint + test). For "try two voices", we need **branches** that run in isolation and merge.

## Data structures

Extend `StepConfig` to support voice and context:

```python
# src/loopflow/lfd/pipelines.py

@dataclass
class StepConfig:
    """Per-step overrides."""
    model: str | None = None
    voice: str | None = None          # NEW
    context: list[str] | None = None  # NEW

    def to_dict(self) -> dict:
        result = {}
        if self.model:
            result["model"] = self.model
        if self.voice:
            result["voice"] = self.voice
        if self.context:
            result["context"] = self.context
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "StepConfig":
        return cls(
            model=data.get("model"),
            voice=data.get("voice"),
            context=data.get("context"),
        )
```

Add a new `branch` construct with merge strategy:

```python
class MergeStrategy(str, Enum):
    FIRST_SUCCESS = "first_success"   # Take first branch that succeeds
    SYNTHESIZE = "synthesize"         # Run synthesis task on all outputs
    ALL = "all"                       # Merge all branch diffs

@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None
    parallel: list["PipelineStep"] | None = None  # concurrent, same worktree
    branch: "BranchStep" | None = None            # NEW: isolated, then merge
    config: StepConfig | None = None

@dataclass
class BranchStep:
    """Run branches in isolated worktrees, then merge."""
    branches: list[list[PipelineStep]]
    merge: MergeStrategy = MergeStrategy.SYNTHESIZE
```

**Distinction:**
- `parallel:` runs steps concurrently in the same worktree (existing behavior)
- `branch:` runs branches in isolated worktrees and merges (new)

YAML example:

```yaml
# .lf/pipelines/dual-voice.yaml
steps:
  - task: design
  - branch:
      branches:
        - - task: implement
            config:
              voice: architect
        - - task: implement
            config:
              voice: minimalist
      merge: synthesize
  - task: polish
```

## Key functions

```python
# Existing (extend)
def load_pipeline(name: str, repo: Path) -> PipelineDef | None:
    """Load pipeline from .lf/pipelines/{name}.yaml."""

def resolve_pipeline(pipeline: PipelineDef, repo: Path) -> list[ResolvedStep]:
    """Expand nested pipelines, mark parallel groups and branches."""

# New functions
def create_branch_worktree(base: Path, branch_name: str) -> Path:
    """Create temp worktree from current state for branch execution."""

def execute_branch(steps: list[PipelineStep], worktree: Path, run_step: callable) -> BranchResult:
    """Run branch steps in isolated worktree, return result with diff."""

def merge_branches(results: list[BranchResult], strategy: MergeStrategy, worktree: Path) -> bool:
    """Merge branch results into main worktree according to strategy."""

def run_synthesis_task(branch_diffs: list[str], worktree: Path) -> bool:
    """Run built-in synthesis task that sees all branch diffs."""
```

### Branch execution flow

1. At `branch:` step, create temp worktree for each branch
2. Execute branch steps in parallel (each in its own worktree)
3. Collect diffs from each branch (git diff against starting point)
4. Merge according to strategy:
   - `first_success`: apply diff from first branch that succeeded
   - `synthesize`: pass all diffs to synthesis task, which produces unified changes
   - `all`: attempt to apply all diffs (fail on conflict)
5. Clean up temp worktrees
6. Continue with remaining steps in main worktree

### Synthesis task

Built-in task that receives branch diffs as context:

```markdown
You have {n} implementation attempts for the same task.

<branch name="architect">
{diff from branch 1}
</branch>

<branch name="minimalist">
{diff from branch 2}
</branch>

Synthesize these into a single implementation that captures the best of each approach.
Apply your changes to the current worktree.
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
│   ship ●─●─●─●           │  (4 sequential steps)
│   dual-voice ●─═─●       │  (branch in middle)
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
┌─ Pipeline: dual-voice ────────────────────────────────┐
│                                                        │
│  [design] ──▶ [═ branch ═] ──▶ [polish]  [+]          │
│                  │                                     │
│               ┌──┴──┐                                  │
│               │ 2   │  (click to expand)               │
│               └─────┘                                  │
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
                if step.task != nil {
                    Text(step.task!)
                } else if step.pipeline != nil {
                    Image(systemName: "arrow.triangle.branch")
                    Text(step.pipeline!)
                } else if step.branch != nil {
                    Image(systemName: "arrow.triangle.branch")
                    Text("\(step.branch!.branches.count)")
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

### Branch block editor

```swift
struct BranchBlockEditor: View {
    @Binding var branch: BranchStep

    var body: some View {
        VStack(alignment: .leading) {
            Text("Branch").font(.headline)

            ForEach(branch.branches.indices, id: \.self) { i in
                HStack {
                    Text("Branch \(i + 1):")
                    // Steps for this branch
                    ForEach(branch.branches[i], id: \.id) { step in
                        StepChip(step: step)
                    }
                    Button("+") { addStepToBranch(i) }
                }
            }

            Button("Add Branch") { addBranch() }

            Picker("Merge", selection: $branch.merge) {
                Text("First success").tag(MergeStrategy.firstSuccess)
                Text("Synthesize").tag(MergeStrategy.synthesize)
                Text("All").tag(MergeStrategy.all)
            }
        }
        .padding()
        .background(.quaternary)
        .cornerRadius(8)
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
- Pipeline files stored as YAML in `.lf/pipelines/` (not config.yaml's simple `pipelines:` format)
- Inner pipelines are just pipelines referenced by name via `pipeline:` field
- `branch:` creates isolated worktrees; `parallel:` runs in same worktree (existing behavior)
- Merge strategy is required for `branch:` blocks
- StepConfig must support model, voice, and context overrides
- UI must distinguish sequential steps, parallel steps, and branch blocks visually

**Acceptable to defer:**
- Drag-and-drop reordering (use buttons/context menu first)
- Live pipeline execution visualization in editor
- Complex branch conditions (if/else based on step output)
- Pipeline versioning or history

## Done when

**Backend:**
```bash
# StepConfig supports voice and context
uv run pytest tests/test_pipelines.py -k "voice or context" -v

# Branch execution creates worktrees and merges
uv run pytest tests/test_pipelines.py -k "branch" -v

# CLI can run pipelines from .lf/pipelines/
lf dual-voice  # runs .lf/pipelines/dual-voice.yaml
```

**UI:**
```bash
# Build succeeds
cd Maestro && xcodebuild -scheme Maestro -configuration Debug build

# Manual verification:
# 1. Open Maestro with a repo
# 2. Create pipeline via Pipelines sidebar
# 3. Add steps with per-step voice config
# 4. Add branch block with 2 branches
# 5. Save → check .lf/pipelines/test.yaml exists
# 6. Run pipeline from Maestro
```

## Files to modify

**Python (backend):**
- `src/loopflow/lfd/pipelines.py` - extend StepConfig, add BranchStep
- `src/loopflow/cli/run.py` - support running pipelines from .lf/pipelines/
- `tests/test_pipelines.py` - new tests for branch execution

**Swift (Maestro):**
- `Maestro/Models/Pipeline.swift` - new model matching Python structures
- `Maestro/Services/PipelineService.swift` - load/save pipeline YAML
- `Maestro/Views/PipelineEditor.swift` - new editor view
- `Maestro/Views/ContentView.swift` - add Pipelines section to sidebar
- `Maestro/Views/PromptLauncher.swift` - add pipeline selection

## Open questions

1. **Synthesis task**: Should there be a built-in "synthesize" task, or should users specify which task to run for synthesis? Leaning toward built-in with option to override.

2. **Branch worktrees**: Should parallel branches run in git worktrees or git stash/branches? Worktrees are cleaner but heavier. Decision: use worktrees since loopflow already manages them.

3. **Conflict resolution**: When merge strategy is "all", how to handle conflicting changes from branches? Decision: fail with error message showing conflicts; user can switch to "synthesize" which handles conflicts via LLM.

4. **Migration**: Should we migrate existing `config.yaml` pipelines to `.lf/pipelines/*.yaml`? Decision: support both, prefer `.lf/pipelines/` for new pipelines. Simple task lists in config.yaml continue to work.
