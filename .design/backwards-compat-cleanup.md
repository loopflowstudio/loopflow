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

### Step 2a: Create ContextConfig Pydantic model

**File:** `src/loopflow/lf/context.py`

Added import and class (following codebase pattern from config.py, flows.py):

```python
from pydantic import BaseModel

class ContextConfig(BaseModel):
    """Specifies what context to include in a prompt."""

    # Explicit file paths to include/exclude
    pathset: list[str] = []
    exclude: list[str] = []

    # Flags for what kinds of context to include
    lfdocs: bool = True
    diff: bool = False
    diff_files: bool = True
    summaries: bool = True
    clipboard: bool = False

    @classmethod
    def for_commit(cls) -> "ContextConfig":
        return cls(lfdocs=False, diff=True, diff_files=False, summaries=False)

    @classmethod
    def for_interactive(
        cls,
        pathset: list[str] | None = None,
        exclude: list[str] | None = None,
        lfdocs: bool = True,
        summaries: bool = True,
        clipboard: bool = True,
    ) -> "ContextConfig":
        return cls(
            pathset=pathset or [],
            exclude=exclude or [],
            lfdocs=lfdocs,
            diff_files=False,
            summaries=summaries,
            clipboard=clipboard,
        )
```

### Step 2b: Update gather_prompt_components signature

**File:** `src/loopflow/lf/context.py`

Changed signature to accept `context_config: ContextConfig | None`:

```python
def gather_prompt_components(
    repo_root: Path,
    step: Optional[str] = None,
    inline: Optional[str] = None,
    step_args: Optional[list[str]] = None,
    run_mode: Optional[str] = None,
    voices: Optional[list[str]] = None,
    context_config: ContextConfig | None = None,
    config=None,
) -> PromptComponents:
    """
    If context_config is None, uses ContextConfig() defaults.
    """
    if context_config is None:
        context_config = ContextConfig()
    # Use context_config.lfdocs, .diff, .diff_files, .summaries, .clipboard internally
```

### Step 2c: Updated call sites

All call sites now use `context_config=ContextConfig(...)` or factory methods:

- `step.py`: Uses `ContextConfig.for_interactive()` for interactive mode, `ContextConfig()` for flows
- `flow.py`: Uses `ContextConfig()` with explicit settings
- `commit.py`: Uses `ContextConfig.for_commit()`
- `runner.py`: Uses `ContextConfig()` defaults
- `cp.py`: Uses `ContextConfig.for_interactive()` for clipboard/copy operations

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
