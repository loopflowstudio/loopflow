---
status: in-progress
phase: 1
---

# Decouple screenshots from publish

## Problem

Screenshot generation is coupled to `_release()` in `publish.py`. Publishing no longer submits PRs, so screenshots generated during release get left as dirty files on main. The `--skip-screenshots` flag exists as a workaround, not a solution.

Two redundant paths to the same script: `_release()` calls `_generate_screenshots()`, and `publish.py screenshots` calls `_generate_screenshots()`. Both shell out to `scripts/generate_screenshots.py`.

Screenshots should be their own concern — generated and committed independently of version bumps.

## Approach

Three changes, all small:

1. **Remove screenshot generation from `_release()`** — delete the `skip_screenshots` parameter from `_release()` and all three release commands (`patch`, `minor`, `major`). Delete the conditional `_generate_screenshots()` call and the dry-run message. Keep the `_generate_screenshots()` helper function since `publish.py screenshots` still uses it.

2. **Keep `publish.py screenshots`** — it already works as a standalone command. No changes needed.

3. **Add `.lf/steps/screenshots.md`** — a step that runs `generate_screenshots.py`, then commits the results with `lf ops commit`. This is the intended entry point going forward. The step is a markdown prompt, not a Rust ops command — consistent with how `ux-review` and other steps work.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `lf ops screenshots` (Rust command) | Direct, no agent overhead | Screenshots don't need Rust — it's a Python script. Steps are the right level for "run a script and commit". Adding Rust code for a subprocess call is over-engineering. |
| Remove `publish.py screenshots` entirely | One fewer path | It's useful for quick manual runs without the commit step. Zero maintenance cost — it's 8 lines calling `_generate_screenshots()`. |
| CI-only screenshot generation | Fully automated | Screenshots need a macOS GUI (Concerto launches, renders, snapshots). CI runners don't have the right environment. |

## Key decisions

**Step, not ops command.** The viz wave README says "Fix the feedback loop." The feedback loop is: generate screenshots, look at them, iterate. A step fits this — it's a prompt that chains with other steps (e.g., `lf screenshots && lf ux-review`). An ops command would be an isolated action with no chaining story.

**No new flags on generate_screenshots.py.** The existing script already has `--output`, `--manifest`, `--snapshot-only`, etc. The step just runs it with defaults.

**Commit in the step prompt, not in the script.** The step tells the agent to commit after generation. This keeps `generate_screenshots.py` as a pure generation tool (consistent with how it works today) and lets the agent write a meaningful commit message based on what changed.

## Scope

- In scope: remove from `_release()`, add step
- Out of scope: screenshot coverage gaps (that's item 02), persona subdivision, new manifest entries

## Implementation

### 1. Edit `scripts/publish.py`

Remove `skip_screenshots` parameter from `_release()` signature and all references:

```python
# Before
def _release(bump_type: str, dry_run: bool, skip_dmg: bool, skip_screenshots: bool) -> None:

# After
def _release(bump_type: str, dry_run: bool, skip_dmg: bool) -> None:
```

Remove the screenshot block inside `_release()` (lines 457-471 and 465-471).

Remove `skip_screenshots` from `patch()`, `minor()`, `major()` command signatures and their `_release()` calls.

### 2. Create `.lf/steps/screenshots.md`

```markdown
---
requires: none
produces: docs/screenshots/*.png
---
Generate fresh Concerto screenshots and commit results.

## Workflow

1. Run the screenshot generator:
   ```bash
   uv run python scripts/generate_screenshots.py
   ```

2. Check what changed:
   ```bash
   git diff --stat docs/screenshots/
   ```

3. If screenshots changed, commit them:
   ```bash
   git add docs/screenshots/
   git commit -m "screenshots: refresh docs/screenshots/"
   ```

4. If nothing changed, report that screenshots are up to date.
```

### 3. Mark roadmap item done

Update `roadmap/viz/README.md` — the item is already struck through in the diff.

## Done when

- `uv run python scripts/publish.py patch --dry-run` output does not mention screenshots
- `lf screenshots` runs `generate_screenshots.py` and commits results
- `uv run python scripts/publish.py screenshots` still works as a manual convenience
