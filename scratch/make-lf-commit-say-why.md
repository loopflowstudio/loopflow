# Commit failure diagnostics

Jack Heart requested this repair on 2026-10-11; delivery is publication for
review, without landing. Implementation plan derived from the supplied brief.

Selected-path commits must save untracked scratch in ordinary Task checkouts,
preserve unrelated staged bytes, and retain persistent workspaces' local scratch
policy. Git failures must retain stderr or stdout and identify silent failures
by exit status. Reproduce only in disposable repositories.

Delete — do not maintain:
- The selected-path runner's stderr-only error conversion; use the shared Git runner.
- Unconditional scratch exclusion from ordinary selected-path staging.

The disposable installed CLI reproduced exit 1 with `Error:` alone; the initial
regression failed with `Message("")`. The branch CLI now saves both selected
scratch files and explains an unchanged selection. No LOO-462 files were touched.

Review removed the duplicate Git command/error path. Selected commits now share
ordinary commits' identity handling and diagnostics while retaining their separate
index. Persistent scratch policy and unrelated staged bytes remain covered.

Remaining: gate's broader affected-suite verification and the caller's publication
step. No landing is authorized.

Checks: `cargo test -p loopflow --test commit_tests -- --skip commit_generates_message_when_none` passed (8); disposable branch CLI passed; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Agent message generation is outside this repair; broader suites belong to gate.
