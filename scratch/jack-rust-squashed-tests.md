# Test Coverage: Rust Migration Gaps

The Python codebase had ~10,700 lines of tests. The Rust codebase has ~1,300. Core engine tests (config, context, prompt, flow, git primitives) are well-covered. Everything else is not.

## Priority 1: Ops Workflows (loopflow-ops)

**Gap**: 2 tests exist (message parsing). Python had ~566 lines covering commit, land, next, pr, rebase.

**Why it matters**: Ops do git mutations. A bug in `land` can delete branches. A bug in `commit` can push broken code. These are the highest-consequence code paths with the least coverage.

**Approach**: Integration tests with temp git repos. Each op gets a test file.

### Tests to write

**rust/loopflow-ops/tests/commit_tests.rs**
- `commit_stages_and_commits` — creates file, runs commit workflow, verifies commit in log
- `commit_with_push` — commits and pushes to bare remote, verifies remote has commit
- `commit_skips_empty` — no changes = no commit, clean exit
- `commit_with_lint_failure` — lint check fails, commit aborted
- `commit_with_message_override` — explicit message used instead of LLM generation

**rust/loopflow-ops/tests/land_tests.rs**
- `land_local_squash_merges_to_main` — feature branch squash-merged, branch deleted
- `land_preserves_main_on_failure` — if merge conflicts, main untouched
- `land_cleans_up_remote_branch` — after land, remote branch deleted
- `land_with_lint_gate` — lint failure blocks land

**rust/loopflow-ops/tests/next_tests.rs**
- `next_creates_branch_from_current` — new branch created, old branch intact
- `next_with_naming_schema` — branch name follows configured schema
- `next_detects_merged_pr_starts_fresh` — merged branch triggers fresh start from main

**rust/loopflow-ops/tests/pr_tests.rs**
- `pr_create_calls_gh` — verifies gh CLI invocation (mock or trace)
- `pr_update_refreshes_body` — existing PR gets updated description
- `pr_skips_when_no_diff` — no changes = no PR

**rust/loopflow-ops/tests/rebase_tests.rs**
- `rebase_onto_main_succeeds` — clean rebase applies
- `rebase_conflict_returns_error` — conflicting changes return conflict list
- `rebase_with_push` — successful rebase pushes with force-with-lease

### Test infrastructure needed

A `test_repo` helper that creates a bare remote + clone with initial commit. Something like:

```rust
struct TestRepo {
    bare: TempDir,    // bare remote
    work: TempDir,    // working clone
    repo: PathBuf,    // path to working clone
}

impl TestRepo {
    fn new() -> Self { /* git init --bare, clone, initial commit */ }
    fn create_file(&self, name: &str, content: &str) { /* write + stage */ }
    fn commit(&self, msg: &str) { /* git commit */ }
    fn create_branch(&self, name: &str) { /* git checkout -b */ }
}
```

This pattern already exists ad-hoc in `loopflow-engine/tests/git_tests.rs` — extract and share it.

---

## Priority 2: Naming (loopflow-engine)

**Gap**: 3 tests exist. Python had 344 lines covering schema formatting, word pairs, timestamp injection, sanitization edge cases.

**Why it matters**: Branch names are user-visible and must be git-legal. Bad names break worktree operations.

**Approach**: Unit tests in `naming.rs`. Pure functions, no I/O.

### Tests to write (inline in naming.rs)

- `format_branch_name_with_schema` — `{user}/{words}` produces `jack/cosmic-piano`
- `format_branch_name_with_timestamp` — `{ts}` injects unix timestamp
- `format_branch_name_with_date` — `{date}` injects YYYY-MM-DD
- `format_branch_name_with_name` — `{name}` injects sanitized step name
- `sanitize_removes_special_chars` — `feat/my thing!` → `feat/my-thing`
- `sanitize_collapses_hyphens` — `a---b` → `a-b`
- `sanitize_trims_leading_trailing` — `-foo-` → `foo`
- `word_pairs_are_two_words` — output matches `{adj}-{noun}` pattern
- `word_pairs_vary_with_different_seeds` — different RNG seeds produce different pairs

