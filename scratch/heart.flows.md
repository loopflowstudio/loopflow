# Wave-Keyed Roadmap Prompt Refinement

Refine prompts so wave context flows through the entire build workflow.

## Problem

Two concepts are conflated:
- **Area**: Path scope (`src/api/`, `swift/`) — what code to read/modify
- **Wave**: Program of work (`rust`, `enterprise`) — strategic context with backlog in `roadmap/<wave>/`

The Wave model has a `name` field and an `area` field — they're distinct. But the prompts don't leverage the connection between `wave.name` and `roadmap/<wave>/`.

The rust roadmap shows what's possible: staged items (01-protocol, 02-core-engine), strategic README with principles/non-goals/success criteria. Current prompts don't surface this context throughout the workflow.

User quote: "I had previously considered the roadmap keys to be an area."

## What to build

Update prompts so:
1. **Wave name links to roadmap** — `wave.name = "rust"` means read `roadmap/rust/`
2. **Wave context propagates through workflow** — README.md informs all steps
3. **Staged roadmaps work correctly** — Numbered prefixes are respected
4. **Terminology is clear** — Prompts distinguish area vs wave

## Data structures

No new Python data structures. The connection is implicit:

**Wave model** (existing in `lfd/models.py`):
```python
class Wave:
    name: str           # e.g., "rust"
    area: list[str]     # e.g., ["src/", "swift/"]
    # ...
```

**Roadmap folder** (filesystem):
```
roadmap/
  rust/                    # matches wave.name
    README.md              # strategic context
    01-protocol.md         # stage 1
    02-core-engine.md      # stage 2
  lfflow/                  # another wave
    docs-inclusion.md      # simple items (no stages)
```

**The link**: When `wave.name = "rust"`, prompts should read `roadmap/rust/`.

Currently `roadmap/` is auto-included in context, but:
- Nothing highlights the wave's specific README.md
- Stage ordering isn't recognized
- Wave context doesn't flow to implement/kickoff

## Approach: Computed wave context

Wave context should be computed once and included in the prompt structure, not inferred ad-hoc by each step.

### Wave determination

```python
@dataclass
class WaveContext:
    """Wave context for prompt assembly."""
    name: str                    # e.g., "rust"
    roadmap_path: Path           # roadmap/rust/
    readme_summary: str | None   # Key principles from README.md
    current_stage: str | None    # e.g., "01-protocol" for staged roadmaps
    source: str                  # "explicit" | "inferred"


def determine_wave(
    repo_root: Path,
    explicit_wave: str | None = None,  # From lfd or --wave flag
) -> WaveContext | None:
    """Determine wave context.

    Priority:
    1. Explicit wave name (from lfd or --wave flag)
    2. Infer from worktree directory name
    """
    if explicit_wave:
        roadmap_path = repo_root / "roadmap" / explicit_wave
        if roadmap_path.is_dir():
            return WaveContext(
                name=explicit_wave,
                roadmap_path=roadmap_path,
                readme_summary=_summarize_readme(roadmap_path),
                current_stage=_find_current_stage(roadmap_path),
                source="explicit",
            )

    # Infer from worktree name
    worktree_name = repo_root.name  # e.g., "loopflow.rust-protocol"
    for candidate in _extract_wave_candidates(worktree_name):
        roadmap_path = repo_root / "roadmap" / candidate
        if roadmap_path.is_dir():
            return WaveContext(
                name=candidate,
                roadmap_path=roadmap_path,
                readme_summary=_summarize_readme(roadmap_path),
                current_stage=_find_current_stage(roadmap_path),
                source="inferred",
            )

    return None
```

### Worktree name inference

Extract wave candidates from worktree/branch names:

| Name | Candidates |
|------|------------|
| `loopflow.rust-protocol` | `rust`, `rust-protocol` |
| `jack.rust-protocol.20260127` | `rust`, `rust-protocol` |
| `loopflow.lfflow` | `lfflow` |
| `feature-enterprise-auth` | `enterprise`, `enterprise-auth` |

Check each candidate against existing `roadmap/<candidate>/` directories.

### Prompt structure addition

Add wave section after run mode (position 2.5 in current structure):

```
1. System docs (loopflow)
2. Run mode
→ 2.5. Wave context (NEW)
3. Reference material (docs, summaries)
4. Instructions (direction, step)
5. Working context (diff, clipboard, images)
```

Wave section format:

```markdown
<lf:wave name="rust" stage="01-protocol">
You are building toward the rust program of work.

**Principles**:
- Protocol first: every project starts by validating the protocol surface
- UX invariants: prompts, flows, directions, and artifact paths must not change
- Control/execution isolation: failures in execution must not destabilize control plane

**Current stage**: 01-protocol (Protocol-First Engine)

**Success criteria**: Protocol supports local + remote clients without UX drift

Roadmap items: roadmap/rust/
</lf:wave>
```

### What to show/collapse

| Scenario | Wave section |
|----------|--------------|
| **lfd with explicit wave** | Full context: principles, stage, success criteria |
| **lf CLI with inferred wave** | Lighter: wave name, roadmap path, brief principles |
| **No wave detected** | Omit section entirely |

### Code changes needed

1. **New module**: `src/loopflow/lf/wave.py`
   - `WaveContext` dataclass
   - `determine_wave()` function
   - `_summarize_readme()` — extract principles/goals/success from README.md
   - `_find_current_stage()` — find earliest incomplete stage
   - `_extract_wave_candidates()` — parse worktree name

2. **Update `PromptComponents`**: Add `wave: WaveContext | None` field

