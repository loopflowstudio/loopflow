# lf ops: Full Parity Analysis

Detailed gap analysis between Python and Rust `lf ops` implementations, with a plan to reach 100% feature parity.

## Architecture Simplification

**Current state:** Python calls `lf-engine` binary (JSON wrapper around `loopflow-engine`) for git operations.

**Target state:**
- `loopflow-engine` - Rust library (shared between `lf` and `lfd`)
- `lf` - Rust CLI using `loopflow-engine` directly
- `lfd` - Rust daemon using `loopflow-engine` directly
- Python `loopflow` - uses PyO3 bindings to `loopflow-engine`

**Action:** Delete `lf-engine` binary. Python uses PyO3 or calls `lf` if needed.

---

## Command-by-Command Gap Analysis

### Core Git Workflow Commands

#### `lf ops rebase`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Fetch origin/main first | Yes | Yes | - |
| Rebase onto ref | Yes | Yes | - |
| Conflict detection | Yes | Yes | - |
| **Abort on conflict** | Yes, then launches assistant | No, just fails | **MISSING** |
| **Launch `lf rebase` step on conflict** | Yes | No | **MISSING** |
| Force-push after success | Yes | Yes | - |
| Progress messages | "Fetching...", "Rebasing..." | Minimal | **UX gap** |

**Work needed:**
1. On conflict: abort rebase, list conflicted files
2. Launch `lf rebase` step (agent) to resolve conflicts
3. Wait for agent, verify rebase completed
4. Add progress messages

---

#### `lf ops push`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Basic push | Yes | Yes | - |
| `--force-with-lease` | Yes, automatic fallback | Yes, via flag | - |

**Status:** Mostly complete. Minor UX differences.

---

#### `lf ops commit`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| `-m/--message` | Yes | Yes | - |
| **Auto-stage (`-a/--add`)** | Yes, default true | No | **MISSING** |
| **Lint before commit (`--lint`)** | Yes, default true | No | **MISSING** |
| **LLM-generated message** | Yes, via `lf commit` step | No, message required | **MISSING** |
| **Push after commit (`-p/--push`)** | Yes | No | **MISSING** |
| **Auto-create draft PR** | Yes, after push | No | **MISSING** |
| Progress messages | Yes | Minimal | **UX gap** |

**Work needed:**
1. Add `--add/--no-add` flag (default: add)
2. Add `--lint/--no-lint` flag (default: lint)
3. Add `--push` flag
4. Integrate agent for message generation when no `-m` provided
5. Create draft PR after push if none exists

---

#### `lf ops pr`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Create PR | Yes | Yes, via `gh pr create --fill` | - |
| `--draft` | Yes | Yes | - |
| **Auto-commit pending changes** | Yes | No | **MISSING** |
| **Auto-rebase if behind main** | Yes | No | **MISSING** |
| **LLM-generated title/body** | Yes | No, uses `--fill` | **MISSING** |
| **Detect draft → mark ready** | Yes | No | **MISSING** |
| **`--refresh` to regenerate** | Yes | No | **MISSING** |
| **Stacked base branch detection** | Yes | No | **MISSING** |
| **Open in browser after create** | Yes | No | **MISSING** |
| **Lint before PR (`--lint`)** | Yes | No | **MISSING** |

**Work needed:**
1. Auto-commit workflow (calls `add_commit_push()` equivalent)
2. Check if behind main, auto-rebase
3. LLM message generation via agent
4. Draft detection and `gh pr ready`
5. `--refresh` flag to regenerate
6. Stacked branch: detect parent branch, set as base
7. Open browser: `gh pr view --web`
8. Lint integration

---

#### `lf ops land`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| `--strategy` | Yes | Yes | - |
| **Auto-commit (`--strict` to disable)** | Yes | No | **MISSING** |
| **Auto-rebase onto main** | Yes | No | **MISSING** |
| **Lint before land (`--lint`)** | Yes | No | **MISSING** |
| **Clear `scratch/` before merge** | Yes | No | **MISSING** |
| **Refresh PR with new message** | Yes | No | **MISSING** |
| **Mark draft as ready** | Yes | No | **MISSING** |
| **Enable auto-merge** | Yes, `gh pr merge --auto` | Has `pr_merge_squash_auto` | Needs integration |
| **Wait for merge (`--block`)** | No (next has it) | No | - |
| **Worktree cleanup** | Yes | Partial | **Gap** |
| **`--local` vs `--gh` modes** | Yes | Yes via strategy | - |
| **`-w/--worktree` target** | Yes | No | **MISSING** |
| **`-c/--create-pr` option** | Yes | No | **MISSING** |
| Conflict → agent handoff | Yes | No | **MISSING** |
| Progress messages | Detailed | Minimal | **UX gap** |

