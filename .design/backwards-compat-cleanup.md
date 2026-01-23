# Plan: Remove Backwards Compat + Consolidate Context Assembly

Implements Opportunities 1 and 2 from `.design/simplification-opportunities.md`.

---

## Part 1: Remove Internal Backwards Compatibility

### Python: Delete `ignore` field

**Files:**
- `src/loopflow/lf/config.py`
  - Delete line 100: `ignore: list[str] = Field(default_factory=list)`
  - Delete lines 133-138: `split_ignore_string()` validator
  - Delete lines 167-173: `merge_ignore_into_exclude()` validator
- `.lf/config.yaml`
  - Line 12: Change `ignore: "uv.lock"` → add to `exclude` list
- `tests/test_config.py`
  - Delete lines 357-435: All 5 `ignore` tests

### Python: Delete `include_tests_for` field

**Files:**
- `src/loopflow/lf/config.py`
  - Delete line 101: `include_tests_for: Optional[list[str]] = None`
  - Delete lines 140-145: `split_include_tests_for_string()` validator
  - Delete lines 245-254: Deprecation warning
- `src/loopflow/lf/frontmatter.py`
  - Delete lines 219-227: `include_tests_for` handling in `resolve_step_config()`
- `src/loopflow/lf/context.py`
  - Remove `include_tests_for` parameter from `gather_prompt_components()` (line 628)
  - Delete lines 678-682: `include_tests_for` logic
  - Remove from `build_prompt()` (lines 814, 826)
- `src/loopflow/lf/step.py`
  - Delete line 704: `include_tests_for=config.include_tests_for if config else None,`
- `tests/test_frontmatter.py`
  - Delete lines 195-221: Both `include_tests_for` tests

### Swift: Delete typealiases

**Files:**
- `swift/LoopflowCore/Services/JobService.swift`
  - Delete lines 266-270: All 4 typealiases
- `swift/Concerto/AppState.swift`
  - Line 134: Change `LoopService()` → `LfdService()`
- `swift/Concerto/Views/LoopRow.swift`
  - Delete lines 72-73: `typealias LoopRow = AgentRow`

### TypeScript: Delete loop.ts adapter, fix LoopStatus.tsx

**Files:**
- `web/src/models/loop.ts` — DELETE entire file
- `web/src/components/LoopStatus.tsx`
  - Rename to `AgentStatus.tsx`
  - Change import from `loop` to `job`
  - Change `Loop` → `Agent`, `LoopStatus` → `AgentStatus`
  - Change `loop.shortId` → `agentShortId(agent)` (use helper function)
  - Change `loop.statusText` → `agentStatusText(agent)`

---

## Part 2: Consolidate Context Assembly

### Step 2a: Create ContextSpec Pydantic model

**File:** `src/loopflow/lf/context.py`

Add import and class (following codebase pattern from config.py, flows.py):

```python
from pydantic import BaseModel

class ContextSpec(BaseModel):
    """Specifies what context to include in a prompt."""
    lfdocs: bool = True
    diff: bool = False
    diff_files: bool = True
    summaries: bool = True
    paste: bool = False

    @classmethod
    def for_flow(cls) -> "ContextSpec":
        return cls(lfdocs=True, diff=False, diff_files=True, summaries=True, paste=False)

    @classmethod
    def for_commit(cls) -> "ContextSpec":
        return cls(lfdocs=False, diff=True, diff_files=False, summaries=False, paste=False)

    @classmethod
    def for_interactive(cls) -> "ContextSpec":
        return cls(lfdocs=True, diff=False, diff_files=False, summaries=True, paste=False)
```

### Step 2b: Update gather_prompt_components signature

**File:** `src/loopflow/lf/context.py`

Change from:
```python
def gather_prompt_components(
    repo_root: Path,
    step: Optional[str] = None,
    ...
    include_loopflow_doc: bool = True,
    include_diff: bool = False,
    include_diff_files: bool = True,
    include_summaries: bool = True,
    ...
) -> PromptComponents:
```

To:
```python
def gather_prompt_components(
    repo_root: Path,
    step: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    step_args: Optional[list[str]] = None,
    paste: bool = False,
    run_mode: Optional[str] = None,
    voices: Optional[list[str]] = None,
    spec: Optional[ContextSpec] = None,
    config=None,
) -> PromptComponents:
    """
    If spec is None, uses ContextSpec() defaults.
    Individual bool params (paste) override spec for convenience.
    """
    if spec is None:
        spec = ContextSpec()
    # Use spec.lfdocs, spec.diff, spec.diff_files, spec.summaries internally
```

### Step 2c: Update call sites

**11 call sites to update:**

| File | Current | New |
|------|---------|-----|
| `step.py:215` | `include_diff_files=False, include_loopflow_doc=...` | `spec=ContextSpec.for_interactive()` |
| `step.py:413` | `include_diff=..., include_diff_files=...` | `spec=ContextSpec(diff=include_diff, diff_files=include_diff_files, ...)` |
| `step.py:567` | defaults | `spec=ContextSpec()` (or omit) |
| `step.py:699` | defaults | `spec=ContextSpec.for_flow()` |
| `flow.py:95` | `include_loopflow_doc=True, include_diff=False, ...` | `spec=ContextSpec.for_flow()` |
| `flow.py:327` | defaults | `spec=ContextSpec.for_flow()` |
| `commit.py:70` | `include_diff=True, include_diff_files=False, ...` | `spec=ContextSpec.for_commit()` |
| `runner.py:67` | defaults | `spec=ContextSpec.for_flow()` |
| `runner.py:162` | defaults | `spec=ContextSpec.for_flow()` |
| `context.py:820` | pass-through | `spec=spec` (accept spec param in build_prompt) |

---

## Verification

1. **Run Python tests:**
   ```bash
   uv run pytest tests/test_config.py tests/test_frontmatter.py tests/test_context.py -v
   ```

2. **Run Swift tests:**
   ```bash
   swift test --package-path swift
   ```

3. **Manual smoke test:**
   ```bash
   lf review -c  # verify context assembly works
   lf debug -v   # verify paste still works
   ```

4. **Verify no `ignore` or `include_tests_for` references remain:**
   ```bash
   rg "ignore.*list\[str\]|merge_ignore_into_exclude|include_tests_for" src/
   ```

---

## Order of Operations

1. Delete Python backwards compat (`ignore`, `include_tests_for`)
2. Delete Swift typealiases, update AppState
3. Delete TypeScript `loop.ts`, rename/fix `LoopStatus.tsx`
4. Create `ContextSpec` dataclass
5. Update `gather_prompt_components()` signature
6. Update all call sites
7. Run tests, fix any failures
