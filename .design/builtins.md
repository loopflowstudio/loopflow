# Builtins

Built-in commands that work without `lfops init`. User commands in `.lf/` or `.claude/commands/` override builtins.

## What to build

Tasks resolve to bundled templates when no user-defined task file exists, eliminating the need for `lfops init` before running core commands.

## Data structures

No new types. The change is in resolution logic.

## Key functions

```python
def _get_builtin_task(name: str) -> Path | None:
    """Return path to bundled template if it exists."""
    builtin = Path(__file__).parent / "templates" / "commands" / f"{name}.md"
    return builtin if builtin.exists() else None

def gather_task(repo_root: Path, name: str) -> TaskFile | None:
    """Gather task file. User files override builtins."""
    # 1. Check user locations (existing logic)
    #    .claude/commands/{name}.md
    #    .lf/{name}.lf
    #    .lf/{name}.md
    #    .lf/{name}.*
    # 2. Fall back to builtin
    #    templates/commands/{name}.md
```

## Resolution order

1. `.claude/commands/{name}.md` (user override)
2. `.lf/{name}.lf` (user override)
3. `.lf/{name}.md` (user override)
4. `.lf/{name}.*` (user override, any extension)
5. `templates/commands/{name}.md` (builtin fallback)

## Changes to `lfops init`

Keep `lfops init` but make it optional:
- Copy config template (still useful for pipelines, context settings)
- Copy STYLE.md, PROMPTS.md if user wants repo-specific guidance
- **Remove command copying entirely** — they're built-in now

The `--prompts` flag and related options can be removed.

## Discoverability

Users need to know what builtins exist:

1. **`lf` with no args** — list available tasks (builtins + user-defined)
2. **Tab completion** — complete builtin names
3. **Maestro task selector** — show builtins even when no `.lf/` exists

## UI changes (Maestro)

The task selector dropdown currently reads from `.lf/` and `.claude/commands/`. Update to:

1. Always include builtin task names in the dropdown
2. Mark builtins differently if desired (e.g., dimmed, or with "(builtin)" suffix)
3. User-defined tasks with same name override builtins (no duplicate entries)

## Constraints

- Builtins must be discoverable for `lf --help` or tab completion
- Must work when no `.lf/` directory exists at all
- User override must be complete (no merging of builtin + user)

## Builtin commands (final set)

- **design** — Create implementation spec in `.design/`. Interactive.
- **implement** — Turn design doc into code.
- **review** — Written assessment, verdict in `.design/`.
- **iterate** — One focused improvement to branch code.
- **polish** — Fix issues, run tests, get to green.
- **debug** — Debug error from clipboard (`-v`).
- **reduce** — Simplify code while preserving user behavior. (NEW)
- **expand** — Explore ambitious changes beyond current scope. (NEW)
- **explore** — Interactive Q&A about the current diff. Let the human drive. (NEW)

Out of scope (stay as `lfops` commands):
- `lfops commit` — already exists, may get upgraded separately
- `lfops compare` — comparing worktrees, better as ops tool

## New prompt sketches

### reduce.md

```markdown
---
requires: diff vs main
produces: simpler code
---
Simplify code touched by this branch while preserving user behavior.

## Goal

Less code that does the same thing. Delete what isn't needed. Flatten unnecessary abstractions. The bar is: if a user ran the same workflows before and after, they wouldn't notice a difference—except maybe things are faster or error messages are clearer.

## What to simplify

Focus only on code this branch modified.

**Dead code.** Unused functions, unreachable branches, commented-out code.

**Over-abstraction.** Layers that don't earn their keep. Inheritance that could be composition. Generics that are only ever used with one type.

**Duplication.** Copy-pasted logic that could be a function. But don't create abstractions for two similar lines.

## What to preserve

**User-visible behavior.** Same inputs, same outputs. Same error messages. Same side effects.

**Performance characteristics.** Don't make things slower to make them prettier.

**Test coverage.** Tests should still pass. If a test fails, the simplification went too far.
```

### expand.md

```markdown
---
requires: diff vs main
produces: ambitious improvements
---
Explore ambitious changes that extend what this branch is already doing.

## Goal

Push beyond the immediate scope. If the branch adds feature X, what would make X great instead of just done? What adjacent features become easy now? What technical debt could be paid down while the context is fresh?

This is exploratory—propose ideas, implement the best one. The human can reject or redirect.

## What to explore

**Natural extensions.** The branch adds auth—what about password reset? The branch adds caching—what about cache invalidation UI?

**Quality upgrades.** The branch works—could it be fast? Could errors be more helpful? Could the API be more intuitive?

**Debt paydown.** Code nearby that's been annoying. Patterns that should be updated to match the new code.

## Constraints

**One thing.** Pick the highest-impact extension and do it well. Don't scatter effort.

**Stay coherent.** The expansion should feel like it belongs with the original branch work. If it's unrelated, it belongs in a different branch.

**Tests required.** New behavior needs tests. This isn't a prototype.
```

### explore.md

```markdown
---
interactive: true
requires: diff vs main
---
Answer questions about the current diff. Let the human drive.

## Role

You're a knowledgeable colleague who's read the diff. The human has questions—answer them. Don't volunteer opinions or suggest changes unless asked. Don't write code unless asked. Don't take initiative.

Start by briefly summarizing what the diff contains (2-3 sentences max), then wait for questions.

## Good responses

- Direct answers to what was asked
- "I don't see that in the diff" when something isn't there
- Asking clarifying questions when the question is ambiguous
- Code snippets when the human asks for them

## Bad responses

- Unsolicited suggestions ("you might also want to...")
- Preemptive reviews ("I notice a potential issue...")
- Writing code without being asked
- Long explanations when a short answer suffices

The human is in charge. Follow their lead.
```

## Done when

```bash
# Fresh repo with no .lf/ directory
cd /tmp && mkdir test-repo && cd test-repo && git init
lf design: add user auth
# Should launch design task using builtin template
```