**Work needed:**
1. `--strict` flag (disable auto-commit)
2. Auto-rebase workflow with conflict agent handoff
3. `--lint/--no-lint` integration
4. `clear_scratch()` implementation
5. PR refresh before merge
6. Integrate auto-merge enablement
7. `-w/--worktree` targeting
8. `-c/--create-pr` shortcut

---

#### `lf ops next`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Preserve current worktree | Yes | Yes | - |
| Create new branch | Yes | Yes | - |
| Shell directive for cd | Yes | Yes | - |
| **Auto-commit pending changes** | Yes | No | **MISSING** |
| **Auto-rebase (`--no-rebase` to skip)** | Yes | No | **MISSING** |
| **Enable auto-merge on current PR** | Yes | No | **MISSING** |
| **`--block` to wait for merge** | Yes | No | **MISSING** |
| **`--create-pr` option** | Yes | No | **MISSING** |
| **Stack vs fresh start logic** | Yes (PR open → stack, merged → fresh) | Always fresh? | **Verify** |
| **Update wave metadata** | Yes | No | **MISSING** |
| **Open terminal in new worktree** | Yes | No | **MISSING** |

**Work needed:**
1. Auto-commit workflow
2. `--rebase/--no-rebase` flags
3. Enable auto-merge on current PR before creating new
4. `--block` with merge polling loop
5. `--create-pr` option
6. Stacking logic: if PR open, stack on HEAD; if merged, fresh from main
7. Wave metadata update (when wave module exists)
8. Terminal opening (configurable IDE/terminal)

---

#### `lf ops abandon`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Delete local branch | Yes | Yes | - |
| `--force` | Yes | Yes | - |
| **Find worktree by branch** | Yes | No, operates on current | **MISSING** |
| **Check uncommitted changes** | Yes | No | **MISSING** |
| **Confirmation prompt** | Yes | No | **MISSING** |
| **Close PR** | Yes, `gh pr close` | No | **MISSING** |
| **Delete remote branch** | Yes | No | **MISSING** |
| **Remove worktree** | Yes | No | **MISSING** |

**Work needed:**
1. Accept branch name argument (find worktree)
2. Dirty check before abandon
3. Confirmation unless `--force`
4. `gh pr close` integration
5. `git push origin --delete`
6. `worktree_remove()` call

---

#### `lf ops sync`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Fetch origin/main | Yes | Yes | - |
| Reset local main | Yes | Yes | - |
| Dirty check on main | Yes | Yes | - |

**Status:** Complete.

---

### Worktree Commands

#### `lf ops wt create`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Schema-based naming | Yes | Yes | - |
| `--base` branch | Yes | Yes | - |
| `--stack` on current | Yes | Yes | - |
| Shell directive for cd | Yes | Yes | - |
| Fallback cd message | Yes | Yes | - |

**Status:** Complete.

---

#### `lf ops wt switch`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Match by short name | Yes | Yes | - |
| Shell directive for cd | Yes | Yes | - |
| Multiple match error | Yes | Yes | - |

**Status:** Complete.

---

#### `lf ops wt list`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| `--format json` | Yes | Yes | - |
| `--sync` before list | Yes | Flag accepted, **not implemented** | **Gap** |
| `--full` details | Yes | Flag accepted, **not implemented** | **Gap** |
| Prunable annotation | Yes | Yes | - |

**Work needed:**
1. Implement `--sync` (call `sync_main()` before listing)
2. Implement `--full` (include commit info, PR status, etc.)

---

#### `lf ops wt prune`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Merge detection | Yes | Yes | - |
| `--dry-run` | Yes | Yes | - |
| `--force` skip confirm | Yes | Yes | - |
| `--debug` diagnostics | Yes | Yes | - |
| Exclude current worktree | Yes | Yes | - |
| Sync before prune | Yes | ? | **Verify** |
| **Confirmation prompt** | Yes | No? | **Verify** |

**Status:** Mostly complete. Verify confirmation UX.

---

#### `lf ops wt ci`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Show CI status | Yes | Yes | - |
| `--watch` | Yes | Yes | - |
| `--logs` for failures | Yes | Yes | - |

**Status:** Complete.

---

### Shell Integration

#### `lf ops shell init`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| zsh support | Yes | Yes | - |
| bash support | Yes | Yes | - |
| **fish support** | Yes | No | **Deferred** |

---

#### `lf ops shell install`

| Feature | Python | Rust | Gap |
|---------|--------|------|-----|
| Auto-detect shell | Yes | Yes | - |
| Append to config | Yes | Yes | - |
| `-y/--yes` skip confirm | Yes | ? | **Verify** |
| Already installed check | Yes | ? | **Verify** |

---

### Missing Commands

| Command | Python Behavior | Priority |
|---------|-----------------|----------|
| **`lf ops add`** | Create `.claude/commands/<name>.md` template | Low |
| **`lf ops cp`** | Copy context to clipboard for web clients | Low |
| **`lf ops doctor`** | Check dependencies (wt, gh, claude, codex, etc.) | Medium |
| **`lf ops version`** | Show loopflow version | Low |
| **`lf ops summarize`** | LLM-generated codebase summaries with caching | Low |

