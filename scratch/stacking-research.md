# Research: lfd stacking commands (next, rebase)

## System understanding

The lfd stacking commands solve the problem of managing dependent PRs (stacking) when using squash-merge workflows. When a base PR is squash-merged to main, the original commits disappear from git history, breaking naive rebase attempts. The `next` and `rebase` commands work together by recording state at branch time and using it for squash-aware rebasing.

### Architecture

The stacking feature spans three layers:

| Layer | Component | Responsibility |
|-------|-----------|----------------|
| **Model** | `Wave` in `models.py:109-111` | `base_branch`, `base_commit` fields |
| **Persistence** | `wave.py:272-288` | `update_wave_stacking()` for atomic updates |
| **CLI** | `cli.py:1375-1606` | `lfd next` and `lfd rebase` commands |
| **Schema** | `m_2026_01_28_wave_stacking.py` | Database migration adding columns |

The commands are wave-centric: they resolve the current wave from the worktree path via `get_wave_by_worktree()` or accept an explicit name. This integrates with lfd's wave model rather than being standalone git tooling.

### Data flow

**`lfd next` flow:**
```
1. Resolve wave from worktree/name
2. Get current branch (old_branch) and HEAD SHA (old_head)
3. Get or create PR → enable auto-merge via gh CLI
4. Generate new branch: {wave.name}.{timestamp}.{word-pair}
5. git checkout -b {new_branch}
6. git push -u origin {new_branch}
7. Update wave.branch = new_branch
8. Update wave.base_branch = old_branch, wave.base_commit = old_head
```

**`lfd rebase` flow:**
```
1. Resolve wave from worktree/name
2. git fetch origin
3. If no base_branch → simple rebase onto origin/main
4. Check base PR state via gh CLI
5. If base PR OPEN → error (wait for merge)
6. If base_commit exists → git rebase --onto origin/main {base_commit}
7. Otherwise → git rebase origin/main
8. git push --force-with-lease
9. Clear wave.base_branch and wave.base_commit
```

### Key abstractions

**Wave state for stacking:**
- `base_branch`: The branch this wave was stacked on (e.g., `jack.auth.20260127_2204`)
- `base_commit`: The exact SHA when branching occurred—critical for squash-aware rebase

**Branch naming convention:**
- Pattern: `{wave_name}.{YYYYMMDD_HHMM}.{magical-musical}` (e.g., `jack.auth.20260128_1206.ripple-cadence`)
- `naming.py` provides `generate_next_branch()` and `parse_branch_base()`

**Wave resolution:**
- `_resolve_wave_from_worktree_or_name()` at `cli.py:1325` provides unified lookup
- Falls back from explicit name → worktree path → error

## Tensions

**lfops vs lfd parallel implementations:**
- `lfops/next.py` has its own implementation without wave state (creates new worktrees)
- `lfd next` reuses the same worktree, switches branches, and records stacking state
- Divergent behavior: lfops creates sibling worktrees, lfd switches branches in place

**GitHub CLI dependency:**
- Both commands shell to `gh` for PR operations
- No fallback if `gh` isn't configured—auto-merge silently fails with warnings

## Observations

### Complexity

**Branch naming generation (`naming.py:147-161`):** Loops up to 100 times to find a unique branch name. Could theoretically loop indefinitely if many branches exist with same timestamp.

**Wave resolution helper (`cli.py:1325-1339`):** Clean pattern for inferring wave from context. Used consistently in `next_cmd` and `rebase_cmd`.

**Squash detection logic (`cli.py:1552-1561`):** Uses `_get_pr_state()` to check if base PR is MERGED. The check is by branch name, not PR number—matches how GitHub tracks PRs.

### Quality

**Test coverage:** `tests/test_next.py` covers the lfops implementation but not the lfd stacking commands. The lfd `next_cmd` and `rebase_cmd` have no dedicated tests.

**Error messages:** Specific and actionable. "No open PR found. Run 'lfops pr' first, or use --create-pr." includes both the problem and the fix.

**Warning vs error handling:** Auto-merge failures emit warnings but don't fail the command—allows stacking to continue even if auto-merge isn't enabled.

### Potential

**lf-core git module (`git.rs`):** Currently minimal (status/diff only). The ops architecture doc (`roadmap/wave/ops-architecture.md`) envisions full git operations here: `rebase()`, `create_stacked_branch()`, `push_force_with_lease()`, `land()`.

**Multi-level stacking:** Current design tracks one base branch per wave. The data model could support chained stacking with minimal changes (just track base recursively).

**Automatic rebase on land:** With webhook integration, lfd could detect when base PR lands and auto-trigger rebase.

## Open questions

- Should `lfd next` and `lfops next` be unified or kept deliberately separate?
- Why does `lfops next` create new worktrees while `lfd next` reuses the same worktree?
- No tests for lfd stacking commands—is this intentional or an oversight?

## Recommendations

### Add lfd stacking command tests
**Observation**: `lfd next` and `lfd rebase` commands have no dedicated test coverage. `tests/test_next.py` only tests `lfops/next.py`.

**Cost**: Medium—requires mocking wave DB, subprocess calls, and git operations.

**Benefit**: Confidence in squash-aware rebase logic, especially edge cases (missing base_commit, base PR states).

**Verdict**: Worth it. The squash-aware rebase is the core value proposition and should have test coverage for the critical path.

### Document the lfops vs lfd next distinction
**Observation**: Two implementations of `next` exist with different semantics. `lfops next` creates new worktrees; `lfd next` switches branches in place and records wave state.

**Cost**: Low—documentation only.

**Benefit**: Users understand which to use when.

**Verdict**: Worth it. Add a note in the docs explaining the distinction and when to use each.

### Consider unifying or clarifying next implementations
**Observation**: The parallel implementations could cause confusion. lfops is stateless and standalone; lfd requires wave state.

**Cost**: Medium—would need to decide on semantics.

**Benefit**: Simpler mental model for users.

**Verdict**: Defer. The current split aligns with the ops architecture doc's philosophy that "lf ops and lfd are siblings, not layers." Document rather than unify.