3. **Update `gather_prompt_components()`**: Accept `wave: str | None`, call `determine_wave()`

4. **Update `format_prompt()`**: Add wave section formatting

5. **Update `ContextConfig`**: Add `wave: str | None` for explicit wave override

6. **Update CLI**: Add `--wave` flag to `lf` commands

7. **Update `lfd/execution/runner.py`**: Pass `wave.name` to `gather_prompt_components()`

### Flow from lfd

```
lfd loop rust-wave src/
    │
    ├── Wave(name="rust", area=["src/"], ...)
    │
    └── runner.py: _build_loop_prompt(wave, ...)
            │
            └── gather_prompt_components(..., wave=wave.name)
                    │
                    └── determine_wave(repo, explicit_wave="rust")
                            │
                            └── WaveContext(name="rust", source="explicit", ...)
```

### Flow from lf CLI

```
lf review --wave rust
    │
    └── step.py: run(..., wave="rust")
            │
            └── gather_prompt_components(..., wave="rust")
                    │
                    └── determine_wave(repo, explicit_wave="rust")

lf review  (in worktree loopflow.rust-protocol)
    │
    └── step.py: run(...)
            │
            └── gather_prompt_components(..., wave=None)
                    │
                    └── determine_wave(repo, explicit_wave=None)
                            │
                            └── infer from worktree name → "rust"
```

## Key changes

### Prompts: Reference wave from context

Since wave context is now in the prompt (in `<lf:wave>` section), prompts reference it directly rather than inferring.

### 1. `ingest.md` — Use wave context for staged selection

**Update workflow to reference wave context:**

```markdown
## Staged roadmaps

The wave context (if present) shows current stage. When selecting items:
- Pick from the current stage first
- README.md is context, not a pickable item
- Stage number prefixes (01-, 02-) indicate ordering

## Using wave context

If `<lf:wave>` is present in context:
- Follow the wave's principles when evaluating priority
- Selected item contributes to wave success criteria
- Include wave name in output path: `scratch/<wave>-<slug>.md`
```

### 2. `kickoff.md` — Design aligns with wave

**Update to reference wave context:**

```markdown
## Wave alignment

If `<lf:wave>` is present in context:

1. Design must follow the wave's principles
2. Scope must exclude wave non-goals
3. "Done when" must contribute to wave success criteria

Quote the specific principles you're following in "Key decisions".
```

### 3. `implement.md` — Follow wave principles

**Update to reference wave context:**

```markdown
## Wave context

If `<lf:wave>` is present in context:

1. Follow the wave's principles during implementation
2. Check against compatibility matrix if mentioned
3. Note drift from wave constraints in `scratch/questions.md`

The wave context has already summarized the key constraints—follow them.
```

### 4. `add-to-roadmap.md` — Use wave context for routing

**Update wave routing:**

```markdown
## Wave routing

If `<lf:wave>` is present in context:
- Route actionable items to `roadmap/<wave>/`
- Use wave's stage structure if applicable

Otherwise, infer wave from:
1. Design doc references to a wave
2. Worktree/branch name patterns
3. Existing wave that matches the work's scope

For staged waves:
- Add to appropriate stage directory
- Or propose new stage if work is a new phase
```

### 5. `LOOPFLOW.md` — Add wave section

**Add after "Run Modes":**

```markdown
## Wave

If you're building toward a program of work, you'll see a `<lf:wave>` section in context.

**What it tells you:**
- **Wave name**: Which roadmap you're contributing to
- **Principles**: Constraints to follow (e.g., "protocol first", "UX invariants")
- **Current stage**: Where you are in staged roadmaps
- **Success criteria**: What "done" looks like for this wave

**How to use it:**
- Follow wave principles in design and implementation decisions
- Note drift from constraints in `scratch/questions.md`
- Route new work items to the same wave

If no wave section is present, you're doing standalone work.
```

## Constraints

- Don't break existing simple roadmaps (lfflow items work as-is)
- Don't require README.md — waves without strategic context still work
- Stage numbering is optional, not required

## Files to change

**Code (wave determination + prompt assembly):**
```
src/loopflow/lf/wave.py                            # NEW: WaveContext, determine_wave()
src/loopflow/lf/context.py                         # Add wave to PromptComponents, format_prompt()
src/loopflow/lf/step.py                            # Add --wave flag, pass to gather_prompt_components()
src/loopflow/lfd/execution/runner.py               # Pass wave.name to context gathering
```

**Prompts (leverage wave context):**
```
src/loopflow/lf/builtins/steps/plan/ingest.md      # Use wave context for staged selection
src/loopflow/lf/builtins/steps/plan/kickoff.md     # Reference wave from prompt context
src/loopflow/lf/builtins/steps/code/implement.md   # Check wave constraints from context
src/loopflow/lf/builtins/steps/ops/add-to-roadmap.md  # Use wave context for routing
src/loopflow/LOOPFLOW.md                           # Add "Waves vs Areas" section
```

**Tests:**
```
tests/test_wave.py                                 # NEW: wave determination tests
tests/test_context.py                              # Update for wave in components
```

## Done when

```bash
# Wave determination works
python -c "from loopflow.lf.wave import determine_wave; print(determine_wave(Path('.'), 'rust'))"
# → WaveContext(name='rust', source='explicit', ...)

# Wave appears in prompt
lf review --wave rust 2>&1 | grep -q "lf:wave"  # Wave section in output

# Inference works
cd ../loopflow.rust-protocol && lf review 2>&1 | grep -q "lf:wave"

# Staged selection respects order
lf ingest --wave rust  # Picks from 01-* before 02-*
```
