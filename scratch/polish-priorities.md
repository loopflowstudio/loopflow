# Polish Priorities

## Priority 1: Documentation shows wrong command names

**Evidence**:
- `docs/waves.md:22` says `lfd subscribe` but the command is `lfd watch`
- `docs/waves.md:52-53` shows `lfd subscribe ship src/api/` — command doesn't exist
- `docs/waves.md:23,63-64` says `lfd schedule` but the command is `lfd cron`
- `README.md:121` shows `lfd subscribe ship designs/` — command doesn't exist
- `README.md:114` shows `lfd create engbot --area src/` but `lfd create` doesn't accept `--area`
- `RELEASE_NOTES.md:10` says `lfops cycle` but the command is `lfops next`

**Impact**: Users copying examples from docs get "No such command" errors. First-time users will hit this immediately.

**Effort**: Low — straightforward find/replace in docs

**Recommendation**: Fix before next release. These are the primary user-facing docs.

---

## Priority 2: Stale "goal" naming in code and docs

**Evidence**:
- `src/loopflow/lfd/execution/README.md:29-33,43-45` uses `{"goal": "infra-engineer"}` but `ForkAgent` uses `direction`
- `src/loopflow/lf/builtins/steps/ops/init.md:150-151` uses `{"goal": "product-engineer"}` in example
- `src/loopflow/lfd/migrations/m_2026_01_24_nullable_goal_area.py` — filename still says "goal"
- RELEASE_NOTES mentions the rename from "voice" to "goal" to "direction" confirming this was intentional

**Impact**: Developers reading execution README will write invalid Fork configs. Init step example is wrong.

**Effort**: Low — update docs and optionally rename migration file

**Recommendation**: Fix docs immediately. Migration filename is lower priority (cosmetic).

---

## Priority 3: "Agent" vs "Wave" terminology inconsistency

**Evidence**:
- `src/loopflow/lfd/execution/README.md:8,40,84,89` uses "Agent" throughout diagrams and headings
- CLI and main README use "Wave" consistently
- Recent commit "naming: rename agent→wave" confirms this rename happened

**Impact**: Confusing for developers reading internal docs. Creates cognitive dissonance between CLI and codebase docs.

**Effort**: Low — update execution README

**Recommendation**: Fix alongside Priority 2.

---

## Priority 4: Missing `lfops next` documentation

**Evidence**:
- `lfops --help` shows `next` command: "Land current PR, create fresh worktree in same space"
- `docs/lfops.md` has no mention of `next` command
- This is a user-facing workflow command that should be documented

**Impact**: Users won't discover this useful workflow command.

**Effort**: Low — add section to lfops.md

**Recommendation**: Add documentation for the command.

---

## Priority 5: Help text for `--direction` is too terse

**Evidence**:
- `lf run --help` shows `--direction -d,-D TEXT Direction to apply (repeatable, or comma-separated)`
- No explanation of what a direction is or where they come from
- Users need to know directions come from `.lf/directions/*.md` or built-ins
- Compare to `--path` which says "Additional files to include" — similarly terse but less mysterious

**Impact**: New users won't understand what directions are from help text alone. They'll need to find the docs.

**Effort**: Medium — improve help strings in CLI code

**Recommendation**: Add brief context like "Direction from .lf/directions/ or built-in (e.g., product-engineer)"

---

## Priority 6: Confusing uppercase flag aliases

**Evidence**:
- `lf run --help` shows `--auto -a,-A` with uppercase alias
- Same for `--interactive -i,-I`, `--path -p,-P`, `--model -m,-M`
- Uppercase aliases add visual noise without clear benefit
- Some commands don't have them, creating inconsistency

**Impact**: Minor confusion. Help text is busier than needed.

**Effort**: Low — remove uppercase aliases or document the convention

**Recommendation**: Low priority. Consider removing in future cleanup.

---

## Lower priority

### Doctor output could be clearer

Running `lfops doctor` shows `- no task files (run: lf init)` but this message is cryptic. What are "task files"? Should say ".lf/steps/ or .claude/commands/ not found" or similar.

### Migration filename

`m_2026_01_24_nullable_goal_area.py` still uses "goal" in filename. Cosmetic issue, migration code itself is correct.

### Reference to docs/lfd.md

`src/loopflow/lfd/README.md:22` says "See `docs/lfd.md` for the full CLI reference" — this reference is valid but should be verified as part of docs audit.

### Help text example formatting

`lfd loop --help` example shows word-wrap artifacts:
```
Examples:     lfd loop swift-falcon                                   # run
existing wave     lfd loop swift-falcon --area src/
```

The formatting is awkward with runs of spaces. Consider using actual newlines in help strings.

