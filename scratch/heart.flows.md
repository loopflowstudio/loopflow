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

## Approach: Unified wave determination

Wave name should be computed once and available throughout the prompt structure. The wave context is minimal—just a name—while `roadmap/<wave>/` is auto-included in docs.

### Wave determination

```python
@dataclass
class WaveContext:
    """Wave context for prompt assembly."""
    name: str         # e.g., "rust"
    source: str       # "explicit" | "inferred"


def determine_wave(
    repo_root: Path,
    explicit_wave: str | None = None,  # From lfd or --wave flag
) -> WaveContext | None:
    """Determine wave context.

    Priority:
    1. Explicit wave name (from lfd or --wave flag)
    2. Infer from worktree directory name (only if roadmap/<candidate>/ exists)

    An explicit wave is always honored, even without a roadmap folder.
    Inference requires a matching roadmap folder to avoid false positives.
    """
    if explicit_wave:
        return WaveContext(name=explicit_wave, source="explicit")

    # Infer from worktree name — requires matching roadmap folder
    worktree_name = repo_root.name  # e.g., "loopflow.rust-protocol"
    for candidate in _extract_wave_candidates(worktree_name):
        roadmap_path = repo_root / "roadmap" / candidate
        if roadmap_path.is_dir():
            return WaveContext(name=candidate, source="inferred")

    return None
```

**Key distinction**:
- **Explicit wave** (`--wave rust` or lfd): Always creates context, even without roadmap folder
- **Inferred wave**: Only if `roadmap/<candidate>/` exists (avoids false positives from worktree names)

### Roadmap inclusion in docs

When wave context exists, `roadmap/<wave>/` is prioritized in docs gathering:

```python
def gather_docs(..., wave: WaveContext | None = None):
    """Gather docs, prioritizing wave roadmap if present."""
    docs = []

    # Always include roadmap/ (or just roadmap/<wave>/ if wave is set)
    if wave:
        wave_roadmap = repo_root / "roadmap" / wave.name
        if wave_roadmap.is_dir():
            docs.extend(gather_dir(wave_roadmap))
    else:
        # No wave: include all of roadmap/
        docs.extend(gather_dir(repo_root / "roadmap"))

    # ... rest of docs gathering
```

This keeps wave context minimal while ensuring the right roadmap content is in context. Steps like `ingest` can then read the roadmap files directly from context to handle stage ordering.

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
→ 2.5. Wave context (NEW) — just the wave name
3. Reference material (docs, summaries) — includes roadmap/<wave>/
4. Instructions (direction, step)
5. Working context (diff, clipboard, images)
```

Wave section format is minimal:

```markdown
<lf:wave name="rust">
You are building toward the rust program of work.
Roadmap context is included in docs below.
</lf:wave>
```

The roadmap content (`roadmap/rust/README.md`, `01-protocol.md`, etc.) appears in the docs section as normal files. Steps like `ingest` read the roadmap files from context and handle stage ordering themselves.

| Scenario | Wave section | Docs inclusion |
|----------|--------------|----------------|
| **Wave with roadmap** | `<lf:wave name="rust">` | `roadmap/rust/*` |
| **Wave without roadmap** | `<lf:wave name="rust">` | (nothing extra) |
| **No wave detected** | (omitted) | All of `roadmap/` |

### Code changes needed

1. **New module**: `src/loopflow/lf/wave.py`
   - `WaveContext` dataclass (just name + source)
   - `determine_wave()` function
   - `_extract_wave_candidates()` — parse worktree name

2. **Update `PromptComponents`**: Add `wave: WaveContext | None` field

3. **Update `gather_prompt_components()`**: Accept `wave: str | None`, call `determine_wave()`

4. **Update `format_prompt()`**: Add minimal wave section (`<lf:wave name="...">`)

5. **Update docs gathering**: Prioritize `roadmap/<wave>/` when wave is set

6. **Update `ContextConfig`**: Add `wave: str | None` for explicit wave override

7. **Update CLI**: Add `--wave` flag to `lf` commands

8. **Update `lfd/execution/runner.py`**: Pass `wave.name` to `gather_prompt_components()`

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

If `<lf:wave>` is present, look for `roadmap/<wave>/` in the docs:
- Pick from numbered stages in order (01-*, 02-*, etc.)
- README.md provides strategic context, not a pickable item
- Follow principles from README.md when evaluating priority

## Output path

Include wave name in output path: `scratch/<wave>-<slug>.md`
```

### 2. `kickoff.md` — Design aligns with wave

**Update to reference wave context:**

```markdown
## Wave alignment

If `<lf:wave>` is present, check `roadmap/<wave>/README.md` in docs:
- Design must follow the wave's principles
- Scope must exclude wave non-goals
- "Done when" must contribute to wave success criteria

Quote the specific principles you're following in "Key decisions".
```

### 3. `implement.md` — Follow wave principles

**Update to reference wave context:**

```markdown
## Wave context

If `<lf:wave>` is present, check `roadmap/<wave>/README.md` in docs:
- Follow the wave's principles during implementation
- Check against compatibility matrix if mentioned
- Note drift from wave constraints in `scratch/questions.md`
```

### 4. `add-to-roadmap.md` — Use wave context for routing

**Update wave routing:**

```markdown
## Wave routing

If `<lf:wave>` is present:
- Route actionable items to `roadmap/<wave>/`
- Check existing stage structure in docs
- Add to appropriate stage, or propose new stage if work is a new phase

Otherwise, infer wave from design doc references or existing roadmap structure.
```

### 5. `LOOPFLOW.md` — Add wave section

**Add after "Run Modes":**

```markdown
## Wave

If you're building toward a program of work, you'll see `<lf:wave name="...">` in context.

**What it tells you:**
- **Wave name**: Which roadmap you're contributing to (e.g., `rust`, `enterprise`)

**Where to find roadmap context:**
- Look for `roadmap/<wave>/` in the docs section
- `README.md` has principles, non-goals, success criteria
- Numbered files (01-*, 02-*) are staged roadmap items

**How to use it:**
- Follow wave principles in design and implementation decisions
- Note drift from constraints in `scratch/questions.md`
- Route new work items to the same wave's roadmap

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
src/loopflow/lf/design.py                          # Update docs gathering to prioritize roadmap/<wave>/
src/loopflow/lf/step.py                            # Add --wave flag, pass to gather_prompt_components()
src/loopflow/lfd/execution/runner.py               # Pass wave.name to context gathering
```

**Prompts (leverage wave context):**
```
src/loopflow/lf/builtins/steps/plan/ingest.md      # Use wave context for staged selection
src/loopflow/lf/builtins/steps/plan/kickoff.md     # Reference wave from prompt context
src/loopflow/lf/builtins/steps/code/implement.md   # Check wave constraints from context
src/loopflow/lf/builtins/steps/ops/add-to-roadmap.md  # Use wave context for routing
src/loopflow/LOOPFLOW.md                           # Add "Wave" section
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
# → WaveContext(name='rust', source='explicit')

# Wave appears in prompt
lf review --wave rust 2>&1 | grep -q "lf:wave"  # Wave section in output

# Wave roadmap included in docs
lf review --wave rust 2>&1 | grep -q "roadmap/rust"  # roadmap/<wave>/ in docs

# Inference works from worktree name
cd ../loopflow.rust-protocol && lf review 2>&1 | grep -q "lf:wave"
```
