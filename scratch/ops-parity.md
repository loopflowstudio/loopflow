# Ops Parity Testing

Mock-based logic comparison between Python and Rust `lf ops` commands.

## Goal

Verify Rust ops workflows make the same decisions as Python, without actually executing side effects.

## Approach

Capture operation traces and compare them.

```
Python lf ops commit --push:
  1. git:status_porcelain -> "M file.txt"
  2. git:add_all
  3. lint:run -> pass
  4. git:diff_cached_quiet -> has_changes
  5. agent:run(prompt="generate commit...", model="claude:opus")
  6. git:commit(message="lf test: add feature")
  7. git:push
  8. gh:pr_create_draft

Rust lf ops commit --push:
  [should produce identical trace]
```

## What to trace

| Category | Operations |
|----------|------------|
| git | status, add, diff, commit, push, push_with_upstream, rebase, branch, checkout |
| agent | run(prompt, model, auto, cwd) |
| gh | pr_create, pr_view, pr_merge, pr_ready |
| lint | run(checker) -> pass/fail |
| fs | read_config, find_step |

## Implementation options

### Option A: Env var triggers JSON trace mode

```bash
LF_TRACE=1 lf ops commit --push > trace.json
```

Both Python and Rust emit JSON traces instead of executing. Test compares traces.

Pros: Simple, no mocking infrastructure
Cons: Need to implement in both languages, may miss internal logic

### Option B: Trait-based mocking (Rust) + patch-based mocking (Python)

Rust:
```rust
trait GitOps {
    fn stage_all(&self, repo: &Path) -> Result<()>;
    fn commit(&self, repo: &Path, message: &str) -> Result<()>;
    // ...
}

struct RealGitOps;
struct TracingGitOps { trace: Vec<Op> }
```

Python:
```python
@patch('loopflow.lf.ops.git.subprocess.run')
def test_commit_trace(mock_run):
    mock_run.side_effect = trace_recorder
    # ...
```

Pros: Can test internal logic paths
Cons: More complex, trait boundaries may not match

### Option C: Subprocess capture

Wrap git/gh in a script that logs invocations:

```bash
# git-wrapper.sh
echo "$@" >> /tmp/git-trace.log
exec /usr/bin/git "$@"

PATH=/wrappers:$PATH lf ops commit
```

Pros: Works without code changes
Cons: Doesn't capture agent prompts, messy

## Recommended: Option A

Keep it simple. Add `--trace` flag (or `LF_TRACE` env var) that:
1. Skips actual execution
2. Emits JSON trace of what would happen
3. Uses deterministic mock responses where needed

### Trace format

```json
{
  "command": "commit",
  "options": {"add": true, "lint": true, "push": true},
  "operations": [
    {"op": "git:status", "result": "dirty"},
    {"op": "git:add_all"},
    {"op": "lint:run", "result": "pass"},
    {"op": "git:diff_cached", "result": "has_changes"},
    {"op": "agent:prompt", "prompt_hash": "abc123", "model": "claude:opus"},
    {"op": "git:commit", "message": "lf test: add feature"},
    {"op": "git:push"},
    {"op": "gh:pr_create_draft"}
  ]
}
```

### Normalization

Before comparing traces:
- Remove timestamps
- Normalize paths to relative
- Hash long prompts (compare hashes, not full text)
- Sort unordered operations (if any)

## Test cases

| Command | Scenario | Key operations to verify |
|---------|----------|--------------------------|
| commit | Clean repo | Early exit, no operations |
| commit | Dirty, no staged | stage_all, commit |
| commit | Lint fails | lint:run -> fail, no commit |
| commit --push | With upstream | push |
| commit --push | No upstream | push_with_upstream |
| pr | No existing PR | pr_create |
| pr | Existing PR | pr_view only |
| land | Fast-forward | merge, branch_delete |
| land | Needs rebase | rebase, merge, branch_delete |
| land --local | Local merge | merge (no gh) |
| rebase | Clean | rebase |
| rebase | Conflicts | rebase -> conflict, agent:assist |
| next | Fresh | branch_create, worktree_add |
| abandon | With PR | pr_close, branch_delete, worktree_remove |

## Done when

- [ ] `--trace` mode implemented in Python ops
- [ ] `--trace` mode implemented in Rust ops
- [ ] Trace format documented and stable
- [ ] `test_ops_parity.py` compares traces for all commands
- [ ] All scenarios in test cases table pass

## Open questions

- Should we trace agent prompts fully or just hash them?
- How to handle agent responses in trace mode? (mock responses needed)
- Should trace include timing/ordering or just operations?
