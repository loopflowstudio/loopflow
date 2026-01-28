# Polish Priorities

## Priority 1: Documentation command syntax mismatch

**Evidence**:
- `docs/waves.md:11` shows `lfd loop ship src/api/ --direction product-engineer` but actual syntax is `lfd loop <wave-name> --area src/api/ --flow ship -d product-engineer`
- `docs/waves.md:30` shows `lfd run ship swift/` but actual syntax requires wave name first
- `docs/waves.md:40` shows `lfd loop ship src/` - same issue
- `docs/waves.md:52-54` shows `lfd watch ship src/api/` - same issue
- `docs/waves.md:63-65` shows `lfd cron ship . "0 9 * * *"` - same issue
- `docs/waves.md:73-74` shows `lfd loop ship src/` - same issue
- `docs/waves.md:84-88` shows `lfd run ship src/` - same issue
- `docs/index.md:154-155` shows `lfd loop ship src/api/` - same issue
- `docs/index.md:161` shows `lfd loop ship src/api/ --direction product-engineer` - same issue
- `docs/getting-started.md:104` shows `lfd loop ship src/` - same issue
- `docs/troubleshooting.md:83` shows `lfd loop <flow> <area> --limit 10` - same issue
- Running `lfd loop ship src/` produces: `Got unexpected extra argument (src/)`

**Impact**: Users following documentation will get errors. The entire `lfd` documentation describes a syntax that doesn't exist.
**Effort**: Medium - need to update all wave documentation consistently
**Recommendation**: Fix immediately. Either update docs to match current CLI, or update CLI to match documented syntax. Current docs describe a cleaner API (`lfd loop ship src/`) that users would prefer.

## Priority 2: Terminology inconsistency (task vs step)

**Evidence**:
- `lfops doctor` outputs "no task files (run: lf init)" at `init.py:39`
- README says "Steps are prompts that run coding agents" and uses `.lf/steps/` path
- `docs/index.md:53` says "Step | Runs a prompt with assembled context"
- `docs/lf.md:31-38` describes "Steps" and searches `.lf/steps/`
- `docs/troubleshooting.md:32,42` mixes "Task" and "step" terminology
- Tests reference "task file" (`test_context.py:115,218,326`)
- `messages.py` uses `task_prompt` variable name and `<lf:task>` XML tag

**Impact**: Confusing for users. "Task" in doctor output doesn't match "step" in documentation.
**Effort**: Low - change doctor message from "task files" to "step files" or ".lf/steps/ and .claude/commands/"
**Recommendation**: Fix doctor message to use consistent terminology: "no step files found (expected .lf/steps/*.md or .claude/commands/*.md)"

## Priority 3: Help text examples have formatting issues

**Evidence**:
- `lfd loop --help` shows:
  ```
  Examples:     lfd loop swift-falcon                                   # run
  existing wave     lfd loop swift-falcon --area src/                       #
  create + set area + run     lfd loop swift-falcon --area src/ -d concise -d
  fast
  ```
  The examples wrap incorrectly with words split mid-line.
- `lfd watch --help` has similar formatting issues
- `lfd cron --help` has similar formatting issues
- `lfops wt create --help` has similar word-wrap issues

**Impact**: Help text is harder to read. Users may miss important examples.
**Effort**: Low - use explicit `\n` or Rich formatting in help strings
**Recommendation**: Format example blocks with proper newlines in the Typer help strings.

## Priority 4: Missing explanation for --direction flag

**Evidence**:
- `lf run --help` shows: `--direction -d,-D TEXT Direction to apply (repeatable, or comma-separated)`
- No explanation of what directions are, where they come from, or examples
- Compare to `docs/config.md:225-232` which explains "Directions shape judgment and intent"

**Impact**: Users must read docs to understand what directions do or what values are valid.
**Effort**: Low - add brief explanation like "Direction from .lf/directions/ or built-in (product-engineer, designer, infra-engineer, ceo)"
**Recommendation**: Update help text with brief context about where directions come from.

## Priority 5: Config doc shows wrong flow syntax

**Evidence**:
- `docs/config.md:91-98` shows:
  ```python
  def flow():
      return Flow(["implement", "rebase", "polish", "draft_commit"])
  ```
- But `docs/index.md:107-112` and actual usage show YAML format:
  ```yaml
  - implement
  - compress
  - gate
  ```
- Line 64 says "Flows are defined in `.lf/flows/<name>.yaml`" which matches YAML format
- The Python function syntax appears to be outdated

**Impact**: Users may create Python flows that don't work.
**Effort**: Low - remove or update the Python example
**Recommendation**: Remove the Python example from config.md and use only YAML format consistently.

## Priority 6: lf flow syntax inconsistency in index.md

**Evidence**:
- `docs/index.md:115` shows `lf --flow ship`
- But `lf flow --help` shows: `lf flow NAME` - the actual command
- `docs/lf.md:104-107` correctly shows `lf flow <name>`

**Impact**: Minor confusion - users may try `lf --flow ship` which doesn't work.
**Effort**: Low - fix to `lf flow ship`
**Recommendation**: Change `lf --flow ship` to `lf flow ship`.

## Lower priority

### Uppercase flag aliases add visual noise
`--auto -a,-A` shows both lowercase and uppercase aliases. The uppercase variants are shown but rarely needed.
**Note**: Listed in existing `reports/cli/ux-polish.md` as lower priority.

### Error message for missing wave could be clearer
`lfd rm test-wave` after wave created in different repo shows "Wave 'test-wave' not found" without explaining this is a repo-scoped error.
**Note**: Edge case, low impact.

### Flow format inconsistency between docs
- `docs/config.md:91-98` shows Python flow syntax
- `docs/index.md:107-112` shows YAML flow syntax
- `builtins/steps/ops/init.md:120-159` shows Python flow syntax with `def flow():`
- Both formats appear to be supported but documentation doesn't clearly explain which to use when
**Note**: Medium priority - causes confusion about which format to use.
