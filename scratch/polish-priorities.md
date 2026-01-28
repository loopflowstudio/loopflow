# Polish Priorities

## Priority 1: Documentation mentions `reports/` inconsistently

**Evidence**:
- `--lfdocs` help text says "Include reports/, roadmap/, scratch/, and .md files"
- Code includes `reports/` in `_DEFAULT_FILE_PATHS` at `context.py:148`
- docs/index.md "Where Files Live" section doesn't mention `reports/`
- docs/config.md doesn't mention `reports/`
- `reports/` folder exists and contains 9 reference docs (landscape.md, target-customer.md, etc.)

**Impact**: Users don't know about `reports/` as a first-class concept. The docs mention `scratch/` vs `roadmap/` but not `reports/` for reference material. Users may not understand where to put research and analysis that should persist.

**Effort**: Low (documentation update)

**Recommendation**: Add `reports/` to the file structure docs. Update "scratch/ vs roadmap/" section to explain all three: scratch/ (ephemeral), roadmap/ (actionable items), reports/ (reference material).

---

## Priority 2: Help text word-wrap artifacts in `lfd` commands

**Evidence**:
- `lfd loop --help` shows:
  ```
  Examples:     lfd loop swift-falcon                                   # run
  existing wave     lfd loop swift-falcon --area src/                       #
  create + set area + run
  ```
- Same issue in `lfd run --help`, `lfd watch --help`, `lfd cron --help`
- Caused by inline examples in docstrings without proper line breaks

**Impact**: Help text is hard to read. Users have to mentally parse the wrapped output to understand the examples.

**Effort**: Low (add `\n` to docstrings in `cli.py`)

**Recommendation**: Reformat examples in docstrings to use explicit newlines so Typer renders them correctly.

---

## Priority 3: `--direction` flag lacks context

**Evidence**:
- `lf run --help` shows: `--direction -d,-D TEXT Direction to apply (repeatable, or comma-separated)`
- No mention of where directions come from (`.lf/directions/`) or built-in options

**Impact**: New users don't understand what a "direction" is or how to use one. They'd need to read documentation to learn about `product-engineer`, `designer`, etc.

**Effort**: Low (update help text)

**Recommendation**: Change to: "Direction from .lf/directions/ or built-in (e.g., product-engineer)"

---

## Priority 4: Doctor output uses cryptic message

**Evidence**:
- `lfops doctor` shows: `- no task files (run: lf init)`
- Doesn't explain what "task files" means or where they should be

**Impact**: Users seeing this message don't know what's missing or where to look.

**Effort**: Low (string change in `init.py:39`)

**Recommendation**: Change to: `.lf/steps/ or .claude/commands/ not found (run: lf init)`

---

## Priority 5: Uppercase flag aliases add visual noise

**Evidence**:
- `--auto -a,-A`, `--interactive -i,-I`, `--clipboard -c,-C` etc.
- Appears in help text for `lf run`, `lf inline`, and other commands
- The uppercase variants exist for historical compatibility but clutter help output

**Impact**: Minor visual noise. Help output is longer and harder to scan.

**Effort**: Low (remove uppercase aliases)

**Recommendation**: Defer. The uppercase aliases may have users depending on them. Consider removing in a future cleanup pass.

---

## Lower priority

### Step descriptions in `lf --list`

The `explore` step description says "Investigate current diff" but the step actually explores the codebase generally, not just the diff. Consider: "Investigate the codebase".

### Error message for missing wave

`lfd show nonexistent` shows: `Error: Wave 'nonexistent' not found`

Could add: `Run 'lfd list' to see available waves.`
