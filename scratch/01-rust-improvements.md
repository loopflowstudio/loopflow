# Rust Improvements: dirty + remote_gone in WorktreeState

## Problem

`wt list` and `wt prune` check dirtiness ad-hoc via separate `is_clean()` calls, so the JSON output and struct consumers can't see it. Remote branch deletion — a strong signal that work is done — isn't detected at all. Worktrees whose branches were merged but have leftover uncommitted changes look like regular `merged` entries, hiding a state that needs human action.

## Approach

Add `dirty: bool` and `remote_gone: bool` to `WorktreeState`. Populate both in `list_worktrees()` so all consumers (list, prune, JSON, agent step) get the data for free.

### WorktreeState changes

```rust
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
    pub dirty: bool,
    pub remote_gone: bool,
}
```

### Populating `dirty`

Move the `is_clean()` call into `list_worktrees()`, per-worktree in the build loop (lines 284-316). It's a local `git status --porcelain` — fast, no network. Store `!is_clean()` as `dirty`.

```rust
let dirty = !is_clean(&path).unwrap_or(true);
```

### Populating `remote_gone`

Single `git ls-remote --heads origin` call for all branches. One network round-trip, returns all remote branches. Parse output and check membership.

New function in `worktrees.rs`:

```rust
fn list_remote_branches(repo: &Path) -> HashSet<String> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .args(["ls-remote", "--heads", "origin"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    line.split('\t')
                        .nth(1)?
                        .strip_prefix("refs/heads/")
                        .map(|b| b.to_string())
                })
                .collect()
        }
        _ => HashSet::new(), // offline or no remote — skip gracefully
    }
}
```

Call once in `list_worktrees()`, before the build loop. Run it in parallel with the squash-merge and PR checks (all three are independent). For each branch: `remote_gone = !remote_branches.contains(branch)`. Default branch is never remote-gone.

### Prunable logic

```rust
let prunable = !is_default
    && (merged || !has_commits || (remote_gone && !dirty));
```

New case: `remote_gone && !dirty` — the remote branch was deliberately deleted, no uncommitted changes to lose. Committed work survives in reflog for 90 days. This catches abandoned branches and branches whose merge detection failed but whose remote was cleaned up.

### wt_list display

Replace the current status logic (lines 439-451) with:

```
merged && dirty    → red "landed-dirty"     (replaces "merged" + "dirty")
merged && !dirty   → green "merged"         (unchanged)
remote_gone && !merged → yellow "remote-gone"
fresh              → dim "fresh"            (unchanged)
else               → cyan "active"          (unchanged)
non-merged dirty   → yellow "dirty" suffix  (unchanged)
```

Priority: `landed-dirty` > `remote-gone` > `merged` > `fresh` > `active`. A worktree that is both `remote_gone` and `merged` shows as `merged` (the merge status is more informative).

Remove the separate `dirty_flag` for merged worktrees — `landed-dirty` subsumes it.

### wt_prune display

Group dry-run output by reason instead of flat list:

```
Merged:
  feature-x  /path/to/repo.feature-x
Remote-gone:
  abandoned-y  /path/to/repo.abandoned-y
Empty:
  test-z  /path/to/repo.test-z
```

Compute reason from struct fields: `merged` → "Merged", `remote_gone` → "Remote-gone", else → "Empty".

### wt_prune consumers

`wt_prune()` currently filters `is_clean(&wt.path)` after `list_worktrees()`. Replace with `!wt.dirty` — the data is now in the struct.

```rust
let mut prunable = worktrees
    .into_iter()
    .filter(|wt| wt.prunable)
    .filter(|wt| wt.path != current_path)
    .filter(|wt| !wt.dirty)  // was: is_clean(&wt.path)
    .collect::<Vec<_>>();
```

### Parallelism in list_worktrees

Current: two parallel thread groups (squash-merge checks + PR GraphQL call).

New: three parallel groups:
1. Squash-merge checks (per-branch threads) — existing
2. PR merged check (single GraphQL call) — existing
3. Remote branch listing (single `ls-remote` call) — new
4. Dirty checks happen in the build loop (local, fast, not worth threading)

```rust
// Spawn alongside existing checks
let repo_for_remote = repo.to_path_buf();
let remote_handle = thread::spawn(move || list_remote_branches(&repo_for_remote));

// ... existing squash_handle and pr_handle ...

let remote_branches = remote_handle.join().unwrap_or_default();
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-branch `ls-remote` | Simple but N network calls for N branches | Single call is strictly better — same data, one round-trip |
| `dirty` as opt-in (not in struct) | Avoids running `git status` for every worktree on every `list_worktrees()` call | Both consumers need it, and it's local/fast. Centralizing eliminates duplicate calls and gives JSON consumers the data |
| `WorktreeStatus` enum instead of bools | Cleaner type, but compound states need multiple variants | Bools compose better — consumers compute display from raw signals. Enum would need `MergedDirty`, `RemoteGone`, `RemoteGoneDirty`, etc. |
| `prune_reason` field on struct | Explicit grouping for prune display | Reason is derivable from existing bools — adding a field duplicates state. Compute it where needed |

## Key decisions

**`remote_gone && !dirty` is prunable.** This is aggressive — it prunes branches with unpushed commits if the remote was deleted and the worktree is clean. Rationale: if someone deleted the remote branch, the work was either merged (detection missed it) or intentionally abandoned. Committed work survives in reflog. The agent step can investigate before the Rust code acts.

**Single `ls-remote` over per-branch calls.** One network round-trip for all branches. Offline returns empty set → all `remote_gone = false` → no false positives.

**`dirty` always computed in `list_worktrees()`.** It's a local git command (fast), both `wt_list` and `wt_prune` need it, and JSON output should include it. No reason to defer.

**`landed-dirty` is display-only, not a field.** Computed from `merged && dirty` in the display layer. The struct stores raw bools — simpler for JSON consumers and agent steps that want to apply their own logic.

## Scope

- In scope: `WorktreeState` struct, `list_worktrees()`, `wt_list()` display, `wt_prune()` filtering/display, JSON output
- Out of scope: prune step (`.lf/steps/prune.md`) — that's sprint 02. `wt_remove()` — no changes needed.

## Done when

1. `cargo test --all` passes
2. `cargo clippy -- -D warnings` passes
3. `lf ops wt list` shows `landed-dirty` (red) and `remote-gone` (yellow) states
4. `lf ops wt list --format json` includes `dirty` and `remote_gone` fields
5. `lf ops wt prune` groups output by reason (merged/remote-gone/empty)
6. `lf ops wt prune --force` removes remote-gone clean worktrees