---

## Priority 3: Discovery & Skills (lf crate)

**Gap**: 0 tests. Python had 467 lines covering skill discovery, name normalization, registry lookup.

**Why it matters**: `lf --list` and skill resolution are user-facing. If discovery breaks, users see empty lists or can't run steps.

**Approach**: Integration tests with temp repos containing `.lf/steps/` and `.lf/flows/`.

### Tests to write

**rust/lf/tests/discovery_tests.rs**
- `discover_builtin_steps` — all builtin steps found without any repo config
- `discover_repo_steps` — steps in `.lf/steps/` appear in listing
- `discover_repo_flows` — flows in `.lf/flows/` appear in listing
- `repo_step_shadows_builtin` — repo step with same name as builtin wins
- `discover_directions` — builtin + repo directions listed
- `categorized_listing` — steps grouped into code/plan/interactive/ops categories

**rust/lf/tests/skill_tests.rs** (if/when skills are ported)
- `discover_superpowers_skill` — `sp:*` namespace resolved
- `normalize_skill_name` — `sp:foo-bar` → canonical form
- `skill_not_found_error` — missing skill gives helpful message

---

## Priority 4: Agent Launching (loopflow-engine)

**Gap**: 11 tests for command *building*, 0 for *launching*. Python had 585 lines.

**Why it matters**: Launch failures are silent. Wrong flags, bad env vars, missing binaries — users get cryptic errors.

**Approach**: Test the launch pipeline without actually spawning agents. Mock the subprocess call or test with a simple echo binary.

### Tests to write (in agent.rs or tests/agent_tests.rs)

- `launch_returns_exit_code` — launch `echo hello`, verify exit 0
- `launch_captures_stdout` — launch `echo hello`, verify stdout contains "hello"
- `launch_captures_stderr` — launch command that writes to stderr
- `launch_nonzero_exit` — launch `false`, verify non-zero exit
- `launch_missing_binary_returns_error` — launch nonexistent binary, get clean error
- `launch_with_cwd` — working directory respected
- `launch_streaming_mode` — streaming output arrives line by line

---

## Priority 5: Worktree Operations

**Gap**: No dedicated tests. git_tests.rs has 2 worktree assertions. Python tested creation, listing, pruning, state inspection.

**Approach**: Add to existing git_tests.rs or create worktree_tests.rs.

### Tests to write

- `worktree_add_creates_directory` — new worktree exists on disk
- `worktree_add_is_on_correct_branch` — worktree branch matches request
- `worktree_remove_deletes_directory` — removed worktree no longer on disk
- `worktree_list_includes_created` — newly created worktree appears in list
- `worktree_state_detects_dirty` — uncommitted changes in worktree detected
- `worktree_move_preserves_content` — moved worktree has same files

---

## Priority 6: Daemon (lfd)

**Gap**: 4 tests (store suite + scheduler). Python had 3,842 lines.

**Defer for now.** The daemon is being redesigned (HTTP + WebSocket replacing gRPC). Writing extensive tests for code that's still in flux wastes effort. Revisit after the daemon API stabilizes.

Minimal tests worth adding now:
- `wave_crud_through_http` — create/read/update/delete wave via HTTP endpoints
- `wave_run_execution_completes` — trigger wave, verify completion status
- `websocket_receives_events` — connect WS, trigger action, receive event

---

## Execution Order

1. **Extract TestRepo helper** from git_tests.rs into a shared test utility
2. **Ops workflow tests** — highest risk, most impact
3. **Naming tests** — easy wins, pure functions
4. **Discovery tests** — user-facing, catches regressions
5. **Agent launch tests** — integration-level
6. **Worktree tests** — extend existing git test infrastructure
7. **Daemon tests** — defer until API stabilizes

Estimated effort: ~800-1000 lines of test code for priorities 1-5. This won't match the 10,700 Python lines, but the Python tests included substantial mock wiring and CLI runner boilerplate that Rust doesn't need.
