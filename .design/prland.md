# prland

Fix `lfpr land` to use `gh pr merge` so GitHub shows PRs as merged (not closed). Add `--local` mode for landing without a PR.

## Review

**Verdict:** Needs work

**Config field naming mismatch.** The design doc specifies `pr: gh | local` but the implementation uses `land: gh | local`. The code in `config.py:48` adds `land: str = "gh"` and `lfpr.py:255` checks `config.land == "local"`. Either update the design doc or rename the config field to `pr`. Recommend keeping `land` since it's more descriptive of what it controls.

**Deleted files not committed.** Git status shows 8 files deleted but unstaged:
- `src/loopflow/cli/commit.py`
- `src/loopflow/cli/compare.py`
- `src/loopflow/cli/land.py`
- `src/loopflow/cli/meta.py`
- `src/loopflow/cli/ops.py`
- `src/loopflow/cli/pr.py`
- `src/loopflow/cli/sessions.py`
- `src/loopflow/cli/status.py`

These need to be staged and committed. The deletions are correct per the design (consolidating into `lfpr.py`), but they're currently in an intermediate state.

**Test file has dead test.** `tests/test_commit.py:67-70` has `test_commit_with_custom_message_skips_generation` that just does `pass` with a comment "this test is now obsolete". Delete the test entirely rather than leaving a stub.

## Design notes

The implementation correctly:
- Uses `gh pr merge --squash --delete-branch` for PR mode, which marks PRs as merged on GitHub
- Removes worktrunk dependency from `--local` mode
- Simplifies the `land` command signature (removed `--force`, `--no-pr`, `--require-clean-design`, `--base`)
- Clears `.design` artifacts in both modes
- Syncs main repo after merge in both modes

Open question from `.design/questions.md`: Auto-rebase on merge conflict is a follow-up feature, not part of this PR.
