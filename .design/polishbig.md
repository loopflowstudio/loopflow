# polishbig

Docs updates, flag refactoring, and module reorganization.

## Review

**Verdict:** Needs work

The branch has substantial doc fixes and a flag refactoring that improves CLI ergonomics. However, there are issues that should be addressed before shipping.

### Issues

1. **README out of sync** — README.md still shows `lfd loop src/ --flow ship` but the docs and code now use `lfd loop ship src/` (flow before area). The README fix is uncommitted.

2. **Incomplete flag mapping in docs/lfd.md** — Changed `-g, --goal` to `-v, --voice` but the semantics don't match. Goals and voices are different concepts. Looking at the actual code in `cli.py:222-226`, the flag is genuinely for voices now, but the design doc and help text still reference the old behavior in places.

3. **Example in lfd.md uses wrong flag** — Line 237 shows `-L product-engineer` but the flag was changed to `-v, --voice`. The `-L` alias doesn't exist.

4. **Missing test coverage** — `tests/test_cli_ops.py` import changed from `loopflow.lf` to `loopflow.lf.cli`, but there are no tests for the new flag behavior (`lf -m codex`, `lf --web` without step).

5. **docs/lfd.md shows YAML flow format** — The committed change in `.design/polish-priorities.md` says this was fixed to Python, but the file still shows:
   ```yaml
   ---
   steps:
     - implement
   ...
   ```
   The git diff shows this was actually changed to Python format, so this is correct—the `.design/polish-priorities.md` tracking doc needs updating since it was already done.

### Minor

- The `lfops wt list` command documented in `.design/polish-priorities.md` doesn't appear in `docs/lfops.md` or the actual changes.
- `lfops wt ci` and `lfops abandon` mentioned as documented but not visible in the diff.

## Design notes

### Flag refactoring rationale

The flag changes improve mnemonics:
- `-c` for clipboard (was `-v` for "paste")
- `-v` for voice (was `-L`)
- Removed `-c, --copy` (redundant with `--web`)

This follows the STYLE.md guidance on CLI flag naming: "Prefer lowercase short flags (`-p`, `-c`), support uppercase as aliases."

### Module reorganization

CLI code moved to dedicated `cli.py` files:
- `src/loopflow/lf/__init__.py` → `src/loopflow/lf/cli.py`
- `src/loopflow/lfd/__init__.py` → `src/loopflow/lfd/cli.py`

This keeps `__init__.py` clean per STYLE.md: "Keep `__init__.py` files empty. They exist only to mark directories as packages."

### What's left from polish-priorities.md

Tracked items not addressed:
- Priority 2: `-p/--path` vs `context:` config naming inconsistency
- Priority 3: Missing module READMEs
- Priority 4: Advanced flow features undocumented
- Priority 5: docs/next/ "Coming soon" content stale
