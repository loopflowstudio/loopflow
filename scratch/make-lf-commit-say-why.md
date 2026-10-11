# Commit failure diagnostics

Jack Heart requested this repair on 2026-10-11; delivery is publication for
review, without landing. Implementation plan derived from the supplied brief.

Selected-path commits must save untracked scratch in ordinary Task checkouts,
preserve unrelated staged bytes, and retain persistent workspaces' local scratch
policy. Git failures must retain stderr or stdout and identify silent failures
by exit status. Reproduce only in disposable repositories.

Delete — do not maintain (removed; no remaining targets):
- The selected-path runner's stderr-only error conversion; use the shared Git runner.
- Unconditional scratch exclusion from ordinary selected-path staging.
- `has_staged_changes` and `staged_diff` in `ops/commit.rs`; use the shared
  Git runner so failed reads cannot masquerade as changes or empty diffs.
- Duplicate test Git runners; one path-based helper preserves exact file bytes.
- `commit_with_message_override`; `commit_stages_and_commits` already proves
  that an explicit message is retained.

The disposable installed CLI reproduced exit 1 with `Error:` alone; the initial
regression failed with `Message("")`. The branch CLI now saves both selected
scratch files and explains an unchanged selection. No LOO-462 files were touched.

Review removed duplicate Git command/error paths. Selected commits now share
ordinary commits' identity handling and diagnostics while retaining their separate
index. Persistent scratch policy and unrelated staged bytes remain covered.

Remaining: gate's broader affected-suite verification and the caller's publication
step. No landing is authorized.

Checks: `cargo test -p loopflow --test commit_tests -- --skip commit_generates_message_when_none` passed (7); `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` passed; prior disposable branch CLI passed. Agent invocation is excluded from this focused suite; broader suites belong to gate.