---

## Cross-Cutting Concerns

### 1. Auto-Commit Workflow

Python has a shared `add_commit_push()` pattern used by:
- `pr` (before creating/updating PR)
- `land` (unless `--strict`)
- `next` (before iterating)

**Implementation:**
```rust
fn add_commit_push(repo: &Path) -> Result<()> {
    stage_all(repo)?;
    // Generate message via agent or use default
    let msg = generate_commit_message(repo)?;
    commit(repo, &msg)?;
    push(repo)?;
    ensure_draft_pr(repo)?;
    Ok(())
}
```

---

### 2. Lint Integration

Python commands support `--lint/--no-lint` with flow:
1. Run `config.lint_check` command if set
2. Fallback: try `ruff check` + `ruff format --check`
3. On failure: run `lf lint` step (agent fixer)
4. On pass: continue

**Commands needing lint:** `commit`, `pr`, `land`

---

### 3. Agent Integration

Rust `lf ops` needs to invoke agents for:
- Commit message generation (no `-m` provided)
- PR title/body generation
- Rebase conflict resolution
- Lint fixing

**Pattern:** Use `loopflow-engine::agent::launch_agent()` with appropriate step.

---

### 4. Progress Messages & UX

Python has polished UX with:
- Progress: "Fetching origin/main...", "Rebasing...", "Pushing..."
- Status: "No new commits. Opening existing PR..."
- Errors: Detailed with suggestions
- Confirmations: "Abandon branch 'X'? [y/N]:"

Rust commands are terse. Add consistent messaging.

---

### 5. Wave Integration

Python tracks:
- Current wave from worktree
- Branch → wave mapping
- Wave updates on `next`

**Decision:** Defer until wave module ported. Add hooks for later.

---

### 6. Scratch Clearing

Before merge, Python calls `clear_scratch()`:
1. Delete marked files in `scratch/`
2. Commit the deletion
3. Push before enabling auto-merge

Prevents draft content from merging to main.

---

## Implementation Phases

### Phase 1: Core Workflow Polish (High Impact)

1. **`commit` enhancements**
   - `--add/--no-add` (stage all)
   - `--push` (push + draft PR)
   - Agent message generation when no `-m`

2. **`pr` enhancements**
   - Auto-commit before PR
   - Auto-rebase if behind
   - `--refresh` regeneration
   - Mark draft as ready
   - Open in browser

3. **`land` enhancements**
   - Auto-commit unless `--strict`
   - Auto-rebase with conflict agent
   - Clear scratch/
   - Enable auto-merge

4. **`abandon` full workflow**
   - Branch argument (find worktree)
   - Close PR
   - Delete remote
   - Remove worktree
   - Confirmation prompt

---

### Phase 2: Advanced Workflows

1. **`rebase` conflict handling**
   - Launch `lf rebase` step on conflict
   - Wait for resolution

2. **`next` full workflow**
   - `--block` merge waiting
   - `--create-pr` option
   - Auto-merge current PR
   - Stack vs fresh logic

3. **Lint integration**
   - Shared lint check function
   - Agent fixer on failure
   - `--lint/--no-lint` flags

---

### Phase 3: UX & Completeness

1. **Progress messages**
   - Consistent status output
   - Clear error messages

2. **`wt list` full flags**
   - `--sync` implementation
   - `--full` details

3. **Missing commands**
   - `doctor` (dependency check)
   - `version`
   - `add` (prompt template)

---

### Phase 4: Architecture Cleanup

1. **Delete `lf-engine` binary**
   - Ensure PyO3 bindings cover all operations
   - Update Python to use bindings

2. **Fish shell support**
   - Add fish init script

---

## Parity Checklist

### Commands
- [ ] `rebase` - conflict → agent handoff
- [ ] `push` - complete (verify UX)
- [ ] `commit` - auto-stage, lint, agent message, push+PR
- [ ] `pr` - auto-commit, rebase, refresh, draft→ready, browser
- [ ] `land` - auto-commit, rebase, lint, scratch, auto-merge
- [ ] `next` - block, create-pr, auto-merge, stack logic
- [ ] `abandon` - full workflow with PR/remote cleanup
- [x] `sync` - complete
- [x] `wt create` - complete
- [x] `wt switch` - complete
- [ ] `wt list` - implement --sync, --full
- [x] `wt prune` - complete (verify confirm)
- [x] `wt ci` - complete
- [x] `shell init` - complete (fish deferred)
- [x] `shell install` - complete

### Cross-Cutting
- [ ] Auto-commit workflow
- [ ] Lint integration
- [ ] Agent integration for messages
- [ ] Progress messages
- [ ] Confirmation prompts
- [ ] Scratch clearing
- [ ] Browser opening

### Architecture
- [ ] Delete `lf-engine` binary
- [ ] Verify PyO3 bindings complete
