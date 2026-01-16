# Pipeline Editor for Maestro

Visual pipeline editor in Maestro for creating, editing, and running pipelines with per-step configuration and parallel execution.

## Implementation status

**Complete:**
- `StepConfig` extended with `voice` and `context` fields (Python + Swift)
- Pipeline YAML loading/saving via `PipelineService`
- `PipelineEditor` view with chip-based step visualization
- Pipelines section in `WorktreeSidebar`
- Pipeline selection in `PromptLauncher` task dropdown
- CLI support: `lf <pipeline>` runs from `.lf/pipelines/` or `config.yaml`
- Per-step config overrides applied during pipeline execution
- Tests for `StepConfig` voice/context serialization
- **Parallel step execution** via temporary worktrees
- **Model racing**: `--race` flag and pipeline `race:` syntax for running same task with multiple models

**Deferred:**
- Branching/merging pipelines
- Drag-and-drop step reordering
- Live execution visualization
- "Save as pipeline" from task selection

## Data structures

Python (`src/loopflow/lfd/pipelines.py`):

```python
@dataclass
class StepConfig:
    model: str | None = None
    voice: str | None = None
    context: list[str] | None = None

@dataclass
class RaceConfig:
    models: list[str]
    judge: str = "compare"

@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None
    parallel: list["PipelineStep"] | None = None
    race: RaceConfig | None = None
    config: StepConfig | None = None
```

Swift (`Maestro/Models/Pipeline.swift`):

```swift
struct StepConfig: Codable, Equatable {
    var model: String?
    var voice: String?
    var context: [String]?
}

struct PipelineStep: Codable, Equatable, Identifiable {
    var task: String?
    var pipeline: String?
    var config: StepConfig?
}
```

## YAML format

```yaml
# .lf/pipelines/ship.yaml
steps:
  - design
  - task: implement
    config:
      model: claude:opus
      voice: architect
      context:
        - src/schema.py
  - parallel:
      - test
      - lint
  - review
  - polish
```

Parallel steps run concurrently in temporary worktrees (`_parallel-{task}-{uuid}`), then clean up.

Race steps run the same task with multiple models:

```yaml
steps:
  - task: implement
    race:
      - claude:opus
      - codex:o3
      - gemini:2.5-pro
  - review
```

CLI equivalent: `lf implement --race claude:opus,codex:o3,gemini:2.5-pro`

## Key files

**Python:**
- `src/loopflow/lfd/pipelines.py` - data structures, load/save, resolve
- `src/loopflow/pipeline.py` - execution with `_run_step()`, `_run_parallel_group()`, `_run_race_step()` helpers
- `src/loopflow/lfd/collector.py` - output collection with `--prefix` for parallel tasks
- `src/loopflow/cli/run.py` - `pipeline` command routing, `--race` flag

**Swift:**
- `Maestro/Models/Pipeline.swift` - Swift models
- `Maestro/Services/PipelineService.swift` - YAML parsing
- `Maestro/Views/PipelineEditor.swift` - editor UI
- `Maestro/Views/WorktreeSidebar.swift` - sidebar section

## Config inheritance

```
global config → pipeline config → step config
```

Each level can override the previous. Step-level `voice`, `model`, and `context` take precedence.

## Open questions

See `.design/questions.md` for deferred branching pipeline questions.
