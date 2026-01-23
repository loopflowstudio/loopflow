# Polish Priorities

## Docs updated

- **docs/lf.md**: Fixed `-x` → `-p` for context file flag (3 occurrences)
- **docs/lfd.md**: Fixed `lfd flow` → `lfd run` command name
- **docs/lfd.md**: Fixed argument order for `lfd loop`, `lfd run`, `lfd schedule`
- **docs/lfd.md**: Changed flow definition example from YAML to Python (matches actual)
- **docs/lfd.md**: Removed non-existent `--goal` flag, added correct `-L, --voice` flag
- **docs/config.md**: Fixed `-x` → `-p` for context file flag (2 occurrences)
- **docs/agents.md**: Fixed `lfd loop src/ --flow ship` → `lfd loop ship src/`
- **docs/agents.md**: Fixed `lfd flow` → `lfd run` and updated syntax
- **docs/index.md**: Fixed `lfd loop src/ --flow ship` → `lfd loop ship src/`
- **README.md**: Fixed `lfd loop src/ --flow ship` → `lfd loop ship src/`
- **docs/loops-demo.tape**: Fixed `lfd flow` → `lfd run` in VHS demo
- **docs/lfops.md**: Added documentation for `lfops wt create`, `lfops wt list`, `lfops wt ci`, `lfops abandon`
- **src/loopflow/lfd/__init__.py**: Fixed "voicees" → "voices" typo in output

## Priority 1: Flag naming convention (`-L` for voice)

**Evidence**:
- `lf run --help` shows `--voice -L` for voice selection
- `-L` is an unusual mnemonic for "voice" (typically `-v`, but that's taken by `--paste`)
- Users may try `--voice` first, but `-L` is the short form documented nowhere
- The `-v` → `--paste` and `-L` → `--voice` pairing is counterintuitive

**Impact**: Users must memorize an arbitrary flag letter. The `-L` choice provides no hint about its purpose. New users typing `-v` for voice will accidentally paste clipboard content.

**Effort**: Medium (breaking change for existing users who use `-L`)

**Recommendation**: Consider `-V` for voice (uppercase) since `-v` is taken. Or keep `-L` but document it prominently in the quick reference. The current state is technically correct but ergonomically poor.

## Priority 2: Inconsistent context file flag naming

**Evidence**:
- `lf run` uses `-p, --path` for additional context files
- Config file uses `context: [...]` for the same concept
- Docs previously said `-x` which doesn't exist
- `lfops cp` has no equivalent flag (uses positional args)

**Impact**: The mental model is unclear. Is it "path" or "context" or "extra files"? Users must remember different names for the same concept in different contexts.

**Effort**: Low (documentation only, flag already works)

**Recommendation**: Update config.md to explain the mapping: CLI `-p/--path` corresponds to config `context:`. Consider renaming the config key to `paths:` for consistency in a future version.

## Priority 3: Missing module READMEs

**Evidence**:
- `src/loopflow/` has no README
- `src/loopflow/lf/` has no README
- `src/loopflow/lfd/` and submodules have no READMEs
- `src/loopflow/lfops/` has no README

**Impact**: New contributors have no entry point to understand module structure. The codebase is navigable via code inspection but lacks overview documentation.

**Effort**: Medium (requires understanding each module's purpose)

**Recommendation**: Per STYLE.md, not every module needs a README—only those with user-facing behavior. The CLI commands are well-documented in `docs/`. Consider adding a top-level `src/loopflow/README.md` as an architecture overview for contributors.

## Priority 4: Flow definition format confusion

**Evidence**:
- docs/lfd.md showed flows as YAML frontmatter (incorrect, now fixed)
- Actual flows are Python files returning `Flow()` objects or dicts
- `.lf/flows/ship.py` uses complex `Flow()` with `Choose()` and `fork`
- `.lf/flows/submit.py` uses simple `{"steps": [...]}` dict
- No documentation on advanced flow features (Choose, fork, join)

**Impact**: Users can create simple flows but have no guidance on advanced features like model racing or conditional execution.

**Effort**: High (requires documenting undocumented features)

**Recommendation**: Create `docs/flows.md` explaining flow authoring, or expand `docs/next/pipelines.md` from "Coming soon" to actual documentation.

## Priority 5: Commands in "Coming soon" state

**Evidence**:
- `docs/next/agents.md` marked "Coming soon" but lfd already exists
- `docs/next/pipelines.md` marked "Coming soon" but flows work
- `docs/next/concerto.md` and `docs/next/multi-model.md` also "Coming soon"

**Impact**: Users may not discover features that already work. The "next" folder creates confusion about what's implemented.

**Effort**: Medium (documentation migration)

**Recommendation**: Move implemented features from `docs/next/` to main `docs/`. Keep only truly unimplemented features in `docs/next/`.

## Lower priority

- **lfd list-voices in wrong place**: The `list-voices` command is under `lfd` but voices are used by `lf`. Consider `lf list-voices` or `lfops list-voices` for discoverability.

- **lfops vs lf boundary unclear**: Some operations like `commit`, `pr`, `land` are in `lfops` but could arguably be `lf` commands since they're part of the coding workflow. The current split (lf = prompts, lfops = operations) is logical but not immediately obvious.

- **GIF demos may be outdated**: After fixing VHS tapes, the generated GIFs may not match current behavior. Run `make gifs` in docs/ to regenerate.
